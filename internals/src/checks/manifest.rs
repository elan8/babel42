//! Manifest rules — missing description, no maintainer.

use crate::checks::model::{Finding, Location, Severity};
use crate::model::Project;

pub fn check_manifest(project: &Project) -> Vec<Finding> {
    let mut findings = Vec::new();

    for pkg in &project.packages {
        if pkg.manifest.description.as_ref().map_or(true, |d| d.trim().is_empty()) {
            findings.push(
                Finding::new(
                    "manifest/missing_description",
                    Severity::Warn,
                    format!("Package '{}' has no description", pkg.name()),
                )
                .with_location(Location {
                    package: Some(pkg.name().to_string()),
                    file: Some("package.xml".to_string()),
                    line: None,
                    context: None,
                })
                .with_fix_hint("Add <description>...</description> to package.xml"),
            );
        }

        if pkg.manifest.maintainers.is_empty() {
            findings.push(
                Finding::new(
                    "manifest/no_maintainer",
                    Severity::Warn,
                    format!("Package '{}' has no maintainers", pkg.name()),
                )
                .with_location(Location {
                    package: Some(pkg.name().to_string()),
                    file: Some("package.xml".to_string()),
                    line: None,
                    context: None,
                })
                .with_fix_hint("Add <maintainer ...>...</maintainer> to package.xml"),
            );
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Package, PackageManifest, Project};
    use petgraph::graph::DiGraph;

    #[test]
    fn missing_description_reported() {
        let manifest = PackageManifest {
            format: 3,
            name: "no_desc".to_string(),
            version: "0.1".to_string(),
            description: None,
            maintainers: vec![crate::model::Maintainer {
                email: Some("a@b.c".to_string()),
                name: "Alice".to_string(),
            }],
            license: Some("Apache".to_string()),
            urls: vec![],
            authors: vec![],
            dependencies: vec![],
            build_types: vec![],
            member_of_groups: vec![],
            is_metapackage: false,
        };
        let pkg = Package {
            manifest,
            path: std::path::PathBuf::from("."),
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
        };
        let project = Project {
            workspace_root: std::path::PathBuf::from("."),
            packages: vec![pkg],
            package_index: [("no_desc".to_string(), 0)].into_iter().collect(),
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
        let findings = check_manifest(&project);
        let desc: Vec<_> = findings
            .iter()
            .filter(|f| f.rule_id == "manifest/missing_description")
            .collect();
        assert_eq!(desc.len(), 1);
    }
}
