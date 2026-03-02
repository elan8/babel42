//! setup.py parser for ament_python packages.
//!
//! Extracts entry_points (console_scripts) for ROS2 executables.
//! These define the commands available via `ros2 run package_name executable_name`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Parsed information from setup.py (ament_python packages).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetupPyInfo {
    /// Executable names from entry_points['console_scripts'].
    pub entry_points: Vec<String>,
    /// (executable_name, module_path) for mapping Python files to executables.
    /// module_path maps to path: "foo.bar" → "foo/bar.py"
    pub entry_point_modules: Vec<(String, String)>,
}

#[derive(Debug, Error)]
pub enum SetupPyError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
}

/// Regex to match entry point strings: 'name=module:func' or "name=module:func"
/// Captures: script name (group 1)
fn entry_point_regex() -> Regex {
    Regex::new(r#"(?m)['"]([a-zA-Z0-9_\-.]+)=([a-zA-Z0-9_.]+:[a-zA-Z0-9_]+)['"]"#).unwrap()
}

/// Parse setup.py and extract console_scripts entry points.
///
/// Looks for patterns like:
///   'executable_name=package.module:main',
///   "my_node=my_pkg.script:main",
pub fn parse_setup_py(path: &Path) -> Result<SetupPyInfo, SetupPyError> {
    let content = fs::read_to_string(path)?;
    Ok(parse_setup_py_str(&content))
}

/// Parse setup.py content (for testing and in-memory use).
pub fn parse_setup_py_str(content: &str) -> SetupPyInfo {
    let re = entry_point_regex();
    let mut entry_points = Vec::new();
    let mut entry_point_modules = Vec::new();
    for cap in re.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str().to_string());
        let module_func = cap.get(2).map(|m| m.as_str().to_string()); // "module:func"
        if let (Some(n), Some(mf)) = (name, module_func) {
            entry_points.push(n.clone());
            let module = mf.split(':').next().unwrap_or(&mf).to_string();
            entry_point_modules.push((n, module));
        }
    }
    SetupPyInfo {
        entry_points,
        entry_point_modules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_entry_points() {
        let s = r#"
from setuptools import setup

setup(
    name='my_pkg',
    version='0.1.0',
    entry_points={
        'console_scripts': [
            'my_node=my_pkg.main:main',
            'other_node=my_pkg.other:run',
        ],
    },
)
"#;
        let info = parse_setup_py_str(s);
        assert_eq!(info.entry_points, ["my_node", "other_node"]);
    }

    #[test]
    fn parse_double_quoted() {
        let s = r#"
entry_points={"console_scripts": ["listener=py_pkg.listener:main"]}
"#;
        let info = parse_setup_py_str(s);
        assert_eq!(info.entry_points, ["listener"]);
    }

    #[test]
    fn parse_hyphen_in_name() {
        let s = r#"
'hello-world=tim.pkg:hello_world',
"#;
        let info = parse_setup_py_str(s);
        assert_eq!(info.entry_points, ["hello-world"]);
    }

    #[test]
    fn parse_empty() {
        let info = parse_setup_py_str("setup(name='x')");
        assert!(info.entry_points.is_empty());
        assert!(info.entry_point_modules.is_empty());
    }

    #[test]
    fn parse_entry_point_modules() {
        let s = "'talker=my_pkg.publisher:main',";
        let info = parse_setup_py_str(s);
        assert_eq!(info.entry_point_modules.len(), 1);
        assert_eq!(info.entry_point_modules[0], ("talker".to_string(), "my_pkg.publisher".to_string()));
    }
}
