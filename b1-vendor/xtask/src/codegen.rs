//! `cargo xtask codegen [--check]`: the frame of the codegen step (design section 8).
//!
//! The registry lists (output path, generator function) pairs. A later feature adds its generated artifacts here.
//! `codegen` writes every output. `codegen --check` fails when an output on disk differs from what its generator makes.

use anyhow::{bail, Context, Result};
use std::path::Path;

pub struct Generator {
    /// The output path, relative to the repo root.
    pub path: &'static str,
    /// Returns the full text of the output.
    pub generate: fn() -> Result<String>,
}

/// The generated artifacts. Later features add their schemas here.
pub fn registry() -> Vec<Generator> {
    let mut out = vec![
        Generator {
            path: "schemas/transcript.schema.json",
            generate: || Ok(botster_conformance::transcript_schema_json()),
        },
        Generator {
            path: "schemas/client-step.schema.json",
            generate: || Ok(botster_conformance::client_step_schema_json()),
        },
    ];
    out.extend(route_schemas());
    out.extend(core_schemas());
    out.push(Generator {
        path: "schemas/core/probe-script.schema.json",
        generate: || Ok(botster_probe_script::render(&botster_probe_script::schema())),
    });
    out.extend(ui_schemas());
    out.extend(plugin_artifacts());
    out.extend(hub_schemas());
    out
}

/// The artifacts of the plugin contract, under `schemas/plugin/` (HP Appendix B, A1, A2; design section 8).
fn plugin_artifacts() -> Vec<Generator> {
    macro_rules! plugin {
        ($file:literal) => {
            Generator {
                path: concat!("schemas/plugin/", $file),
                generate: || {
                    botster_plugin_contract::prelude::artifact_files()
                        .into_iter()
                        .find(|(name, _)| *name == $file)
                        .map(|(_, text)| text)
                        .ok_or_else(|| anyhow::anyhow!("no plugin artifact {}", $file))
                },
            }
        };
    }
    vec![
        plugin!("plugin-frames.schema.json"),
        plugin!("plugin-manifest.schema.json"),
        plugin!("plugin-calls.schema.json"),
        plugin!("plugin-errors.json"),
        plugin!("lua-host-globals.json"),
        plugin!("agent-definition.schema.json"),
        plugin!("accessory.schema.json"),
        plugin!("catalog.schema.json"),
    ]
}

/// The route-body schemas of the codec (design section 8). One file for each schema of `botster_route_codec::schema`.
fn route_schemas() -> Vec<Generator> {
    // Each path is a literal, so the registry keeps `&'static str`. The test below checks that the list matches the codec.
    macro_rules! route {
        ($($stem:literal),*) => {
            vec![$(Generator {
                path: concat!("schemas/hub-client/route/", $stem, ".schema.json"),
                generate: || {
                    let (_, schema) = botster_route_codec::schema::all()
                        .into_iter()
                        .find(|(name, _)| *name == $stem)
                        .ok_or_else(|| anyhow::anyhow!("no codec schema {}", $stem))?;
                    Ok(botster_route_codec::schema::render(&schema))
                },
            }),*]
        };
    }
    route!(
        "to-client",
        "to-worker",
        "attached",
        "baseline_begin",
        "baseline_end",
        "modes",
        "resync",
        "input_refused",
        "input_done",
        "terminal_query",
        "output_dropped",
        "route_closed",
        "input",
        "mode_flags",
        "route_limits"
    )
}

/// The schemas of the Core contract types, under `schemas/core/` (design section 8). One file for each schema of `botster_core_contract::schema`.
fn core_schemas() -> Vec<Generator> {
    macro_rules! core {
        ($($stem:literal),*) => {
            vec![$(Generator {
                path: concat!("schemas/core/", $stem, ".schema.json"),
                generate: || {
                    let (_, schema) = botster_core_contract::schema::all()
                        .into_iter()
                        .find(|(name, _)| *name == $stem)
                        .ok_or_else(|| anyhow::anyhow!("no core schema {}", $stem))?;
                    Ok(botster_core_contract::schema::render(&schema))
                },
            }),*]
        };
    }
    core!(
        "op",
        "op-result",
        "event",
        "core-error",
        "core-limits",
        "open-config",
        "spawn-request",
        "session-record",
        "terminal-state",
        "input-result",
        "cancel-result",
        "attach-options",
        "attach-result",
        "service-limits",
        "features",
        "facts",
        "pump-report",
        "wake"
    )
}

/// The schemas of the UI contract types, under `schemas/ui/` (design section 8).
fn ui_schemas() -> Vec<Generator> {
    macro_rules! ui {
        ($file:literal) => {
            Generator {
                path: concat!("schemas/ui/", $file),
                generate: || {
                    botster_ui_contract::prelude::schema_files()
                        .into_iter()
                        .find(|(name, _)| *name == $file)
                        .map(|(_, text)| text)
                        .ok_or_else(|| anyhow::anyhow!("no UI schema {}", $file))
                },
            }
        };
    }
    vec![
        ui!("view-document.schema.json"),
        ui!("realize-input.schema.json"),
        ui!("realized-tree.schema.json"),
        ui!("admission-failure.schema.json"),
        ui!("admission-context.schema.json"),
        ui!("action-ok.schema.json"),
        ui!("action-error.schema.json"),
        ui!("navigation-target.schema.json"),
    ]
}

/// The schemas of the Hub→client types, under `schemas/hub-client/` (design section 8): the envelope, the pairing frames, the MCP tool data, `errors.json`,
/// and one schema per kind.
fn hub_schemas() -> Vec<Generator> {
    macro_rules! hub {
        ($($file:literal),*) => {
            vec![$(Generator {
                path: concat!("schemas/hub-client/", $file),
                generate: || {
                    botster_hub_contract::schema::all()
                        .into_iter()
                        .find(|(name, _)| name == $file)
                        .map(|(_, schema)| botster_hub_contract::schema::render(&schema))
                        .ok_or_else(|| anyhow::anyhow!("no hub-client schema {}", $file))
                },
            }),*]
        };
    }
    let mut out = hub!(
        "envelope.schema.json",
        "pairing.schema.json",
        "mcp.schema.json",
        "kinds/hub.schema.json",
        "kinds/session.schema.json",
        "kinds/session_type.schema.json",
        "kinds/spawn_target.schema.json",
        "kinds/worktree.schema.json",
        "kinds/package.schema.json",
        "kinds/catalog.schema.json",
        "kinds/quarantine.schema.json",
        "kinds/navigation.schema.json",
        "kinds/view.schema.json",
        "kinds/plugin_log.schema.json",
        "kinds/operation.schema.json",
        "kinds/client_credential.schema.json",
        "kinds/push_subscription.schema.json",
        "kinds/route.schema.json",
        "kinds/kind.schema.json"
    );
    out.push(Generator {
        path: "schemas/hub-client/errors.json",
        generate: || Ok(botster_hub_contract::prelude::errors_json()),
    });
    out
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => bail!("usage: cargo xtask codegen [--check]"),
    };
    run(root, &registry(), check)
}

pub fn run(root: &Path, generators: &[Generator], check: bool) -> Result<()> {
    let mut stale = Vec::new();
    for g in generators {
        let want = (g.generate)().with_context(|| format!("generate {}", g.path))?;
        let path = root.join(g.path);
        if check {
            match std::fs::read_to_string(&path) {
                Ok(have) if have == want => {}
                Ok(_) => stale.push(format!("{}: differs from the generated text", g.path)),
                Err(_) => stale.push(format!("{}: missing", g.path)),
            }
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, want)?;
        }
    }
    if stale.is_empty() {
        println!(
            "codegen: {} artifact(s) {}",
            generators.len(),
            if check { "up to date" } else { "written" }
        );
        return Ok(());
    }
    let summary = format!(
        "{} artifact(s) out of date. Run `cargo xtask codegen`.",
        stale.len()
    );
    Err(crate::Problems::fail(stale, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_codec_schema_is_registered() {
        let registered = route_schemas().len();
        assert_eq!(registered, botster_route_codec::schema::all().len());
    }

    #[test]
    fn every_core_schema_is_registered() {
        assert_eq!(
            core_schemas().len(),
            botster_core_contract::schema::all().len()
        );
    }

    #[test]
    fn every_hub_schema_is_registered() {
        // One file for each schema of the crate, plus `errors.json`.
        assert_eq!(
            hub_schemas().len(),
            botster_hub_contract::schema::all().len() + 1
        );
    }

    #[test]
    fn an_empty_registry_succeeds() {
        run(Path::new("."), &[], true).unwrap();
        run(Path::new("."), &[], false).unwrap();
    }
}
