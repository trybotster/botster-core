//! `FakeCore` as a [`CoreHarness`] (design section 4: the adapter lives in the runner crate, so the fake never depends on the runner).

use crate::harness::*;
use botster_conformance::Deadline;
use botster_core_contract::prelude::*;
use botster_fake_core::{FakeCore, FakeStore, CONTROLS};
use botster_fake_route::{Read, CONTROLS as ROUTE_CONTROLS};
use botster_hub_conformance::route::RouteRead;
use serde_json::Value;
use std::collections::BTreeMap;

/// Whether a type holds a bound, decided at compile time at the call site (autoref specialization).
macro_rules! holds {
    ($t:ty: $bound:path) => {{
        struct Probe<T>(std::marker::PhantomData<T>);
        #[allow(dead_code)]
        trait Yes {
            fn is(&self) -> bool {
                true
            }
        }
        impl<T: $bound> Yes for Probe<T> {}
        #[allow(dead_code)]
        trait No {
            fn is(&self) -> bool {
                false
            }
        }
        impl<T> No for &Probe<T> {}
        #[allow(clippy::needless_borrow)]
        (&Probe::<$t>(std::marker::PhantomData)).is()
    }};
}

/// The fake, built for one seed.
pub struct FakeCoreHarness {
    seed: u64,
    stores: BTreeMap<String, FakeStore>,
    cores: BTreeMap<String, FakeCore>,
}

impl FakeCoreHarness {
    pub fn new(seed: u64) -> Self {
        FakeCoreHarness {
            seed,
            stores: BTreeMap::new(),
            cores: BTreeMap::new(),
        }
    }
}

struct FakeEnd(botster_fake_core::FakeRouteEnd);

impl RouteClient for FakeEnd {
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }

    fn read(&mut self, max: usize, _deadline: &Deadline) -> RouteRead {
        match self.0.read(max) {
            Read::Bytes(b) => RouteRead::Bytes(b),
            Read::Blocked => RouteRead::Blocked,
            Read::Empty => RouteRead::Empty,
            Read::Eof { ended } => RouteRead::Eof {
                ended: ended.map(|r| {
                    serde_json::to_value(r)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                }),
            },
        }
    }

    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String> {
        self.0.control(op, args)
    }

    fn has_control(&self, op: &str) -> bool {
        ROUTE_CONTROLS.contains(&op)
    }
}

impl CoreHarness for FakeCoreHarness {
    fn is_fake(&self) -> bool {
        true
    }

    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        let store = self
            .stores
            .entry(spec.data_dir.0.clone())
            .or_default()
            .clone();
        // Core 9B: a limits value that is not a `CoreLimits` is `InvalidConfig`.
        let limits: CoreLimits = serde_json::from_value(spec.limits.clone()).map_err(|e| {
            CoreError::new(
                ErrorCode::InvalidConfig {
                    field: e.to_string(),
                },
                e.to_string(),
            )
        })?;
        let config = OpenConfig {
            data_dir: spec.data_dir.0.clone().into(),
            // Core LC-1: `worker_path` is set only when the harness offers a worker.
            worker_path: spec.worker.as_ref().map(|_| "fake-worker".into()),
            limits,
        };
        let core = FakeCore::open(&store, config, self.seed)?;
        self.cores.insert(spec.handle.clone(), core.clone());
        Ok(Box::new(core))
    }

    fn data_dir(&mut self, name: &str) -> DataDirRef {
        self.stores.entry(name.to_string()).or_default();
        DataDirRef(name.to_string())
    }

    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        // The fake models protocol T only: AD-4 skew needs a real previous worker build.
        (which == WorkerBuild::Current).then_some(WorkerRef {
            build: which,
            file_name: None,
        })
    }

    fn drop_handle(&mut self, handle: &str) {
        self.cores.remove(handle);
    }

    fn core_is_send_not_sync(&self) -> Option<bool> {
        Some(holds!(FakeCore: Send) && !holds!(FakeCore: Sync))
    }

    fn has_control(&self, op: &str) -> bool {
        FakeCore::has_control(op) || CONTROLS.contains(&op)
    }

    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        let core = self
            .cores
            .get(handle)
            .ok_or_else(|| ControlError::Bad(format!("handle '{handle}' is not open")))?;
        core.control(op, args).map_err(ControlError::Bad)
    }

    fn probe_binary(&self) -> String {
        botster_probe_script::PROBE_BINARY.to_string()
    }

    fn attach_stream(
        &mut self,
        handle: &str,
        core: &mut dyn CoreApi,
        client: ClientId,
        session: &SessionId,
        options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        let result = core
            .attach(
                client,
                session.clone(),
                RouteTransport::Stream(StreamEndpoint::new(())),
                options,
            )
            .map_err(|refused| refused.error)?;
        // The route belongs to the named handle: two data directories can issue the same route id (Core ID-1).
        let end = self
            .cores
            .get(handle)
            .and_then(|c| c.route_end(result.route))
            .ok_or_else(|| CoreError::new(ErrorCode::Internal, "the handle is unknown"))?;
        Ok((result, Box::new(FakeEnd(end))))
    }
}
