//! YAML config file parsing — scans config/*.yaml in ROS2 packages.
//!
//! ROS2 packages often use config/ for node parameters, MoveIt config, etc.

use crate::model::ConfigFile;
use std::path::Path;

/// Scan config/*.yaml and config/*.yml in a package directory.
pub fn scan_config_files(pkg_dir: &Path) -> Vec<ConfigFile> {
    let config_dir = pkg_dir.join("config");
    if !config_dir.is_dir() {
        return vec![];
    }
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(&config_dir).max_depth(2).into_iter() {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let ext = p.extension().and_then(|e| e.to_str());
        if ext != Some("yaml") && ext != Some("yml") {
            continue;
        }
        let rel = p.strip_prefix(pkg_dir).unwrap_or(p).to_path_buf();
        let content = std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok());
        files.push(ConfigFile { path: rel, content });
    }
    files
}
