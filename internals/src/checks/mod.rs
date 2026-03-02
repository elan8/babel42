//! Static checks for ROS2 workspaces — dependency, launch, runtime, manifest rules.

mod dep;
mod launch;
mod manifest;
mod model;
mod runtime;
mod sarif;

pub use model::{CheckOpts, FailOn, Finding, Location, RuleSet, Severity};
pub use sarif::findings_to_sarif;

use crate::model::Project;

/// Run all enabled checks and return findings (filtered by min_severity).
pub fn run_checks(project: &Project, opts: &CheckOpts) -> Vec<Finding> {
    let mut findings = Vec::new();

    if opts.rule_set.includes_dep() {
        findings.extend(dep::check_dep_with_config(project, opts.config.as_ref()));
    }
    if opts.rule_set.includes_launch() {
        findings.extend(launch::check_launch(project));
    }
    if opts.rule_set.includes_runtime() {
        findings.extend(runtime::check_runtime(project));
    }
    if opts.rule_set.includes_manifest() {
        findings.extend(manifest::check_manifest(project));
    }

    findings
        .into_iter()
        .filter(|f| f.severity >= opts.min_severity)
        .collect()
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::project::build_project;
    use crate::workspace::discover_workspace;
    use std::path::PathBuf;

    #[derive(serde::Serialize)]
    struct RepoSummary {
        name: String,
        errors: usize,
        warnings: usize,
        info: usize,
    }

    #[derive(serde::Serialize)]
    struct RepoFindings {
        name: String,
        findings: Vec<Finding>,
    }

    #[derive(serde::Serialize)]
    struct IntegrationReport {
        generated_at: String,
        summary: Vec<RepoSummary>,
        total_errors: usize,
        total_warnings: usize,
        total_info: usize,
        repos: Vec<RepoFindings>,
        skipped: Vec<String>,
    }

    fn fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("fixtures")
    }

    /// All fixture repos: moveit2_robot_arm + submodules at fixtures root
    fn all_fixture_paths() -> Vec<(&'static str, PathBuf)> {
        let root = fixtures_root();
        let mut paths = Vec::new();
        paths.push(("moveit2_robot_arm", root.join("moveit2_robot_arm")));
        for name in [
            "examples",
            "demos",
            "tutorials",
            "Universal_Robots_ROS2_Driver",
            "ament_cmake",
            "navigation2",
            "moveit2",
            "ros2_control",
            "slam_toolbox",
        ] {
            paths.push((name, root.join(name)));
        }
        paths
    }

    fn count_by_severity(findings: &[Finding]) -> (usize, usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        let mut info = 0;
        for f in findings {
            match f.severity {
                Severity::Error => errors += 1,
                Severity::Warn => warnings += 1,
                Severity::Info => info += 1,
            }
        }
        (errors, warnings, info)
    }

    fn format_finding(f: &Finding) -> String {
        let sev = match f.severity {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
        };
        let mut s = format!("{} {}  {}", sev, f.rule_id, f.message);
        if let Some(ref loc) = f.location {
            if let Some(ref pkg) = loc.package {
                s.push_str(&format!(" [{}]", pkg));
            }
        }
        if let Some(ref hint) = f.fix_hint {
            s.push_str(&format!("\n  Fix: {}", hint));
        }
        s
    }

    /// Full integration test: discover + build + check on all fixture repos.
    /// Run `git submodule update --init --recursive` to populate fixtures.
    #[test]
    fn integration_all_fixtures_analyze_and_check() {
        let mut ran = 0;
        let mut skipped = Vec::new();
        let mut summaries: Vec<(&'static str, usize, usize, usize)> = Vec::new();
        let mut all_findings: Vec<(&'static str, Vec<Finding>)> = Vec::new();

        for (name, path) in all_fixture_paths() {
            if !path.exists() {
                skipped.push(name);
                continue;
            }

            let workspace = discover_workspace(&path).unwrap_or_else(|| {
                panic!(
                    "{}: failed to discover workspace at {}",
                    name,
                    path.display()
                )
            });

            let project = build_project(&workspace)
                .unwrap_or_else(|e| panic!("{}: build_project failed: {}", name, e));

            let opts = CheckOpts::default();
            let findings = run_checks(&project, &opts);

            let (errors, warnings, info) = count_by_severity(&findings);
            summaries.push((name, errors, warnings, info));
            all_findings.push((name, findings.clone()));

            assert!(
                findings.iter().all(|f| f.severity >= opts.min_severity),
                "{}: findings should meet min_severity (warn)",
                name
            );

            ran += 1;
        }

        assert!(
            ran > 0,
            "No fixtures found. Run: scripts/fetch_fixtures.ps1 or scripts/fetch_fixtures.sh"
        );

        // Print per-repo summary
        eprintln!("\n--- Integration test summary (errors / warnings / info) ---");
        for (name, errors, warnings, info) in &summaries {
            eprintln!(
                "  {:30} {} errors, {} warnings, {} info",
                name, errors, warnings, info
            );
        }
        let total_e = summaries.iter().map(|(_, e, _, _)| e).sum::<usize>();
        let total_w = summaries.iter().map(|(_, _, w, _)| w).sum::<usize>();
        let total_i = summaries.iter().map(|(_, _, _, i)| i).sum::<usize>();
        eprintln!(
            "  {:30} {} errors, {} warnings, {} info",
            "TOTAL", total_e, total_w, total_i
        );
        eprintln!("---------------------------------------------------------\n");

        // Print all specific findings per repo
        eprintln!("--- All findings (for false positive review) ---");
        for (name, findings) in &all_findings {
            if findings.is_empty() {
                continue;
            }
            eprintln!("\n## {}", name);
            for f in findings {
                eprintln!("  {}", format_finding(f));
            }
        }
        eprintln!("\n---------------------------------------------------------\n");

        if !skipped.is_empty() {
            eprintln!(
                "Skipped (not fetched): {}. Run: scripts/fetch_fixtures.ps1 or fetch_fixtures.sh",
                skipped.join(", ")
            );
        }

        // Write JSON report when BABEL42_JSON_OUTPUT is set (e.g. in CI)
        if let Ok(path) = std::env::var("BABEL42_JSON_OUTPUT") {
            let total_e = summaries.iter().map(|(_, e, _, _)| e).sum::<usize>();
            let total_w = summaries.iter().map(|(_, _, w, _)| w).sum::<usize>();
            let total_i = summaries.iter().map(|(_, _, _, i)| i).sum::<usize>();
            let report = IntegrationReport {
                generated_at: chrono::Utc::now().to_rfc3339(),
                summary: summaries
                    .iter()
                    .map(|(n, e, w, i)| RepoSummary {
                        name: (*n).to_string(),
                        errors: *e,
                        warnings: *w,
                        info: *i,
                    })
                    .collect(),
                total_errors: total_e,
                total_warnings: total_w,
                total_info: total_i,
                repos: all_findings
                    .iter()
                    .map(|(n, f)| RepoFindings {
                        name: (*n).to_string(),
                        findings: f.clone(),
                    })
                    .collect(),
                skipped: skipped.iter().map(|s| (*s).to_string()).collect(),
            };
            let json = serde_json::to_string_pretty(&report).expect("serialize report");
            std::fs::write(&path, json).unwrap_or_else(|e| panic!("write {}: {}", path, e));
        }
    }

    #[test]
    fn check_moveit2_fixture_runs() {
        let fixture = fixtures_root().join("moveit2_robot_arm");
        if !fixture.exists() {
            return;
        }
        let workspace = discover_workspace(&fixture).expect("discover");
        let project = build_project(&workspace).expect("build");
        let opts = CheckOpts::default();
        let findings = run_checks(&project, &opts);
        assert!(
            findings.iter().all(|f| f.severity >= opts.min_severity),
            "all findings should meet min_severity"
        );
    }

    #[test]
    fn check_ros2_github_fixtures_run() {
        for (name, path) in all_fixture_paths() {
            if name == "moveit2_robot_arm" {
                continue;
            }
            if !path.exists() {
                continue;
            }
            let workspace = discover_workspace(&path).expect("discover");
            let project = build_project(&workspace).expect("build");
            let opts = CheckOpts::default();
            let findings = run_checks(&project, &opts);
            assert!(
                findings.iter().all(|f| f.severity >= opts.min_severity),
                "{}: all findings should meet min_severity",
                name
            );
        }
    }
}
