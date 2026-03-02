//! SARIF 2.1.0 output for GitHub Code Scanning.

use crate::checks::model::{Finding, Severity};
use crate::model::Project;
use serde_json::json;

/// Known rule IDs and their short descriptions for SARIF.
const RULE_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "dep/find_package_missing",
        "find_package(X) in CMake but not in package.xml",
    ),
    (
        "dep/ament_target_undeclared",
        "ament_target_dependencies references package not in package.xml",
    ),
    ("dep/circular", "Circular dependency in package graph"),
    ("launch/include_cycle", "Include cycle in launch file graph"),
    (
        "launch/missing_package",
        "Included package not in workspace",
    ),
    (
        "runtime/topic_no_publisher",
        "Topic has subscriber(s) but no publishers",
    ),
    (
        "runtime/topic_no_subscriber",
        "Topic has publisher(s) but no subscribers",
    ),
    (
        "runtime/topic_type_mismatch",
        "Topic has publisher/subscriber type mismatch",
    ),
    (
        "runtime/service_type_mismatch",
        "Service has server/client type mismatch",
    ),
    ("manifest/missing_description", "Package has no description"),
    ("manifest/no_maintainer", "Package has no maintainers"),
];

fn severity_to_sarif_level(sev: Severity) -> &'static str {
    match sev {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Info => "note",
    }
}

fn rule_description(rule_id: &str) -> &'static str {
    RULE_DESCRIPTIONS
        .iter()
        .find(|(id, _)| *id == rule_id)
        .map(|(_, desc)| *desc)
        .unwrap_or("ROS2 project analysis finding")
}

/// Resolve location to a URI relative to workspace root.
fn location_uri(loc: &crate::checks::model::Location, project: &Project) -> Option<String> {
    let package_name = loc.package.as_ref()?;
    let file = loc.file.as_ref()?;
    let pkg = project.packages.iter().find(|p| p.name() == package_name)?;
    let pkg_dir = pkg.path.parent().or(Some(&pkg.path))?;
    let full = pkg_dir.join(file);
    full.strip_prefix(&project.workspace_root)
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .or_else(|| full.to_str().map(String::from))
}

/// Convert findings to SARIF 2.1.0 JSON for GitHub Code Scanning.
pub fn findings_to_sarif(findings: &[Finding], project: &Project) -> serde_json::Value {
    let unique_rule_ids: std::collections::HashSet<_> =
        findings.iter().map(|f| f.rule_id.as_str()).collect();

    let rules: Vec<serde_json::Value> = unique_rule_ids
        .iter()
        .map(|&id| {
            json!({
                "id": id,
                "shortDescription": {"text": rule_description(id)}
            })
        })
        .collect();

    let results: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let mut result = json!({
                "ruleId": f.rule_id,
                "level": severity_to_sarif_level(f.severity),
                "message": {"text": f.message}
            });

            if let Some(ref loc) = f.location {
                let uri = location_uri(loc, project);
                let region = loc
                    .line
                    .map(|ln| json!({"startLine": ln}))
                    .unwrap_or(json!({"startLine": 1}));

                let ploc = if let Some(uri) = uri {
                    json!({
                        "artifactLocation": {"uri": uri},
                        "region": region
                    })
                } else {
                    json!({"artifactLocation": {"uri": "unknown"}, "region": region})
                };

                result["locations"] = json!([{"physicalLocation": ploc}]);
            }
            result
        })
        .collect();

    json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif/sarif-2.1.0/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "babel42",
                    "informationUri": "https://github.com/elan8/babel42",
                    "rules": rules
                }
            },
            "results": results
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::model::{Finding, Location};
    use crate::model::{Package, PackageManifest, Project};
    use petgraph::graph::DiGraph;
    use std::path::PathBuf;

    fn minimal_project(root: PathBuf) -> Project {
        Project {
            workspace_root: root.clone(),
            packages: vec![Package {
                manifest: PackageManifest {
                    format: 3,
                    name: "test_pkg".to_string(),
                    version: "0.1".to_string(),
                    description: Some("x".to_string()),
                    maintainers: vec![],
                    license: None,
                    urls: vec![],
                    authors: vec![],
                    dependencies: vec![],
                    build_types: vec![],
                    member_of_groups: vec![],
                    is_metapackage: false,
                },
                path: root.join("test_pkg").join("package.xml"),
                messages: vec![],
                services: vec![],
                actions: vec![],
                cmake_info: None,
                setup_py_info: None,
                xacro_files: vec![],
                launch_files: vec![],
                config_files: vec![],
                python_node_files: vec![],
                cpp_node_files: vec![],
            }],
            package_index: [("test_pkg".to_string(), 0)].into_iter().collect(),
            dependency_graph: DiGraph::new(),
            launch_include_graph: crate::model::LaunchIncludeGraph(DiGraph::new()),
            runtime_graph: crate::model::RuntimeGraph {
                nodes: vec![],
                topics: vec![],
                services: vec![],
                topic_publishers: vec![],
                topic_subscribers: vec![],
                service_servers: vec![],
                service_clients: vec![],
            },
        }
    }

    #[test]
    fn findings_to_sarif_produces_valid_structure() {
        let root = PathBuf::from("/ws");
        let project = minimal_project(root);
        let findings = vec![Finding::new(
            "dep/find_package_missing",
            Severity::Error,
            "find_package(sensor_msgs) in CMake but not in package.xml",
        )
        .with_location(Location {
            package: Some("test_pkg".to_string()),
            file: Some("CMakeLists.txt".to_string()),
            line: Some(5),
            context: None,
        })
        .with_fix_hint("Add <depend>sensor_msgs</depend>")];

        let sarif = findings_to_sarif(&findings, &project);
        assert_eq!(sarif["version"], "2.1.0");
        assert!(sarif["runs"][0]["tool"]["driver"]["name"]
            .as_str()
            .unwrap()
            .contains("babel42"));
        assert_eq!(sarif["runs"][0]["results"].as_array().unwrap().len(), 1);
        assert_eq!(
            sarif["runs"][0]["results"][0]["ruleId"],
            "dep/find_package_missing"
        );
        assert_eq!(sarif["runs"][0]["results"][0]["level"], "error");
    }
}
