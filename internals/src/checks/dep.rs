//! Dependency rules — find_package vs package.xml, ament_target_deps, circular deps.

use crate::checks::model::{Finding, Location, Severity};
use crate::model::Project;
use crate::project_config::Babel42Config;
use crate::skip_config::{dep_should_skip_package, is_system_vendor_package, is_test_only_package};
use petgraph::algo::toposort;
use std::collections::HashSet;

#[allow(dead_code)] // Public API; run_checks uses check_dep_with_config
pub fn check_dep(project: &Project) -> Vec<Finding> {
    check_dep_with_config(project, None)
}

pub fn check_dep_with_config(project: &Project, config: Option<&Babel42Config>) -> Vec<Finding> {
    let mut findings = Vec::new();

    for pkg in &project.packages {
        // dep/find_package_missing
        if let Some(ref cmake) = pkg.cmake_info {
            let declared: HashSet<&str> = pkg
                .manifest
                .dependencies
                .iter()
                .map(|d| d.name.as_str())
                .collect();

            for fp in &cmake.find_package {
                let fp_trim = fp.trim();
                if fp_trim.is_empty() || fp_trim.starts_with('$') {
                    continue;
                }
                let project_skip = config.map(|c| c.skip_packages.as_slice());
                if dep_should_skip_package(fp_trim, project_skip) {
                    continue;
                }
                if !declared.contains(fp_trim) {
                    let tag = if is_test_only_package(fp_trim) {
                        "test_depend"
                    } else {
                        "depend"
                    };
                    let severity = if is_system_vendor_package(fp_trim) {
                        Severity::Warn
                    } else {
                        Severity::Error
                    };
                    findings.push(
                        Finding::new(
                            "dep/find_package_missing",
                            severity,
                            format!("find_package({}) in CMake but not in package.xml", fp_trim),
                        )
                        .with_location(Location {
                            package: Some(pkg.name().to_string()),
                            file: Some("CMakeLists.txt".to_string()),
                            line: None,
                            context: None,
                        })
                        .with_fix_hint(format!("Add <{}>{}</{}> to package.xml", tag, fp_trim, tag)),
                    );
                }
            }

            // dep/ament_target_undeclared
            for (target, deps) in &cmake.ament_target_deps {
                for dep in deps {
                    let dep_trim = dep.trim();
                    if dep_trim.is_empty()
                        || dep_trim.starts_with('$')
                        || dep_trim.contains("${")
                    {
                        continue;
                    }
                    let project_skip = config.map(|c| c.skip_packages.as_slice());
                    if dep_should_skip_package(dep_trim, project_skip) {
                        continue;
                    }
                    if !declared.contains(dep_trim) {
                        let in_find = cmake.find_package.iter().any(|f| f.trim() == dep_trim);
                        if !in_find {
                            let tag = if is_test_only_package(dep_trim) {
                                "test_depend"
                            } else {
                                "depend"
                            };
                            let severity = if is_system_vendor_package(dep_trim) {
                                Severity::Warn
                            } else {
                                Severity::Error
                            };
                            findings.push(
                                Finding::new(
                                    "dep/ament_target_undeclared",
                                    severity,
                                    format!(
                                        "ament_target_dependencies({} ...) references '{}' not in package.xml",
                                        target, dep_trim
                                    ),
                                )
                                .with_location(Location {
                                    package: Some(pkg.name().to_string()),
                                    file: Some("CMakeLists.txt".to_string()),
                                    line: None,
                                    context: Some(format!("target: {}", target)),
                                })
                                .with_fix_hint(format!("Add <{}>{}</{}> to package.xml", tag, dep_trim, tag)),
                            );
                        }
                    }
                }
            }
        }
    }

    // dep/circular
    match toposort(&project.dependency_graph, None) {
        Ok(_) => {}
        Err(cycle) => {
            let node_idx = cycle.node_id();
            let cycle_pkg = project
                .dependency_graph
                .node_weight(node_idx)
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            findings.push(
                Finding::new(
                    "dep/circular",
                    Severity::Error,
                    format!(
                        "Circular dependency in package graph (involving '{}')",
                        cycle_pkg
                    ),
                )
                .with_location(Location {
                    package: Some(cycle_pkg),
                    file: None,
                    line: None,
                    context: None,
                })
                .with_fix_hint("Remove circular dependency between packages"),
            );
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmake::CmakePackageInfo;
    use crate::model::{Package, PackageManifest, Project};
    use petgraph::graph::DiGraph;

    #[test]
    fn find_package_missing_reported() {
        let mut manifest = PackageManifest {
            format: 3,
            name: "test_pkg".to_string(),
            version: "0.1".to_string(),
            description: Some("x".to_string()),
            maintainers: vec![],
            license: Some("Apache".to_string()),
            urls: vec![],
            authors: vec![],
            dependencies: vec![], // empty - sensor_msgs not declared
            build_types: vec![],
            member_of_groups: vec![],
            is_metapackage: false,
        };
        manifest.dependencies.push(crate::model::Dependency {
            name: "ament_cmake".to_string(),
            role: crate::model::DependencyRole::BuildTool,
        });
        let cmake = CmakePackageInfo {
            find_package: vec!["ament_cmake".to_string(), "sensor_msgs".to_string()],
            ..Default::default()
        };
        let pkg = Package {
            manifest,
            path: std::path::PathBuf::from("."),
            messages: vec![],
            services: vec![],
            actions: vec![],
            cmake_info: Some(cmake),
            setup_py_info: None,
            xacro_files: vec![],
            launch_files: vec![],
            config_files: vec![],
            python_node_files: vec![],
            cpp_node_files: vec![],
        };
        let project = Project {
            workspace_root: std::path::PathBuf::from("."),
            packages: vec![pkg],
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
        };
        let findings = check_dep(&project);
        let fp_missing: Vec<_> = findings
            .iter()
            .filter(|f| f.rule_id == "dep/find_package_missing")
            .collect();
        assert_eq!(fp_missing.len(), 1);
        assert!(fp_missing[0].message.contains("sensor_msgs"));
    }
}

