//! Launch rules — include cycles, missing packages.

use crate::checks::model::{Finding, Severity};
use crate::model::Project;
use petgraph::algo::is_cyclic_directed;
use std::collections::BTreeSet;

pub fn check_launch(project: &Project) -> Vec<Finding> {
    let mut findings = Vec::new();

    if is_cyclic_directed(&project.launch_include_graph.0) {
        findings.push(
            Finding::new(
                "launch/include_cycle",
                Severity::Error,
                "Include cycle in launch file graph (A includes B, B includes A)",
            )
            .with_fix_hint("Remove circular includes between launch files"),
        );
    }

    let workspace_packages: BTreeSet<&str> =
        project.package_index.keys().map(|s| s.as_str()).collect();

    for pkg in &project.packages {
        for lf in &pkg.launch_files {
            for (inc_pkg, inc_file) in &lf.info.included_launches {
                if !workspace_packages.contains(inc_pkg.as_str()) {
                    findings.push(
                        Finding::new(
                            "launch/missing_package",
                            Severity::Warn,
                            format!(
                                "Included package '{}' (in {}) not in workspace",
                                inc_pkg, inc_file
                            ),
                        )
                        .with_location(crate::checks::model::Location {
                            package: Some(pkg.name().to_string()),
                            file: Some(lf.path.to_string_lossy().to_string()),
                            line: None,
                            context: Some(format!("{}:{}", inc_pkg, inc_file)),
                        })
                        .with_fix_hint(format!(
                            "Add {} to workspace or ensure it is installed",
                            inc_pkg
                        )),
                    );
                }
            }
        }
    }

    findings
}
