//! CMakeLists.txt parser using Pest.
//!
//! Extracts ROS2/ament-relevant commands: find_package, rosidl_generate_interfaces,
//! ament_target_dependencies, install(DIRECTORY), etc.

use crate::skip_config::{
    implicit_components_for, is_find_package_option, is_implicit_component_package,
    is_scope_keyword,
};
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Parser)]
#[grammar = "src/cmake/grammar.pest"]
struct CmakeParser;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CmakePackageInfo {
    /// Packages from find_package(xxx REQUIRED)
    pub find_package: Vec<String>,
    /// Interface files from rosidl_generate_interfaces(${PROJECT_NAME} "srv/X.srv" ...)
    pub rosidl_interfaces: Vec<String>,
    /// (target, deps) from ament_target_dependencies(target dep1 dep2)
    pub ament_target_deps: Vec<(String, Vec<String>)>,
    /// Directories from install(DIRECTORY x y DESTINATION ...)
    pub install_directories: Vec<String>,
    /// (executable_name, source_files) from add_executable(name src/a.cpp src/b.cpp)
    pub add_executables: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Error)]
pub enum CmakeError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] pest::error::Error<Rule>),
}

/// Parse CMakeLists.txt and extract ROS2-relevant information.
pub fn parse_cmake_lists(path: &Path) -> Result<CmakePackageInfo, CmakeError> {
    let content = fs::read_to_string(path)?;
    let content = content.trim_end();
    // Pest SOI/EOI expect newline at end for some edge cases; ensure we have it
    let content = if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{}\n", content)
    };
    parse_cmake_lists_str(&content)
}

pub fn parse_cmake_lists_str(content: &str) -> Result<CmakePackageInfo, CmakeError> {
    let pairs = CmakeParser::parse(Rule::file, content)?;
    let mut info = CmakePackageInfo::default();
    for pair in pairs {
        if pair.as_rule() == Rule::file {
            for inner in pair.into_inner() {
                process_element(&inner, &mut info);
            }
        }
    }
    Ok(info)
}

fn process_element(pair: &Pair<Rule>, info: &mut CmakePackageInfo) {
    match pair.as_rule() {
        Rule::command => process_command(pair, info),
        Rule::element => {
            for inner in pair.clone().into_inner() {
                process_element(&inner, info);
            }
        }
        _ => {}
    }
}

fn process_command(pair: &Pair<Rule>, info: &mut CmakePackageInfo) {
    let mut inner = pair.clone().into_inner();
    let name = inner
        .find(|p| p.as_rule() == Rule::identifier)
        .map(|p| p.as_str().to_owned())
        .unwrap_or_default();
    let name_lower = name.to_lowercase();
    let args = inner
        .find(|p| p.as_rule() == Rule::arguments)
        .map(collect_arguments)
        .unwrap_or_default();
    match name_lower.as_str() {
        "find_package" => extract_find_package(&args, info),
        "rosidl_generate_interfaces" => extract_rosidl_interfaces(&args, info),
        "ament_target_dependencies" => extract_ament_target_dependencies(&args, info),
        "install" => extract_install_directories(&args, info),
        "add_executable" => extract_add_executable(&args, info),
        _ => {}
    }
}

fn collect_arguments(pair: Pair<Rule>) -> Vec<String> {
    let mut args = Vec::new();
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::arg_or_nested {
            for p in inner.into_inner() {
                match p.as_rule() {
                    Rule::argument => {
                        let s = p.as_str().trim();
                        if !s.is_empty() {
                            args.push(unquote(s));
                        }
                    }
                    Rule::arguments => args.extend(collect_arguments(p)),
                    _ => {}
                }
            }
        }
    }
    args
}

fn unquote(s: &str) -> String {
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
    } else {
        s.to_string()
    }
}

/// Returns true if the argument looks like a CMake version spec (e.g. "2.87", "1.9.7...<1.10.0").
fn looks_like_version(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    s.chars().next().map_or(false, |c| c.is_ascii_digit())
        || s.contains("...")
        || (s.contains('<') || s.contains('>'))
}

fn extract_find_package(args: &[String], info: &mut CmakePackageInfo) {
    let mut skip_rest = false;
    let mut implicit_component_mode: Option<&[&str]> = None;
    for arg in args {
        let lower = arg.to_lowercase();
        if lower == "components" || lower == "optional_components" {
            skip_rest = true;
            implicit_component_mode = None;
            continue;
        }
        if skip_rest {
            continue;
        }
        if looks_like_version(arg) {
            continue;
        }
        if is_find_package_option(&lower) {
            continue;
        }
        if is_implicit_component_package(&lower) {
            implicit_component_mode = implicit_components_for(&lower);
            info.find_package.push(arg.clone());
            continue;
        }
        if let Some(comps) = implicit_component_mode {
            if comps.contains(&lower.as_str()) {
                continue;
            }
        }
        if arg.starts_with('$') {
            continue; // CMake variable, skip without resetting implicit component mode
        }
        implicit_component_mode = None;
        if !arg.is_empty() {
            info.find_package.push(arg.clone());
        }
    }
}

fn extract_rosidl_interfaces(args: &[String], info: &mut CmakePackageInfo) {
    for a in args.iter().skip(1) {
        // First arg is often ${PROJECT_NAME}; rest are "srv/X.srv", "action/Y.action"
        if a.ends_with(".msg") || a.ends_with(".srv") || a.ends_with(".action") {
            let path = Path::new(a);
            info.rosidl_interfaces.push(
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(a)
                    .to_string(),
            );
        }
    }
}

fn extract_ament_target_dependencies(args: &[String], info: &mut CmakePackageInfo) {
    let target = args.first().cloned().unwrap_or_default();
    let deps: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|s| !s.is_empty() && !is_scope_keyword(s))
        .cloned()
        .collect();
    if !target.is_empty() && !deps.is_empty() {
        info.ament_target_deps.push((target, deps));
    }
}

fn extract_add_executable(args: &[String], info: &mut CmakePackageInfo) {
    let target = args.first().cloned().unwrap_or_default();
    if target.is_empty() {
        return;
    }
    let sources: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|s| {
            !s.is_empty()
                && (s.ends_with(".cpp")
                    || s.ends_with(".cxx")
                    || s.ends_with(".cc")
                    || s.ends_with(".c"))
        })
        .cloned()
        .collect();
    if !sources.is_empty() {
        info.add_executables.push((target, sources));
    }
}

fn extract_install_directories(args: &[String], info: &mut CmakePackageInfo) {
    let mut i = 0;
    while i < args.len() {
        if args[i].to_uppercase() == "DIRECTORY" {
            i += 1;
            while i < args.len() && args[i].to_uppercase() != "DESTINATION" {
                if !args[i].is_empty() {
                    info.install_directories.push(args[i].clone());
                }
                i += 1;
            }
            break;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_find_package() {
        let s = r#"find_package(ament_cmake REQUIRED)
find_package(std_msgs REQUIRED)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.find_package, ["ament_cmake", "std_msgs"]);
    }

    #[test]
    fn parse_find_package_components_skipped() {
        let s = r#"find_package(OpenCV REQUIRED COMPONENTS core highgui imgcodecs imgproc videoio)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.find_package, ["OpenCV"]);
    }

    #[test]
    fn parse_rosidl_generate_interfaces() {
        let s = r#"rosidl_generate_interfaces(${PROJECT_NAME}
  "srv/AddTwoInts.srv"
  "srv/EulerToQuaternion.srv"
  "action/Fibonacci.action"
)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert!(info
            .rosidl_interfaces
            .contains(&"AddTwoInts.srv".to_string()));
        assert!(info
            .rosidl_interfaces
            .contains(&"EulerToQuaternion.srv".to_string()));
        assert!(info
            .rosidl_interfaces
            .contains(&"Fibonacci.action".to_string()));
    }

    #[test]
    fn parse_ament_target_dependencies() {
        let s = r#"ament_target_dependencies(slider_control rclcpp trajectory_msgs sensor_msgs)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.ament_target_deps.len(), 1);
        assert_eq!(info.ament_target_deps[0].0, "slider_control");
        assert!(info.ament_target_deps[0].1.contains(&"rclcpp".to_string()));
    }

    #[test]
    fn parse_install_directories() {
        let s = r#"install(
  DIRECTORY launch config
  DESTINATION share/${PROJECT_NAME}
)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert!(info.install_directories.contains(&"launch".to_string()));
        assert!(info.install_directories.contains(&"config".to_string()));
    }

    #[test]
    fn parse_find_package_skips_version() {
        let s = r#"find_package(Bullet 2.87 REQUIRED)
find_package(octomap 1.9.7...<1.10.0 REQUIRED)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.find_package, ["Bullet", "octomap"]);
        assert!(!info.find_package.contains(&"2.87".to_string()));
        assert!(!info.find_package.contains(&"1.9.7...<1.10.0".to_string()));
    }

    #[test]
    fn parse_find_package_boost_components_without_keyword() {
        let s = r#"find_package(
  Boost
  REQUIRED
  system
  filesystem
  date_time
  thread
  serialization)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.find_package, ["Boost"]);
        assert!(!info.find_package.contains(&"system".to_string()));
    }

    #[test]
    fn parse_find_package_qt5_without_components_keyword() {
        let s = r#"find_package(Qt5 ${QT_VERSION} REQUIRED Core Widgets)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.find_package.len(), 1);
        assert!(info.find_package[0].to_lowercase().starts_with("qt5"));
        assert!(!info.find_package.contains(&"Core".to_string()));
        assert!(!info.find_package.contains(&"Widgets".to_string()));
    }

    #[test]
    fn parse_find_package_skips_no_cmake_package_registry() {
        let s = r#"find_package(TBB REQUIRED NO_CMAKE_PACKAGE_REGISTRY)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.find_package, ["TBB"]);
        assert!(!info
            .find_package
            .contains(&"NO_CMAKE_PACKAGE_REGISTRY".to_string()));
    }

    #[test]
    fn parse_ament_target_dependencies_skips_scope() {
        let s = r#"ament_target_dependencies(moveit_ros_occupancy_map_server PUBLIC
                          ${THIS_PACKAGE_INCLUDE_DEPENDS})
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.ament_target_deps.len(), 1);
        assert!(!info.ament_target_deps[0].1.contains(&"PUBLIC".to_string()));
    }

    #[test]
    fn parse_add_executable() {
        let s = r#"add_executable(slider_control src/slider_control.cpp)
ament_target_dependencies(slider_control rclcpp trajectory_msgs sensor_msgs)
"#;
        let info = parse_cmake_lists_str(s).unwrap();
        assert_eq!(info.add_executables.len(), 1);
        assert_eq!(info.add_executables[0].0, "slider_control");
        assert_eq!(info.add_executables[0].1, ["src/slider_control.cpp"]);
    }
}
