//! `botster-fake-route` as a [`RouteSubject`] (design section 4: the adapter lives in the runner crate).

use super::{ClientModel, Framing, RouteRead, RouteSubject};
use botster_fake_route::{AttachSpec, FakeWorker, Read, WorkerConfig};
use botster_route_codec::prelude::FrameBounds;
use serde_json::Value;

/// The fake session worker, built for one seed.
pub struct FakeRouteSubject {
    seed: u64,
    cfg: WorkerConfig,
    worker: Option<FakeWorker>,
}

impl FakeRouteSubject {
    pub fn new(seed: u64) -> Self {
        FakeRouteSubject {
            seed,
            cfg: WorkerConfig::default(),
            worker: None,
        }
    }

    fn w(&mut self) -> &mut FakeWorker {
        let (seed, cfg) = (self.seed, &self.cfg);
        self.worker
            .get_or_insert_with(|| FakeWorker::new(seed, cfg.clone()))
    }
}

impl RouteSubject for FakeRouteSubject {
    fn is_fake(&self) -> bool {
        true
    }

    fn has_feature(&self, name: &str) -> bool {
        self.cfg.features.iter().any(|f| f == name)
    }

    fn has_control(&self, op: &str) -> bool {
        botster_fake_route::CONTROLS.contains(&op)
    }

    fn configure(&mut self, worker: &Value) -> Result<(), String> {
        if !worker.is_null() {
            self.cfg = serde_json::from_value(worker.clone()).map_err(|e| e.to_string())?;
        }
        self.worker = Some(FakeWorker::new(self.seed, self.cfg.clone()));
        Ok(())
    }

    fn attach(&mut self, spec: &Value) -> Result<u32, String> {
        let spec: AttachSpec = if spec.is_null() {
            AttachSpec::default()
        } else {
            serde_json::from_value(spec.clone()).map_err(|e| e.to_string())?
        };
        Ok(self.w().attach(&spec))
    }

    fn framing(&self) -> Framing {
        match self.cfg.framing {
            botster_fake_route::Framing::Stream => Framing::Stream,
            botster_fake_route::Framing::Chunked => Framing::Chunked,
        }
    }

    fn bounds(&self) -> FrameBounds {
        FrameBounds {
            max_frame: self.cfg.max_frame_bytes,
            max_screen: self.cfg.max_screen_frame_bytes,
            max_history: self.cfg.max_history_page_bytes,
        }
    }

    fn max_chunk_bytes(&self) -> usize {
        self.cfg.max_chunk_bytes as usize
    }

    fn write(&mut self, route: u32, bytes: &[u8]) {
        self.w().write(route, bytes);
    }

    fn read(
        &mut self,
        route: u32,
        max: usize,
        _deadline: &botster_conformance::Deadline,
    ) -> RouteRead {
        match self.w().read(route, max) {
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

    fn control(&mut self, op: &str, args: &Value, route: Option<u32>) -> Result<Value, String> {
        self.w().control(op, args, route).map_err(|e| e.0)
    }

    /// The fake has no terminal, so it has no client model either (R-5, R-7). A real subject supplies one.
    fn client_model(&self, _terminal_format: &str) -> Option<Box<dyn ClientModel>> {
        None
    }
}
