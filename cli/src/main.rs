//! Babel42 — ROS2 project analysis CLI (open source edition).

use std::path::PathBuf;

use internals::{
    build_project, discover_workspace_with_config, findings_to_sarif, load_project_config,
    run_checks, CheckOpts, FailOn, RuleSet, Severity,
};
use clap::Parser;

#[derive(Parser)]
#[command(name = "babel42")]
#[command(about = "ROS2 project analysis tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Analyze a ROS2 workspace
    Analyze {
        /// Path to the workspace root (default: current directory)
        path: Option<PathBuf>,
    },
    /// Export project model to JSON
    Export {
        /// Path to the workspace root (default: current directory)
        path: Option<PathBuf>,
        /// Output format (default: json)
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Check workspace for potential problems (CI-friendly)
    Check {
        /// Path to the workspace root (default: current directory)
        path: Option<PathBuf>,
        /// Output format: human, json, or sarif (for GitHub Code Scanning)
        #[arg(long, default_value = "human")]
        format: String,
        /// Minimum severity to show (info, warn, error)
        #[arg(long, default_value = "warn")]
        severity: String,
        /// Rule sets to run (all, dep, launch, runtime, manifest)
        #[arg(long, default_value = "all")]
        rules: String,
        /// Exit with code 1 when findings >= this severity (error, warn, info, none)
        #[arg(long, default_value = "error")]
        fail_on: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Analyze { path } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            run_analyze(&root)?;
        }
        Command::Export { path, format } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            run_export(&root, &format)?;
        }
        Command::Check {
            path,
            format,
            severity,
            rules,
            fail_on,
        } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            let exit = run_check(&root, &format, &severity, &rules, &fail_on)?;
            std::process::exit(exit);
        }
    }

    Ok(())
}

fn run_analyze(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_project_config(root);
    let workspace = match discover_workspace_with_config(root, Some(&config.workspace)) {
        Some(w) => w,
        None => {
            eprintln!("No ROS2 workspace found at {}", root.display());
            return Ok(());
        }
    };

    let project = match build_project(&workspace) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to build project: {}", e);
            return Ok(());
        }
    };

    println!("Workspace: {}", project.workspace_root.display());
    println!("Packages: {}", project.packages.len());
    println!();

    for pkg in &project.packages {
        println!("  - {} ({}):", pkg.name(), pkg.manifest.version);
        if !pkg.messages.is_empty() {
            println!(
                "      msgs:   {}",
                pkg.messages.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if !pkg.services.is_empty() {
            println!(
                "      srv:    {}",
                pkg.services.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if !pkg.actions.is_empty() {
            println!(
                "      action: {}",
                pkg.actions.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if let Some(ref cmake) = pkg.cmake_info {
            if !cmake.find_package.is_empty() {
                println!("      find_package: {}", cmake.find_package.join(", "));
            }
        }
        if !pkg.xacro_files.is_empty() {
            for xf in &pkg.xacro_files {
                println!("      xacro: {} (includes: {}, packages: {})",
                    xf.path.display(),
                    xf.info.includes.len(),
                    xf.info.package_refs.join(", "));
            }
        }
        if !pkg.launch_files.is_empty() {
            for lf in &pkg.launch_files {
                let includes: String = lf.info.included_launches
                    .iter()
                    .map(|(p, f)| format!("{}:{}", p, f))
                    .collect::<Vec<_>>()
                    .join(", ");
                let nodes: String = lf.info.nodes
                    .iter()
                    .map(|n| format!("{}/{}", n.package, n.executable))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("      launch: {} (includes: [{}], nodes: [{}])",
                    lf.path.display(), includes, nodes);
            }
        }
    }

    println!();
    println!("Build order: {}", project.topological_order().join(" -> "));

    Ok(())
}

fn run_export(root: &std::path::Path, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_project_config(root);
    let workspace = match discover_workspace_with_config(root, Some(&config.workspace)) {
        Some(w) => w,
        None => {
            eprintln!("No ROS2 workspace found at {}", root.display());
            return Ok(());
        }
    };

    let project = match build_project(&workspace) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to build project: {}", e);
            return Ok(());
        }
    };

    match format.to_lowercase().as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&project)?;
            println!("{}", json);
        }
        _ => {
            eprintln!("Unsupported format: {} (use: json)", format);
        }
    }

    Ok(())
}

fn run_check(
    root: &std::path::Path,
    format: &str,
    severity: &str,
    rules: &str,
    fail_on: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    let config = load_project_config(root);
    let workspace = match discover_workspace_with_config(root, Some(&config.workspace)) {
        Some(w) => w,
        None => {
            eprintln!("No ROS2 workspace found at {}", root.display());
            return Ok(2);
        }
    };

    let project = match build_project(&workspace) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to build project: {}", e);
            return Ok(2);
        }
    };

    let min_severity = match severity.to_lowercase().as_str() {
        "info" => Severity::Info,
        "warn" => Severity::Warn,
        "error" => Severity::Error,
        _ => Severity::Warn,
    };

    let rule_set = rules.parse().unwrap_or(RuleSet::All);
    let fail_on_parsed = fail_on.parse().unwrap_or(FailOn::Error);

    let opts = CheckOpts {
        min_severity,
        rule_set,
        fail_on: fail_on_parsed,
        config: Some(config),
    };

    let findings = run_checks(&project, &opts);

    let has_error = findings.iter().any(|f| f.severity == Severity::Error);
    let has_warn = findings.iter().any(|f| f.severity == Severity::Warn);
    let has_info = findings.iter().any(|f| f.severity == Severity::Info);

    match format.to_lowercase().as_str() {
        "json" => {
            let summary = serde_json::json!({
                "error": findings.iter().filter(|f| f.severity == Severity::Error).count(),
                "warn": findings.iter().filter(|f| f.severity == Severity::Warn).count(),
                "info": findings.iter().filter(|f| f.severity == Severity::Info).count(),
            });
            let output = serde_json::json!({
                "findings": findings,
                "summary": summary
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        "sarif" => {
            let sarif = findings_to_sarif(&findings, &project);
            println!("{}", serde_json::to_string_pretty(&sarif)?);
        }
        _ => {
            for f in &findings {
                let sev = match f.severity {
                    Severity::Info => "info",
                    Severity::Warn => "warn",
                    Severity::Error => "error",
                };
                print!("{} {}  {}", sev, f.rule_id, f.message);
                if let Some(ref loc) = f.location {
                    if let Some(ref pkg) = loc.package {
                        print!(" [{}]", pkg);
                    }
                }
                println!();
                if let Some(ref hint) = f.fix_hint {
                    println!("  Fix: {}", hint);
                }
            }
            let e = findings.iter().filter(|f| f.severity == Severity::Error).count();
            let w = findings.iter().filter(|f| f.severity == Severity::Warn).count();
            let i = findings.iter().filter(|f| f.severity == Severity::Info).count();
            if !findings.is_empty() {
                println!();
                println!("{} findings ({} error, {} warn, {} info)", findings.len(), e, w, i);
            }
        }
    }

    let exit = if fail_on_parsed.should_fail(has_error, has_warn, has_info) {
        1
    } else {
        0
    };

    Ok(exit)
}
