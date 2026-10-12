//! Services (Core section 10), only as far as the listed clauses need: the admission refusals and the lifecycle events.

use crate::pump::{fail, ok, Step};
use crate::state::*;
use crate::Inner;
use botster_core_contract::prelude::*;

fn bound_of(feature: Feature) -> Option<Bound> {
    match feature {
        Feature::BoundCpuSeconds => Some(Bound::CpuSeconds),
        Feature::BoundOpenFiles => Some(Bound::OpenFiles),
        Feature::BoundProcesses => Some(Bound::Processes),
        Feature::BoundAddressSpaceBytes => Some(Bound::AddressSpaceBytes),
        _ => None,
    }
}

impl Inner {
    fn requested(limits: &ServiceLimits) -> Vec<(Bound, bool)> {
        let r = &limits.rlimits;
        [
            (Bound::CpuSeconds, &r.cpu_seconds),
            (Bound::OpenFiles, &r.open_files),
            (Bound::Processes, &r.processes),
            (Bound::AddressSpaceBytes, &r.address_space_bytes),
        ]
        .into_iter()
        .filter_map(|(b, spec)| spec.as_ref().map(|s| (b, s.required)))
        .collect()
    }

    fn supports(&self, bound: Bound) -> bool {
        !self.unavailable.contains(&bound)
            && self
                .feature_set()
                .into_iter()
                .filter_map(bound_of)
                .any(|b| b == bound)
    }

    /// Core SV-1, SV-4, 9.3: the refusals of `SpawnService` at `begin`.
    pub(crate) fn admit_spawn(
        &mut self,
        id: &ServiceId,
        limits: &ServiceLimits,
    ) -> Result<(Lane, Option<InstanceId>), CoreError> {
        if self.services.contains_key(id) {
            return Self::err(ErrorCode::IdInUse, "the service id is taken");
        }
        if self.services.len() >= self.limits.max_services as usize {
            return Self::err(ErrorCode::ServiceLimit, "max_services reached");
        }
        // Core SV-4: a bound that the host marked `required` and that cannot be applied refuses the spawn.
        for (bound, required) in Self::requested(limits) {
            if required && !self.supports(bound) {
                return Self::err(
                    ErrorCode::BoundUnavailable { bound },
                    "the bound cannot be applied",
                );
            }
        }
        let report = SpawnReport {
            applied: Self::requested(limits)
                .into_iter()
                .map(|(b, _)| b)
                .filter(|b| self.supports(*b))
                .collect(),
            unsupported: Self::requested(limits)
                .into_iter()
                .map(|(b, _)| b)
                .filter(|b| !self.supports(*b))
                .collect(),
        };
        self.services.insert(
            *id,
            Svc {
                state: None,
                report,
            },
        );
        Ok((Lane::Free, None))
    }

    /// Core A2-1: `StopService` and `RemoveService` are refused for an unknown id or a service in the wrong state.
    pub(crate) fn admit_service(
        &self,
        id: &ServiceId,
        op: &Op,
    ) -> Result<(Lane, Option<InstanceId>), CoreError> {
        let svc = self
            .services
            .get(id)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownService, "no such service"))?;
        let ended = matches!(
            svc.state,
            Some(ServiceState::Exited(_) | ServiceState::Lost { .. })
        );
        let live = matches!(
            svc.state,
            Some(ServiceState::Started | ServiceState::Reconnected { .. })
        );
        let admitted = match op {
            Op::StopService { .. } => live || ended,
            _ => ended,
        };
        if !admitted {
            return Self::err(ErrorCode::WrongState, "not admitted in this state");
        }
        Ok((Lane::Free, None))
    }

    pub(crate) fn service_op(&mut self, op: &Op, _stage: u8) -> Step {
        match op {
            Op::SpawnService { service_id, .. } => {
                let Some(svc) = self.services.get_mut(service_id) else {
                    return fail(ErrorCode::UnknownService, "no such service");
                };
                // Core SV-5: `Started` is posted after `exec`; the completion carries the report.
                svc.state = Some(ServiceState::Started);
                let report = svc.report.clone();
                self.post(Event::ServiceState {
                    id: *service_id,
                    state: ServiceState::Started,
                });
                ok(OpOutput::Spawn(report))
            }
            Op::StopService { id } => {
                let Some(svc) = self.services.get_mut(id) else {
                    return fail(ErrorCode::UnknownService, "no such service");
                };
                let exit = match svc.state {
                    Some(ServiceState::Exited(e)) => e,
                    _ => ServiceExit {
                        code: None,
                        signal: Some(15),
                        cause: ServiceExitCause::HostStop,
                        descendants_may_remain: false,
                    },
                };
                svc.state = Some(ServiceState::Exited(exit));
                self.post(Event::ServiceState {
                    id: *id,
                    state: ServiceState::Exited(exit),
                });
                ok(OpOutput::ServiceEnd(ServiceEnd::Exited(exit)))
            }
            Op::RemoveService { id } => {
                self.services.remove(id);
                // Core EV-6: a service's `Released` retires that service's keyed events.
                self.post(Event::ServiceState {
                    id: *id,
                    state: ServiceState::Released,
                });
                ok(OpOutput::Unit)
            }
            _ => fail(ErrorCode::Internal, "not a service op"),
        }
    }
}
