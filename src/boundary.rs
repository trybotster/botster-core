//! Layer names and ownership descriptions.

/// A named Botster architecture layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Reusable mechanisms and stable contracts.
    Core,
    /// Botster policy, orchestration, lifecycle, and extension supervision.
    Hub,
    /// Executable entrypoint and operator commands.
    Cli,
    /// User or provider supplied behavior loaded through extension contracts.
    Extension,
    /// Concrete user interfaces and transport adapters.
    Client,
}

/// Human-readable responsibility text for a layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerResponsibility {
    /// Layer being described.
    pub layer: Layer,
    /// Concise ownership statement.
    pub owns: &'static str,
    /// Concise exclusion statement.
    pub does_not_own: &'static str,
}

/// Return the canonical ownership statement for each layer.
#[must_use]
pub const fn responsibility(layer: Layer) -> LayerResponsibility {
    match layer {
        Layer::Core => LayerResponsibility {
            layer,
            owns: "reusable mechanisms and transport-neutral contracts",
            does_not_own: "Botster product policy or executable startup flow",
        },
        Layer::Hub => LayerResponsibility {
            layer,
            owns: "runtime policy, lifecycle, routing, recovery, and extension supervision",
            does_not_own: "raw terminal byte delivery or CLI argument parsing",
        },
        Layer::Cli => LayerResponsibility {
            layer,
            owns: "operator commands and process startup",
            does_not_own: "runtime policy or reusable protocol contracts",
        },
        Layer::Extension => LayerResponsibility {
            layer,
            owns: "installed behavior composed from granted capabilities",
            does_not_own: "implicit access to hub internals or private key material",
        },
        Layer::Client => LayerResponsibility {
            layer,
            owns: "presentation, local input, and concrete transport adaptation",
            does_not_own: "session lifecycle policy",
        },
    }
}
