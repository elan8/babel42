//! Python launch file (.launch.py) parser using tree-sitter.
//!
//! Extracts IncludeLaunchDescription, Node, get_package_share_directory refs, etc.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use thiserror::Error;
use tree_sitter::{Node, Parser, Tree};

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Parsed information from a launch file.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LaunchFileInfo {
    /// Included launch files: (package, launch_file_path)
    pub included_launches: Vec<(String, String)>,
    /// Package names from get_package_share_directory("pkg")
    pub package_refs: Vec<String>,
    /// Nodes launched: (package, executable, name?)
    pub nodes: Vec<LaunchNode>,
}

/// Topic remapping: (from_topic, to_topic) — node's "from" appears as "to" at runtime.
pub type Remapping = (String, String);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LaunchNode {
    pub package: String,
    pub executable: String,
    pub name: Option<String>,
    /// Topic remappings: (from, to) — e.g. ("/joint_states", "/joint_commands")
    #[serde(default)]
    pub remappings: Vec<Remapping>,
}

/// Parse a .launch.py file.
pub fn parse_launch_file(path: &Path) -> Result<LaunchFileInfo, LaunchError> {
    let content = fs::read_to_string(path)?;
    parse_launch_str(&content)
}

pub fn parse_launch_str(content: &str) -> Result<LaunchFileInfo, LaunchError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| LaunchError::Parse(e.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| LaunchError::Parse("parse failed".to_string()))?;

    let mut info = LaunchFileInfo::default();
    let mut package_refs_set = HashSet::new();

    walk_tree(&tree, content, &mut |node, src| {
        if node.kind() == "call" {
            if let Some((pkg, file)) = extract_include_launch(node, src) {
                package_refs_set.insert(pkg.clone());
                info.included_launches.push((pkg, file));
            } else if let Some(pkg) = extract_get_package_share_dir(node, src) {
                package_refs_set.insert(pkg);
            } else if let Some(n) = extract_node(node, src) {
                info.nodes.push(n);
            }
        }
    });

    info.package_refs = package_refs_set.into_iter().collect();
    info.package_refs.sort();
    Ok(info)
}

fn walk_tree<F>(tree: &Tree, src: &str, f: &mut F)
where
    F: FnMut(&Node, &str),
{
    let root = tree.root_node();
    walk_node(&root, src, f);
}

fn walk_node<F>(node: &Node, src: &str, f: &mut F)
where
    F: FnMut(&Node, &str),
{
    f(node, src);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(&child, src, f);
    }
}

fn get_call_name(node: &Node, src: &str) -> Option<String> {
    // call has: function (identifier or attribute) and argument_list
    let child = node.child(0)?;
    match child.kind() {
        "identifier" => Some(src[child.byte_range()].trim().to_string()),
        "attribute" => {
            // a.b.c → take the full "a.b.c" or just the last part for matching
            Some(src[child.byte_range()].trim().to_string())
        }
        "call" => get_call_name(&child, src),
        _ => None,
    }
}

fn get_string_arg(node: &Node, src: &str) -> Option<String> {
    let text = src[node.byte_range()].trim();
    let text = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
    Some(text.to_string())
}

fn get_node_string(node: &Node, src: &str) -> String {
    src[node.byte_range()].trim().to_string()
}

/// Extract package from get_package_share_directory("pkg")
fn extract_get_package_share_dir(node: &Node, src: &str) -> Option<String> {
    let name = get_call_name(node, src)?;
    if !name.ends_with("get_package_share_directory") {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let first_arg = args.child(1)?; // argument_list: ( first_arg , ...
    if first_arg.kind() == "string" {
        return get_string_arg(&first_arg, src);
    }
    None
}

/// Extract (package, launch_file) from IncludeLaunchDescription(os.path.join(get_package_share_directory("pkg"), "launch", "file.launch.py"))
fn extract_include_launch(node: &Node, src: &str) -> Option<(String, String)> {
    let name = get_call_name(node, src)?;
    if name != "IncludeLaunchDescription" {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let first_arg = args.child(1)?; // first argument
    extract_package_and_launch_from_join(&first_arg, src)
}

/// os.path.join(get_package_share_directory("pkg"), "launch", "file.launch.py") → ("pkg", "file.launch.py")
fn extract_package_and_launch_from_join(node: &Node, src: &str) -> Option<(String, String)> {
    if node.kind() == "call" {
        let name = get_call_name(node, src)?;
        if name.ends_with("join") {
            let args = node.child_by_field_name("arguments")?;
            let mut strings = Vec::new();
            let mut pkg = None;
            let mut cursor = args.walk();
            for child in args.children(&mut cursor) {
                match child.kind() {
                    "call" => {
                        pkg = extract_get_package_share_dir(&child, src);
                    }
                    "string" => {
                        if let Some(s) = get_string_arg(&child, src) {
                            strings.push(s);
                        }
                    }
                    _ => {}
                }
            }
            let pkg = pkg?;
            // Typically ["launch", "controller.launch.py"] or similar
            let launch_file = strings.iter().find(|s| s.ends_with(".launch.py"))?.clone();
            return Some((pkg, launch_file));
        }
    }
    None
}

/// Extract remappings from list of tuples: [("/a", "/b"), ("/c", "/d")]
fn extract_remappings_list(list_node: &Node, src: &str) -> Vec<Remapping> {
    let mut result = Vec::new();
    let mut list_cursor = list_node.walk();
    for child in list_node.children(&mut list_cursor) {
        if child.kind() == "tuple" {
            let mut parts = Vec::new();
            let mut tuple_cursor = child.walk();
            for grandchild in child.children(&mut tuple_cursor) {
                if grandchild.kind() == "string" {
                    if let Some(s) = get_string_arg(&grandchild, src) {
                        parts.push(s);
                    }
                }
            }
            if parts.len() >= 2 {
                result.push((parts[0].clone(), parts[1].clone()));
            }
        }
    }
    result
}

/// Extract Node(package="...", executable="...", name="...", remappings=[...])
fn extract_node(node: &Node, src: &str) -> Option<LaunchNode> {
    let name = get_call_name(node, src)?;
    if name != "Node" {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let mut package = None;
    let mut executable = None;
    let mut node_name = None;
    let mut remappings = Vec::new();
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() == "keyword_argument" {
            let arg_name = child.child_by_field_name("name")?;
            let name_str = get_node_string(&arg_name, src);
            let value = child.child_by_field_name("value")?;
            if name_str == "remappings" && value.kind() == "list" {
                remappings = extract_remappings_list(&value, src);
            } else if value.kind() == "string" {
                if let Some(value_str) = get_string_arg(&value, src) {
                    match name_str.as_str() {
                        "package" => package = Some(value_str),
                        "executable" => executable = Some(value_str),
                        "name" => node_name = Some(value_str),
                        _ => {}
                    }
                }
            }
        }
    }
    let package = package?;
    let executable = executable?;
    Some(LaunchNode {
        package,
        executable,
        name: node_name,
        remappings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_get_package_share_directory() {
        let s = r#"get_package_share_directory("arduinobot_controller")"#;
        let info = parse_launch_str(s).unwrap();
        assert!(info
            .package_refs
            .contains(&"arduinobot_controller".to_string()));
    }

    #[test]
    fn parse_include_launch_description() {
        let s = r#"
IncludeLaunchDescription(
    os.path.join(
        get_package_share_directory("arduinobot_moveit"),
        "launch",
        "moveit.launch.py"
    ),
    launch_arguments={"is_sim": "True"}.items()
)"#;
        let info = parse_launch_str(s).unwrap();
        assert_eq!(info.included_launches.len(), 1);
        assert_eq!(info.included_launches[0].0, "arduinobot_moveit");
        assert_eq!(info.included_launches[0].1, "moveit.launch.py");
    }

    #[test]
    fn parse_node() {
        let s = r#"
Node(
    package="moveit_ros_move_group",
    executable="move_group",
    output="screen",
)
"#;
        let info = parse_launch_str(s).unwrap();
        assert_eq!(info.nodes.len(), 1);
        assert_eq!(info.nodes[0].package, "moveit_ros_move_group");
        assert_eq!(info.nodes[0].executable, "move_group");
        assert!(info.nodes[0].remappings.is_empty());
    }

    #[test]
    fn parse_node_with_remappings() {
        let s = r#"
Node(
    package='joint_state_publisher_gui',
    executable='joint_state_publisher_gui',
    remappings=[
        ('/joint_states', '/joint_commands'),
    ]
)
"#;
        let info = parse_launch_str(s).unwrap();
        assert_eq!(info.nodes.len(), 1);
        assert_eq!(info.nodes[0].package, "joint_state_publisher_gui");
        assert_eq!(info.nodes[0].remappings.len(), 1);
        assert_eq!(info.nodes[0].remappings[0].0, "/joint_states");
        assert_eq!(info.nodes[0].remappings[0].1, "/joint_commands");
    }
}
