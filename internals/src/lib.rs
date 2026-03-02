//! Core ROS2 project analysis: workspace discovery, package.xml parsing, interface definitions.

mod checks;
mod cmake;
mod config;
mod cpp_nodes;
mod interfaces;
mod launch;
mod model;
mod package_xml;
mod project;
mod project_config;
mod python_nodes;
mod setup_py;
mod skip_config;
mod workspace;
mod xacro;

pub use checks::{
    findings_to_sarif, run_checks, CheckOpts, FailOn, Finding, Location, RuleSet, Severity,
};
pub use cmake::{parse_cmake_lists, CmakeError, CmakePackageInfo};
pub use config::scan_config_files;
pub use cpp_nodes::{scan_cpp_nodes, CppNodeFile, CppNodeInterface, CppNodeInterfaceKind};
pub use interfaces::{parse_action_file, parse_msg_file, parse_srv_file, InterfaceError};
pub use launch::{parse_launch_file, LaunchError, LaunchFileInfo, LaunchNode};
pub use model::*;
pub use package_xml::{parse_package_xml, PackageXmlError};
pub use project::{build_project, ProjectError};
pub use project_config::{load_project_config, Babel42Config, WorkspaceConfig};
pub use python_nodes::{scan_python_nodes, PyNodeInterface, PyNodeInterfaceKind, PythonNodeFile};
pub use setup_py::{parse_setup_py, SetupPyError, SetupPyInfo};
pub use workspace::{discover_workspace, discover_workspace_with_config, Workspace};
pub use xacro::{parse_xacro_file, XacroError, XacroFileInfo};
