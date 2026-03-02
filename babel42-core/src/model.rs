//! ROS2 data model — types aligned with ROS2 terminology.
//!
//! Package, Node, Message, Service, Action, etc. Graph structure for
//! dependency analysis and (future) ROS graph construction.

use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};

// =============================================================================
// Package manifest (from package.xml)
// =============================================================================

/// Build system type: ament_cmake, ament_python, cmake, etc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildType {
    AmentCmake,
    AmentPython,
    Cmake,
    PythonDistutils,
    /// Unknown or custom build type
    Other(String),
}

impl BuildType {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "ament_cmake" => BuildType::AmentCmake,
            "ament_python" => BuildType::AmentPython,
            "cmake" => BuildType::Cmake,
            "python_distutils" | "python-setuptools" => BuildType::PythonDistutils,
            other => BuildType::Other(other.to_string()),
        }
    }
}

/// Dependency with role (build, exec, test, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub role: DependencyRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyRole {
    Build,
    BuildTool,
    BuildExport,
    Exec,
    Test,
    Doc,
    /// Format 3: unified depend
    Depend,
}

/// Full package manifest parsed from package.xml (REP-140/149)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub format: u32,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub maintainers: Vec<Maintainer>,
    pub license: Option<String>,
    pub urls: Vec<PackageUrl>,
    pub authors: Vec<Author>,
    pub dependencies: Vec<Dependency>,
    pub build_types: Vec<BuildType>,
    pub member_of_groups: Vec<String>,
    pub is_metapackage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Maintainer {
    pub email: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub email: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageUrl {
    pub r#type: String, // e.g. "website", "repository", "bugtracker"
    pub url: String,
}

// =============================================================================
// Interface definitions (.msg, .srv, .action)
// =============================================================================

/// A single field in a msg/srv/action definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    /// Type: primitive (int32, string) or composite (geometry_msgs/Point)
    pub field_type: String,
    /// Name of the field
    pub name: String,
    /// Array modifier: None, Some(N) for fixed, Some(0) for unbounded
    pub array_len: Option<u32>,
}

/// Message definition (.msg)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgDefinition {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

/// Service definition (.srv): request and response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrvDefinition {
    pub name: String,
    pub request: Vec<FieldDef>,
    pub response: Vec<FieldDef>,
}

/// Action definition (.action): goal, result, feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub name: String,
    pub goal: Vec<FieldDef>,
    pub result: Vec<FieldDef>,
    pub feedback: Vec<FieldDef>,
}

// =============================================================================
// Package (full in-workspace package with interfaces)
// =============================================================================

/// Parsed xacro file with path (relative to package).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XacroFile {
    /// Path relative to package root (e.g. urdf/arduinobot.urdf.xacro)
    pub path: std::path::PathBuf,
    pub info: crate::xacro::XacroFileInfo,
}

/// Parsed launch file with path (relative to package).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchFile {
    /// Path relative to package root (e.g. launch/real_robot.launch.py)
    pub path: std::path::PathBuf,
    pub info: crate::launch::LaunchFileInfo,
}

/// Parsed YAML config file (config/*.yaml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    /// Path relative to package root (e.g. config/params.yaml)
    pub path: std::path::PathBuf,
    /// Parsed YAML content, None if parse failed
    pub content: Option<serde_yaml::Value>,
}

/// A ROS2 package in the workspace: manifest + discovered interfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub manifest: PackageManifest,
    pub path: std::path::PathBuf,
    pub messages: Vec<MsgDefinition>,
    pub services: Vec<SrvDefinition>,
    pub actions: Vec<ActionDefinition>,
    /// Parsed from CMakeLists.txt (ament_cmake packages only)
    pub cmake_info: Option<crate::cmake::CmakePackageInfo>,
    /// Parsed from setup.py (ament_python packages only)
    pub setup_py_info: Option<crate::setup_py::SetupPyInfo>,
    /// Parsed xacro/URDF.xacro files
    pub xacro_files: Vec<XacroFile>,
    /// Parsed .launch.py files
    pub launch_files: Vec<LaunchFile>,
    /// Parsed config/*.yaml files
    pub config_files: Vec<ConfigFile>,
    /// Python node files with pub/sub/service interfaces
    pub python_node_files: Vec<crate::python_nodes::PythonNodeFile>,
    /// C++ node files with pub/sub/service interfaces
    pub cpp_node_files: Vec<crate::cpp_nodes::CppNodeFile>,
}

impl Package {
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn exec_deps(&self) -> impl Iterator<Item = &str> {
        self.manifest.dependencies.iter().filter_map(|d| {
            if matches!(d.role, DependencyRole::Exec | DependencyRole::Depend) {
                Some(d.name.as_str())
            } else {
                None
            }
        })
    }

    pub fn build_deps(&self) -> impl Iterator<Item = &str> {
        self.manifest.dependencies.iter().filter_map(|d| {
            if matches!(
                d.role,
                DependencyRole::Build | DependencyRole::BuildTool | DependencyRole::Depend
            ) {
                Some(d.name.as_str())
            } else {
                None
            }
        })
    }
}

// =============================================================================
// Project / workspace with graph
// =============================================================================

/// Node in the package dependency graph
#[derive(Debug, Clone)]
pub enum GraphNode {
    Package { name: String, index: usize },
}

/// Edge in the package dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub role: DependencyRole,
}

/// Reference to a launch file: (package, relative path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaunchRef {
    pub package: String,
    pub file: String,
}

/// Graph of launch file includes: (from_launch) -> (to_launch).
#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchIncludeGraph(pub DiGraph<LaunchRef, ()>);

/// ROS2 runtime node (from launch or inferred).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeNode {
    pub package: String,
    pub executable: String,
    pub name: Option<String>,
}

/// ROS2 runtime topic.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeTopic {
    pub name: String,
    pub msg_type: String,
}

/// ROS2 runtime service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeService {
    pub name: String,
    pub srv_type: String,
}

/// ROS2 runtime graph: nodes, topics, services, and their connections.
#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeGraph {
    pub nodes: Vec<RuntimeNode>,
    pub topics: Vec<RuntimeTopic>,
    pub services: Vec<RuntimeService>,
    /// (node_index, topic_index) for publishers
    pub topic_publishers: Vec<(usize, usize)>,
    /// (node_index, topic_index) for subscribers
    pub topic_subscribers: Vec<(usize, usize)>,
    /// (node_index, service_index) for service servers
    pub service_servers: Vec<(usize, usize)>,
    /// (node_index, service_index) for service clients
    pub service_clients: Vec<(usize, usize)>,
}

/// ROS2 project model: workspace with packages and dependency graph
#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub workspace_root: std::path::PathBuf,
    pub packages: Vec<Package>,
    /// Package name -> index in packages
    pub package_index: std::collections::HashMap<String, usize>,
    /// Dependency graph: packages as nodes, dependencies as edges
    pub dependency_graph: DiGraph<String, DependencyEdge>,
    /// Graph of launch file includes
    pub launch_include_graph: LaunchIncludeGraph,
    /// Runtime graph: nodes, topics, services (built from launch + Python/C++ interfaces)
    pub runtime_graph: RuntimeGraph,
}

impl Project {
    /// Look up package by name
    pub fn get_package(&self, name: &str) -> Option<&Package> {
        self.package_index.get(name).map(|&i| &self.packages[i])
    }

    /// Topological order of packages (build order)
    pub fn topological_order(&self) -> Vec<&str> {
        match petgraph::algo::toposort(&self.dependency_graph, None) {
            Ok(order) => order
                .into_iter()
                .filter_map(|idx| self.dependency_graph.node_weight(idx))
                .map(|s| s.as_str())
                .collect(),
            Err(_) => vec![],
        }
    }
}
