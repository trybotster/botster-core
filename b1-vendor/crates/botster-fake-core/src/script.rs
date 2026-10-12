//! The probe script of a session (design 6.2, Core A5-1). The format lives in `botster-probe-script`, which has no fake dependency.
//! This module only fixes the type of the top-level `fake` part to the fake route's `SessionScript`, which a non-fake crate cannot name.

use botster_core_contract::prelude::SpawnRequest;
use botster_fake_route::script::SessionScript;

pub use botster_probe_script::{Step, StepFake, PROBE_BINARY};

/// A probe script whose `fake` part is the script of the fake worker (R-5). Real Core ignores that part.
pub type Script = botster_probe_script::Script<SessionScript>;

/// The script of a spawn request. A request that does not name the probe binary runs `Hold`. A script that does not parse is an error.
pub fn of(request: &SpawnRequest) -> Result<Script, String> {
    Script::from_argv(&request.argv).map_err(|e| e.to_string())
}
