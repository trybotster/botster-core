//! Package runnable entrypoint launch contract DTOs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PackageManifest;

/// Static runnable entrypoint metadata carried by a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnableEntrypoint {
    /// Stable runnable entrypoint id within the package.
    pub id: String,
    /// Semantic application kind.
    pub kind: RunnableEntrypointKind,
    /// Host launch mode requested by the package.
    pub launch_mode: RunnableEntrypointLaunchMode,
    /// Command path or executable name relative to the host launch policy.
    pub command: String,
    /// Command arguments declared by the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Declarative working-directory policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<RunnableEntrypointWorkingDirectory>,
    /// Hub-owned values the launcher must inject before running this entrypoint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub injections: Vec<RunnableEntrypointInjection>,
    /// Environment requirements declared by the package, not host-resolved values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment: Vec<RunnableEntrypointEnvironmentRequirement>,
    /// Readiness metadata a host may use to interpret structured launch output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<RunnableEntrypointReadiness>,
}

/// Semantic runnable entrypoint kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnableEntrypointKind {
    /// Browser-rendered first-party/client app.
    WebApp,
    /// Terminal-rendered first-party/client app.
    TerminalApp,
}

/// Requested launch mode for a runnable entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnableEntrypointLaunchMode {
    /// Host may keep the process in the background.
    Background,
    /// Host should attach foreground stdio if it launches the process.
    ForegroundStdio,
}

/// Declarative working-directory policies for runnable entrypoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum RunnableEntrypointWorkingDirectory {
    /// Launch from the package root.
    PackageRoot,
    /// Launch from the directory containing the command path.
    EntrypointDir,
    /// Launch from a path relative to the package root.
    Relative {
        /// Relative working-directory path.
        path: String,
    },
}

/// Hub-owned value a runnable entrypoint requires before launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnableEntrypointInjection {
    /// Required value kind.
    pub kind: RunnableEntrypointInjectionKind,
    /// Where the launcher should inject the value.
    pub target: RunnableEntrypointInjectionTarget,
    /// Whether the value is required before launch.
    #[serde(default)]
    pub required: bool,
    /// Optional host-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Required hub-owned injection value kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnableEntrypointInjectionKind {
    /// Hub connection endpoint or descriptor.
    HubConnection,
    /// Host-selected package data directory.
    DataDir,
    /// Local hub socket path or descriptor.
    HubSocket,
}

/// Injection target shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnableEntrypointInjectionTarget {
    /// Inject as an environment variable.
    Environment {
        /// Environment variable name.
        name: String,
    },
    /// Inject as a command argument placeholder.
    Argument {
        /// Argument placeholder value.
        value: String,
    },
}

/// Declarative environment requirement for a runnable entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnableEntrypointEnvironmentRequirement {
    /// Environment variable name.
    pub name: String,
    /// Whether the host must provide this environment variable before launch.
    #[serde(default)]
    pub required: bool,
    /// Optional manifest default. This is not a host-resolved value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Optional host-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Readiness metadata for structured runnable launch output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnableEntrypointReadiness {
    /// Structured output fields a host may expect from launch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result_fields: Vec<RunnableEntrypointResultField>,
}

/// Structured launch output fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnableEntrypointResultField {
    /// Local URL exposed by a launched app.
    LocalUrl,
}

/// Structured launch result DTO emitted by a host after launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnableEntrypointLaunchResult {
    /// Runnable entrypoint id this result belongs to.
    pub entrypoint_id: String,
    /// Observed process state.
    #[serde(default)]
    pub process_state: RunnableEntrypointProcessState,
    /// Optional local URL produced by the launched entrypoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_url: Option<String>,
}

/// Process state DTO for runnable entrypoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunnableEntrypointProcessState {
    /// The entrypoint has not been launched.
    #[default]
    NotStarted,
    /// The host has started the entrypoint process.
    Running,
    /// The entrypoint process exited.
    Exited,
    /// The host observed a launch failure.
    Failed,
}

/// Runnable entrypoint validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunnableEntrypointValidationError {
    /// Runnable entrypoint id is blank.
    #[error("runnable entrypoint id is blank")]
    BlankId,
    /// Runnable entrypoint id is duplicated.
    #[error("duplicate runnable entrypoint id {0}")]
    DuplicateId(String),
    /// Runnable entrypoint command is blank.
    #[error("runnable entrypoint {0} has a blank command")]
    BlankCommand(String),
    /// Relative working-directory path is blank.
    #[error("runnable entrypoint {0} has a blank relative working directory")]
    BlankRelativeWorkingDirectory(String),
    /// Required injection metadata is missing.
    #[error("runnable entrypoint {entrypoint_id} is missing required injection {kind:?}")]
    MissingRequiredInjection {
        /// Runnable entrypoint id.
        entrypoint_id: String,
        /// Missing injection kind.
        kind: RunnableEntrypointInjectionKind,
    },
    /// Injection environment variable name is blank.
    #[error("runnable entrypoint {0} has a blank injection environment name")]
    BlankInjectionEnvironment(String),
    /// Injection argument placeholder is blank.
    #[error("runnable entrypoint {0} has a blank injection argument")]
    BlankInjectionArgument(String),
    /// Environment requirement name is blank.
    #[error("runnable entrypoint {0} has a blank environment requirement name")]
    BlankEnvironmentRequirement(String),
}

impl RunnableEntrypoint {
    /// Validate core contract invariants for one runnable entrypoint.
    pub fn validate(&self) -> Result<(), RunnableEntrypointValidationError> {
        let id = self.id.trim();
        if id.is_empty() {
            return Err(RunnableEntrypointValidationError::BlankId);
        }

        if self.command.trim().is_empty() {
            return Err(RunnableEntrypointValidationError::BlankCommand(
                self.id.clone(),
            ));
        }

        if let Some(RunnableEntrypointWorkingDirectory::Relative { path }) = &self.working_directory
        {
            if path.trim().is_empty() {
                return Err(
                    RunnableEntrypointValidationError::BlankRelativeWorkingDirectory(
                        self.id.clone(),
                    ),
                );
            }
        }

        let required_injections: HashSet<_> = self
            .injections
            .iter()
            .filter(|injection| injection.required)
            .map(|injection| injection.kind.clone())
            .collect();
        for kind in [
            RunnableEntrypointInjectionKind::HubConnection,
            RunnableEntrypointInjectionKind::DataDir,
            RunnableEntrypointInjectionKind::HubSocket,
        ] {
            if !required_injections.contains(&kind) {
                return Err(
                    RunnableEntrypointValidationError::MissingRequiredInjection {
                        entrypoint_id: self.id.clone(),
                        kind,
                    },
                );
            }
        }

        for injection in &self.injections {
            match &injection.target {
                RunnableEntrypointInjectionTarget::Environment { name }
                    if name.trim().is_empty() =>
                {
                    return Err(
                        RunnableEntrypointValidationError::BlankInjectionEnvironment(
                            self.id.clone(),
                        ),
                    );
                }
                RunnableEntrypointInjectionTarget::Argument { value }
                    if value.trim().is_empty() =>
                {
                    return Err(RunnableEntrypointValidationError::BlankInjectionArgument(
                        self.id.clone(),
                    ));
                }
                _ => {}
            }
        }

        if self
            .environment
            .iter()
            .any(|requirement| requirement.name.trim().is_empty())
        {
            return Err(
                RunnableEntrypointValidationError::BlankEnvironmentRequirement(self.id.clone()),
            );
        }

        Ok(())
    }
}

/// Validate runnable entrypoint invariants declared by a package manifest.
pub fn validate_package_runnable_entrypoints(
    manifest: &PackageManifest,
) -> Result<(), RunnableEntrypointValidationError> {
    let mut ids = HashSet::new();

    for entrypoint in &manifest.runnable_entrypoints {
        entrypoint.validate()?;
        if !ids.insert(entrypoint.id.clone()) {
            return Err(RunnableEntrypointValidationError::DuplicateId(
                entrypoint.id.clone(),
            ));
        }
    }

    Ok(())
}
