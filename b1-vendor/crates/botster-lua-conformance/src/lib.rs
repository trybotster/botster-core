//! The Lua binding conformance runner (design 6.5; ledger prefix `lb`, plus the Lua-host ids of the `hp` prefix). A `lua_host` transcript is a host-side script: the runner plays the
//! Hub with HP frames, and the plugin is Lua source in the transcript's `setup.package`.
//!
//! **No Lua host is built here.** The subject is a [`LuaHostSubject`]: the real Hub's Lua host, loaded in-process with the package, speaking HP frames through the same channel
//! endpoint as any plugin host ([`HpSubject`]). A Phase 1 repo calls `lua_host_conformance_tests!(make)` with `Fn(u64) -> Box<dyn LuaHostSubject>`. In Phase 0 the transcripts are
//! `fake: structural` (ruling R-1.4: this subject has no fake), and [`structural::StructuralLuaHost`] is the inert subject that selects the structural check.
//!
//! `setup.package` is `{manifest, files}`: the complete `botster-plugin.json` object, and each Lua source by its path relative to the package root (LB-17b). The harness obligations
//! of each control are in `docs/lua-host-controls.md`.

// Failure values carry whole JSON documents for the report. They occur on a cold path.
#![allow(clippy::result_large_err)]

pub mod structural;

use botster_conformance::SchemaResolver;
use botster_plugin_conformance::{Backpressured, HpSubject, Unsupported};
use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::collections::BTreeMap;

/// The Lua binding transcripts (`conformance/lb`), embedded.
pub static LB_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/lb");

/// The plugin package that a transcript runs (`setup.package`): LB-17 (manifest keys), LB-17b (the entry and the staged files).
#[derive(Debug, Clone, PartialEq)]
pub struct LuaPackage {
    /// The complete manifest object (HP MN-1).
    pub manifest: Value,
    /// Every file of the package by its path relative to the package root; each `*.lua` file is staged at load (LB-17b).
    pub files: BTreeMap<String, String>,
}

impl LuaPackage {
    /// Reads `setup.package`. `Err` names what is missing.
    pub fn from_setup(setup: &Value) -> Result<Option<LuaPackage>, String> {
        let Some(pkg) = setup.get("package") else {
            return Ok(None);
        };
        let manifest = pkg
            .get("manifest")
            .cloned()
            .ok_or("setup.package.manifest is missing")?;
        let mut files = BTreeMap::new();
        for (path, text) in pkg
            .get("files")
            .and_then(Value::as_object)
            .ok_or("setup.package.files is missing")?
        {
            files.insert(
                path.clone(),
                text.as_str()
                    .ok_or_else(|| format!("file '{path}' is not a string"))?
                    .to_string(),
            );
        }
        Ok(Some(LuaPackage { manifest, files }))
    }
}

/// The real Lua host as the subject of a transcript. It is an [`HpSubject`] (one channel endpoint that speaks HP frames; the runner plays the Hub) that also loads a package.
///
/// Clause: HP LH-1 to LH-4, LB-17b. A real implementation builds the host with injected parts (the clock, the random source, the package files) and never with a test branch in
/// production code (design 6.3).
pub trait LuaHostSubject: HpSubject {
    /// Stages the package files and records the manifest (LB-17b: every `*.lua` file under the package root is staged at load). The entrypoint runs when the runner sends `Hello`.
    fn load_package(&mut self, package: &LuaPackage) -> Result<(), String>;
}

/// Gives the HP driver a [`LuaHostSubject`]: `configure` loads the package first, then passes the setup on.
pub struct LuaAdapter(pub Box<dyn LuaHostSubject>);

impl HpSubject for LuaAdapter {
    fn is_fake(&self) -> bool {
        self.0.is_fake()
    }
    fn has_feature(&self, name: &str) -> bool {
        self.0.has_feature(name)
    }
    fn has_control(&self, op: &str) -> bool {
        self.0.has_control(op)
    }
    fn configure(&mut self, setup: &Value) -> Result<(), String> {
        if let Some(package) = LuaPackage::from_setup(setup)? {
            self.0.load_package(&package)?;
        }
        self.0.configure(setup)
    }
    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured> {
        self.0.write(conn, bytes)
    }
    fn read(&mut self, conn: &str) -> Option<Vec<u8>> {
        self.0.read(conn)
    }
    fn conns(&self) -> Vec<String> {
        self.0.conns()
    }
    fn closed(&self, conn: &str) -> Option<String> {
        self.0.closed(conn)
    }
    fn close_channel(&mut self, conn: &str) {
        self.0.close_channel(conn);
    }
    fn advance_clock(&mut self, ms: u64) {
        self.0.advance_clock(ms);
    }
    fn control(&mut self, op: &Value) -> Result<Value, Unsupported> {
        self.0.control(op)
    }
}

/// The controls that a Lua host subject implements (design 6.3); each is documented in `docs/lua-host-controls.md`.
pub const LUA_HOST_CONTROLS: &[&str] = &[
    "settle",
    "status",
    "received",
    "counters",
    "admit_package",
    "package_session_types",
    "load_entrypoint",
    "readable_globals",
];

/// Runs one transcript by id for a generated `#[test]`. Panics with the report line unless it passed.
///
/// The seed set and the selection come from `BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE` and `BOTSTER_SEED`.
pub fn run_test(
    crate_name: &str,
    dir: &'static Dir<'static>,
    id: &str,
    make: &dyn Fn(u64) -> Box<dyn LuaHostSubject>,
    schemas: &dyn SchemaResolver,
) {
    // The frame templates (`fixtures/frames.json`) are the HP ones: a Lua host speaks HP.
    botster_plugin_conformance::run_test_with(
        crate_name,
        dir,
        &botster_plugin_conformance::HP_TRANSCRIPTS,
        id,
        &|seed| Box::new(LuaAdapter(make(seed))),
        schemas,
    );
}

include!(concat!(env!("OUT_DIR"), "/lb_tests.rs"));
