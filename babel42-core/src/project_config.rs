//! Babel42 project configuration — .babel42.yaml in workspace root.

use serde::Deserialize;
use std::path::Path;

/// Babel42 project configuration.
#[derive(Debug, Clone, Default)]
pub struct Babel42Config {
    /// Packages to skip in dep checks (project-specific overrides).
    pub skip_packages: Vec<String>,
    /// Workspace discovery settings.
    pub workspace: WorkspaceConfig,
}

/// Workspace discovery configuration.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Maximum depth to search for package.xml (default 8).
    pub max_depth: u32,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self { max_depth: 8 }
    }
}

#[derive(Deserialize, Default)]
struct Babel42ConfigDeser {
    #[serde(default)]
    skip_packages: Vec<String>,
    #[serde(default)]
    workspace: WorkspaceConfigDeser,
}

#[derive(Deserialize, Default)]
struct WorkspaceConfigDeser {
    #[serde(default = "default_max_depth")]
    max_depth: u32,
}

fn default_max_depth() -> u32 {
    8
}

/// Load project config from .babel42.yaml in workspace root.
/// Returns default config if file not found or invalid.
pub fn load_project_config(workspace_root: &Path) -> Babel42Config {
    let path = workspace_root.join(".babel42.yaml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Babel42Config::default(),
    };
    match serde_yaml::from_str::<Babel42ConfigDeser>(&content) {
        Ok(deser) => Babel42Config {
            skip_packages: deser.skip_packages,
            workspace: WorkspaceConfig {
                max_depth: deser.workspace.max_depth,
            },
        },
        Err(_) => Babel42Config::default(),
    }
}
