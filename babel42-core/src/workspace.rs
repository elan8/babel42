//! ROS2 workspace discovery.

use crate::project_config::WorkspaceConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A discovered ROS2 workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Root path of the workspace.
    pub root: PathBuf,
    /// Paths to package.xml files (one per package).
    pub packages: Vec<PathBuf>,
}

/// Detect a ROS2 workspace rooted at the given path.
///
/// Supports three layouts:
/// - **Standard**: `src/` subdirectory containing package directories with `package.xml`
/// - **Flat**: Package directories with `package.xml` directly under the root
/// - **Root-as-package**: `package.xml` in root (e.g. slam_toolbox; root is the package dir)
///
/// Uses default WorkspaceConfig (max_depth 8) when workspace_config is None.
pub fn discover_workspace(root: &Path) -> Option<Workspace> {
    discover_workspace_with_config(root, None)
}

/// Like discover_workspace but with optional workspace config for max_depth etc.
pub fn discover_workspace_with_config(
    root: &Path,
    workspace_config: Option<&WorkspaceConfig>,
) -> Option<Workspace> {
    let max_depth = workspace_config
        .map(|c| c.max_depth as usize)
        .unwrap_or(8);
    discover_workspace_impl(root, max_depth)
}

fn dir_is_ignored(path: &Path) -> bool {
    path.join("COLCON_IGNORE").is_file() || path.join("AMENT_IGNORE").is_file()
}

fn discover_workspace_impl(root: &Path, max_depth: usize) -> Option<Workspace> {
    let mut packages = Vec::new();
    let root_manifest = root.join("package.xml");
    if root_manifest.is_file() && !dir_is_ignored(root) {
        packages.push(root_manifest.clone());
    }
    for entry in WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                !dir_is_ignored(e.path())
            } else {
                true
            }
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // skip errors (e.g. permission, symlink)
        };
        if entry.file_type().is_file() && entry.file_name() == "package.xml" {
            let path = entry.path().to_path_buf();
            if path != root_manifest {
                packages.push(path);
            }
        }
    }

    if packages.is_empty() {
        return None;
    }

    Some(Workspace {
        root: root.to_path_buf(),
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discover_workspace_empty_dir_returns_none() {
        let tmp = std::env::temp_dir().join("babel42_test_empty");
        let _ = fs::create_dir_all(&tmp);
        assert!(discover_workspace(&tmp).is_none());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_workspace_flat_layout() {
        let tmp = std::env::temp_dir().join("babel42_test_flat");
        let _ = fs::remove_dir_all(&tmp);
        let pkg1 = tmp.join("pkg1");
        let pkg2 = tmp.join("pkg2");
        fs::create_dir_all(&pkg1).unwrap();
        fs::create_dir_all(&pkg2).unwrap();
        fs::write(
            pkg1.join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>pkg1</name><version>1.0</version></package>"#,
        )
        .unwrap();
        fs::write(
            pkg2.join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>pkg2</name><version>2.0</version></package>"#,
        )
        .unwrap();

        let ws = discover_workspace(&tmp).expect("should find workspace");
        assert_eq!(ws.packages.len(), 2);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_workspace_finds_packages() {
        let tmp = std::env::temp_dir().join("babel42_test_ws");
        let _ = fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        let pkg1 = src.join("pkg1");
        let pkg2 = src.join("pkg2");
        fs::create_dir_all(&pkg1).unwrap();
        fs::create_dir_all(&pkg2).unwrap();
        fs::write(
            pkg1.join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>pkg1</name><version>1.0</version></package>"#,
        )
        .unwrap();
        fs::write(
            pkg2.join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>pkg2</name><version>2.0</version></package>"#,
        )
        .unwrap();

        let ws = discover_workspace(&tmp).expect("should find workspace");
        assert_eq!(ws.packages.len(), 2);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_workspace_root_as_package() {
        let tmp = std::env::temp_dir().join("babel42_test_root_pkg");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::create_dir_all(tmp.join("src")).unwrap(); // package's C++ src, not workspace src
        fs::write(
            tmp.join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>root_pkg</name><version>1.0</version></package>"#,
        )
        .unwrap();
        let ws = discover_workspace(&tmp).expect("should find workspace");
        assert_eq!(ws.packages.len(), 1);
        assert!(ws.packages[0].ends_with("package.xml"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_workspace_root_as_package_with_nested() {
        // slam_toolbox layout: root/package.xml + root/lib/karto_sdk/package.xml
        let tmp = std::env::temp_dir().join("babel42_test_root_pkg_nested");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("lib").join("karto_sdk")).unwrap();
        fs::write(
            tmp.join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>main</name><version>1.0</version></package>"#,
        )
        .unwrap();
        fs::write(
            tmp.join("lib").join("karto_sdk").join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>karto_sdk</name><version>1.0</version></package>"#,
        )
        .unwrap();
        let ws = discover_workspace(&tmp).expect("should find workspace");
        assert_eq!(ws.packages.len(), 2, "should find root + lib/karto_sdk");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_workspace_skips_colcon_ignore() {
        let tmp = std::env::temp_dir().join("babel42_test_colcon_ignore");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("pkg1")).unwrap();
        fs::create_dir_all(tmp.join("pkg2").join("pkg3")).unwrap();
        fs::write(
            tmp.join("pkg1").join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>pkg1</name><version>1.0</version></package>"#,
        )
        .unwrap();
        fs::write(
            tmp.join("pkg2").join("COLCON_IGNORE"),
            "",
        )
        .unwrap();
        fs::write(
            tmp.join("pkg2").join("pkg3").join("package.xml"),
            r#"<?xml version="1.0"?><package format="2"><name>pkg3</name><version>1.0</version></package>"#,
        )
        .unwrap();
        let ws = discover_workspace(&tmp).expect("should find workspace");
        assert_eq!(ws.packages.len(), 1, "pkg2 and pkg3 should be skipped due to COLCON_IGNORE");
        assert!(ws.packages[0].to_string_lossy().contains("pkg1"));
        let _ = fs::remove_dir_all(&tmp);
    }

}
