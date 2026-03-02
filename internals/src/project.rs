//! Build Project model from discovered workspace.

use crate::cmake::parse_cmake_lists;
use crate::config::scan_config_files;
use crate::cpp_nodes::scan_cpp_nodes;
use crate::interfaces::{parse_action_file, parse_msg_file, parse_srv_file};
use crate::launch::parse_launch_file;
use crate::model::{
    BuildType, DependencyEdge, DependencyRole, LaunchFile, LaunchIncludeGraph, LaunchRef, Package,
    Project, RuntimeGraph, RuntimeNode, RuntimeService, RuntimeTopic, XacroFile,
};
use crate::package_xml::parse_package_xml;
use crate::python_nodes::scan_python_nodes;
use crate::setup_py::parse_setup_py;
use crate::workspace::Workspace;
use crate::xacro::parse_xacro_file;
use petgraph::graph::DiGraph;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Build a full Project from a discovered workspace.
pub fn build_project(workspace: &Workspace) -> Result<Project, ProjectError> {
    let mut packages = Vec::new();
    let mut package_index = HashMap::new();
    let mut graph = DiGraph::<String, DependencyEdge>::new();
    let mut node_indices: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    // First pass: parse all packages
    for pkg_xml in &workspace.packages {
        let manifest = parse_package_xml(pkg_xml).map_err(ProjectError::PackageXml)?;
        let pkg_dir = pkg_xml.parent().unwrap_or_else(|| Path::new("."));

        let messages = scan_msgs(pkg_dir);
        let services = scan_srvs(pkg_dir);
        let actions = scan_actions(pkg_dir);
        let xacro_files = scan_xacro_files(pkg_dir);
        let launch_files = scan_launch_files(pkg_dir);
        let config_files = scan_config_files(pkg_dir);
        let python_node_files = scan_python_nodes(pkg_dir, &manifest.name);

        let cmake_info = {
            let has_ament_cmake = manifest
                .build_types
                .iter()
                .any(|b| matches!(b, BuildType::AmentCmake))
                || manifest
                    .dependencies
                    .iter()
                    .any(|d| d.role == DependencyRole::BuildTool && d.name == "ament_cmake");
            let cmake_path = pkg_dir.join("CMakeLists.txt");
            if has_ament_cmake && cmake_path.is_file() {
                parse_cmake_lists(&cmake_path).ok()
            } else {
                None
            }
        };
        let cpp_node_files = scan_cpp_nodes(pkg_dir, cmake_info.as_ref());

        let setup_py_info = {
            let has_ament_python = manifest
                .build_types
                .iter()
                .any(|b| matches!(b, BuildType::AmentPython))
                || manifest
                    .dependencies
                    .iter()
                    .any(|d| d.role == DependencyRole::BuildTool && d.name == "ament_python");
            let setup_path = pkg_dir.join("setup.py");
            if has_ament_python && setup_path.is_file() {
                parse_setup_py(&setup_path).ok()
            } else {
                None
            }
        };

        let pkg = Package {
            manifest: manifest.clone(),
            path: pkg_dir.to_path_buf(),
            messages,
            services,
            actions,
            cmake_info,
            setup_py_info,
            xacro_files,
            launch_files,
            config_files,
            python_node_files,
            cpp_node_files,
        };

        let idx = packages.len();
        package_index.insert(manifest.name.clone(), idx);
        packages.push(pkg);

        let node_idx = graph.add_node(manifest.name.clone());
        node_indices.insert(manifest.name.clone(), node_idx);
    }

    // Second pass: add dependency edges (only for packages in workspace)
    for pkg in &packages {
        let from_idx = match node_indices.get(pkg.name()) {
            Some(&i) => i,
            None => continue,
        };

        for dep in pkg.manifest.dependencies.iter() {
            if let Some(&to_idx) = node_indices.get(&dep.name) {
                graph.add_edge(from_idx, to_idx, DependencyEdge { role: dep.role });
            }
        }
    }

    // Third pass: build launch include graph
    let launch_graph = build_launch_include_graph(&packages);

    // Fourth pass: build runtime graph
    let runtime_graph = build_runtime_graph(&packages);

    Ok(Project {
        workspace_root: workspace.root.clone(),
        packages,
        package_index,
        dependency_graph: graph,
        launch_include_graph: launch_graph,
        runtime_graph,
    })
}

fn scan_msgs(pkg_dir: &Path) -> Vec<crate::model::MsgDefinition> {
    let msg_dir = pkg_dir.join("msg");
    if !msg_dir.is_dir() {
        return vec![];
    }
    let mut msgs = Vec::new();
    for entry in WalkDir::new(&msg_dir).max_depth(1) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "msg") {
                if let Ok(m) = parse_msg_file(p) {
                    msgs.push(m);
                }
            }
        }
    }
    msgs
}

fn scan_srvs(pkg_dir: &Path) -> Vec<crate::model::SrvDefinition> {
    let srv_dir = pkg_dir.join("srv");
    if !srv_dir.is_dir() {
        return vec![];
    }
    let mut srvs = Vec::new();
    for entry in WalkDir::new(&srv_dir).max_depth(1) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "srv") {
                if let Ok(s) = parse_srv_file(p) {
                    srvs.push(s);
                }
            }
        }
    }
    srvs
}

fn scan_launch_files(pkg_dir: &Path) -> Vec<LaunchFile> {
    let mut files = Vec::new();
    for entry in WalkDir::new(pkg_dir).max_depth(4) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "py")
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map_or(false, |s| s.ends_with(".launch"))
            {
                if let Ok(info) = parse_launch_file(p) {
                    let rel = p.strip_prefix(pkg_dir).unwrap_or(p).to_path_buf();
                    files.push(LaunchFile { path: rel, info });
                }
            }
        }
    }
    files
}

fn scan_xacro_files(pkg_dir: &Path) -> Vec<XacroFile> {
    let mut files = Vec::new();
    for entry in WalkDir::new(pkg_dir).max_depth(4) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "xacro") {
                if let Ok(info) = parse_xacro_file(p) {
                    let rel = p.strip_prefix(pkg_dir).unwrap_or(p).to_path_buf();
                    files.push(XacroFile { path: rel, info });
                }
            }
        }
    }
    files
}

fn scan_actions(pkg_dir: &Path) -> Vec<crate::model::ActionDefinition> {
    let action_dir = pkg_dir.join("action");
    if !action_dir.is_dir() {
        return vec![];
    }
    let mut actions = Vec::new();
    for entry in WalkDir::new(&action_dir).max_depth(1) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "action") {
                if let Ok(a) = parse_action_file(p) {
                    actions.push(a);
                }
            }
        }
    }
    actions
}

fn build_launch_include_graph(packages: &[Package]) -> LaunchIncludeGraph {
    let mut g = petgraph::graph::DiGraph::<LaunchRef, ()>::new();
    let mut ref_to_idx: HashMap<LaunchRef, petgraph::graph::NodeIndex> = HashMap::new();

    fn ensure_node(
        g: &mut petgraph::graph::DiGraph<LaunchRef, ()>,
        ref_to_idx: &mut HashMap<LaunchRef, petgraph::graph::NodeIndex>,
        r: LaunchRef,
    ) -> petgraph::graph::NodeIndex {
        ref_to_idx
            .entry(r.clone())
            .or_insert_with(|| g.add_node(r))
            .clone()
    }

    for pkg in packages {
        for lf in &pkg.launch_files {
            let from_ref = LaunchRef {
                package: pkg.name().to_string(),
                file: lf.path.to_string_lossy().replace('\\', "/"),
            };
            let from_idx = ensure_node(&mut g, &mut ref_to_idx, from_ref);
            for (inc_pkg, inc_file) in &lf.info.included_launches {
                let to_ref = LaunchRef {
                    package: inc_pkg.clone(),
                    file: inc_file.clone(),
                };
                let to_idx = ensure_node(&mut g, &mut ref_to_idx, to_ref.clone());
                g.add_edge(from_idx, to_idx, ());
            }
        }
    }

    LaunchIncludeGraph(g)
}

/// Normalize topic name for comparison (strip leading slash).
fn normalize_topic(s: &str) -> String {
    s.trim_start_matches('/').to_string()
}

/// Known external ROS2 nodes: (package, executable) -> default (topic, msg_type) they publish.
fn known_external_publishers(package: &str, executable: &str) -> Vec<(String, String)> {
    let key = (package, executable);
    match key {
        ("joint_state_publisher_gui", "joint_state_publisher_gui") => {
            vec![(
                "joint_states".to_string(),
                "sensor_msgs/msg/JointState".to_string(),
            )]
        }
        ("joint_state_publisher", "joint_state_publisher") => {
            vec![(
                "joint_states".to_string(),
                "sensor_msgs/msg/JointState".to_string(),
            )]
        }
        _ => vec![],
    }
}

fn build_runtime_graph(packages: &[Package]) -> RuntimeGraph {
    use crate::cpp_nodes::CppNodeInterfaceKind;
    use crate::python_nodes::PyNodeInterfaceKind;
    use std::collections::{HashMap, HashSet};

    let workspace_packages: HashSet<String> =
        packages.iter().map(|p| p.name().to_string()).collect();

    let mut nodes: Vec<RuntimeNode> = Vec::new();
    let mut node_key_to_idx: HashMap<(String, String), usize> = HashMap::new();

    // Collect nodes from launch files
    for pkg in packages {
        for lf in &pkg.launch_files {
            for n in &lf.info.nodes {
                let key = (n.package.clone(), n.executable.clone());
                if !node_key_to_idx.contains_key(&key) {
                    let idx = nodes.len();
                    nodes.push(RuntimeNode {
                        package: n.package.clone(),
                        executable: n.executable.clone(),
                        name: n.name.clone(),
                    });
                    node_key_to_idx.insert(key, idx);
                }
            }
        }
    }

    // Add ament_python nodes from setup.py entry_points (if not already from launch)
    for pkg in packages {
        if let Some(ref sp) = pkg.setup_py_info {
            for exec in &sp.entry_points {
                let key = (pkg.name().to_string(), exec.clone());
                if !node_key_to_idx.contains_key(&key) {
                    let idx = nodes.len();
                    nodes.push(RuntimeNode {
                        package: pkg.name().to_string(),
                        executable: exec.clone(),
                        name: None,
                    });
                    node_key_to_idx.insert(key, idx);
                }
            }
        }
    }

    let mut topics: Vec<RuntimeTopic> = Vec::new();
    let mut topic_key_to_idx: HashMap<String, usize> = HashMap::new();
    let mut services: Vec<RuntimeService> = Vec::new();
    let mut service_key_to_idx: HashMap<String, usize> = HashMap::new();
    let mut topic_publishers: Vec<(usize, usize)> = Vec::new();
    let mut topic_subscribers: Vec<(usize, usize)> = Vec::new();
    let mut service_servers: Vec<(usize, usize)> = Vec::new();
    let mut service_clients: Vec<(usize, usize)> = Vec::new();

    fn ensure_topic(
        topics: &mut Vec<RuntimeTopic>,
        topic_key_to_idx: &mut HashMap<String, usize>,
        name: &str,
        msg_type: &str,
    ) -> usize {
        let key = normalize_topic(name);
        *topic_key_to_idx.entry(key).or_insert_with(|| {
            let idx = topics.len();
            topics.push(RuntimeTopic {
                name: name.to_string(),
                msg_type: msg_type.to_string(),
            });
            idx
        })
    }

    fn ensure_service(
        services: &mut Vec<RuntimeService>,
        service_key_to_idx: &mut HashMap<String, usize>,
        name: &str,
        srv_type: &str,
    ) -> usize {
        let key = name.to_string();
        *service_key_to_idx.entry(key).or_insert_with(|| {
            let idx = services.len();
            services.push(RuntimeService {
                name: name.to_string(),
                srv_type: srv_type.to_string(),
            });
            idx
        })
    }

    fn executable_from_path(path: &std::path::Path) -> Option<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }

    /// Path to module: "my_pkg/publisher.py" -> "my_pkg.publisher"
    fn path_to_module(path: &std::path::Path) -> String {
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches(".py")
            .replace('/', ".")
    }

    for pkg in packages {
        let pkg_name = pkg.name();

        for py_file in &pkg.python_node_files {
            let exec = match &pkg.setup_py_info {
                Some(sp) => {
                    let module = path_to_module(&py_file.path);
                    sp.entry_point_modules
                        .iter()
                        .find(|(_, m)| *m == module)
                        .map(|(e, _)| e.clone())
                }
                None => executable_from_path(&py_file.path),
            };
            let node_idx =
                exec.and_then(|e| node_key_to_idx.get(&(pkg_name.to_string(), e)).copied());
            let Some(nidx) = node_idx else { continue };

            for iface in &py_file.interfaces {
                match &iface.kind {
                    PyNodeInterfaceKind::Publisher => {
                        let tidx = ensure_topic(
                            &mut topics,
                            &mut topic_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        topic_publishers.push((nidx, tidx));
                    }
                    PyNodeInterfaceKind::Subscriber => {
                        let tidx = ensure_topic(
                            &mut topics,
                            &mut topic_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        topic_subscribers.push((nidx, tidx));
                    }
                    PyNodeInterfaceKind::Service => {
                        let sidx = ensure_service(
                            &mut services,
                            &mut service_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        service_servers.push((nidx, sidx));
                    }
                    PyNodeInterfaceKind::Client => {
                        let sidx = ensure_service(
                            &mut services,
                            &mut service_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        service_clients.push((nidx, sidx));
                    }
                    _ => {}
                }
            }
        }

        for cpp_file in &pkg.cpp_node_files {
            let exec = cpp_file
                .executable
                .as_ref()
                .cloned()
                .or_else(|| executable_from_path(&cpp_file.path));
            let node_idx =
                exec.and_then(|e| node_key_to_idx.get(&(pkg_name.to_string(), e)).copied());
            let Some(nidx) = node_idx else { continue };

            for iface in &cpp_file.interfaces {
                match &iface.kind {
                    CppNodeInterfaceKind::Publisher => {
                        let tidx = ensure_topic(
                            &mut topics,
                            &mut topic_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        topic_publishers.push((nidx, tidx));
                    }
                    CppNodeInterfaceKind::Subscriber => {
                        let tidx = ensure_topic(
                            &mut topics,
                            &mut topic_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        topic_subscribers.push((nidx, tidx));
                    }
                    CppNodeInterfaceKind::Service => {
                        let sidx = ensure_service(
                            &mut services,
                            &mut service_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        service_servers.push((nidx, sidx));
                    }
                    CppNodeInterfaceKind::Client => {
                        let sidx = ensure_service(
                            &mut services,
                            &mut service_key_to_idx,
                            &iface.name,
                            &iface.interface_type,
                        );
                        service_clients.push((nidx, sidx));
                    }
                    _ => {}
                }
            }
        }
    }

    // Apply remappings for external (known) nodes: e.g. joint_state_publisher_gui
    // publishes to joint_states by default, remapping to joint_commands means it
    // effectively publishes to joint_commands at runtime.
    for pkg in packages {
        for lf in &pkg.launch_files {
            for n in &lf.info.nodes {
                if n.remappings.is_empty() {
                    continue;
                }
                let nidx = match node_key_to_idx.get(&(n.package.clone(), n.executable.clone())) {
                    Some(&i) => i,
                    None => continue,
                };
                // Only apply for external nodes (not in our workspace)
                if workspace_packages.contains(&n.package) {
                    continue;
                }
                let defaults = known_external_publishers(&n.package, &n.executable);
                if defaults.is_empty() {
                    continue;
                }
                for (from_default, msg_type) in &defaults {
                    let from_norm = normalize_topic(from_default);
                    for (from, to) in &n.remappings {
                        if normalize_topic(from) == from_norm {
                            let to_idx =
                                ensure_topic(&mut topics, &mut topic_key_to_idx, to, msg_type);
                            topic_publishers.push((nidx, to_idx));
                        }
                    }
                }
            }
        }
    }

    RuntimeGraph {
        nodes,
        topics,
        services,
        topic_publishers,
        topic_subscribers,
        service_servers,
        service_clients,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("package.xml: {0}")]
    PackageXml(#[from] crate::package_xml::PackageXmlError),
}
