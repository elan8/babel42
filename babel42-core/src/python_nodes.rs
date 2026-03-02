//! Python node parsing — scans Python scripts for ROS2 publishers, subscribers, services.
//!
//! Uses tree-sitter to find create_publisher, create_subscription, create_service,
//! create_client, create_action_client calls and extract topic/service/action names and types.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_python;
use walkdir::WalkDir;

/// Kind of ROS2 communication interface used by a Python node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PyNodeInterfaceKind {
    Publisher,
    Subscriber,
    Service,
    Client,
    ActionClient,
    ActionServer,
}

/// A single interface (pub/sub/service/client) used by a Python node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PyNodeInterface {
    pub kind: PyNodeInterfaceKind,
    /// Topic, service, or action name
    pub name: String,
    /// Message, service, or action type (e.g. "std_msgs/msg/String")
    pub interface_type: String,
    /// Line number in source (1-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// Parsed Python node file with its interfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonNodeFile {
    /// Path relative to package root
    pub path: std::path::PathBuf,
    /// Interfaces (publishers, subscribers, services, clients) found in the file
    pub interfaces: Vec<PyNodeInterface>,
}

/// Scan Python files for ROS2 node interfaces.
/// Looks in scripts/, lib/<pkg>/ and package root.
pub fn scan_python_nodes(pkg_dir: &Path, _pkg_name: &str) -> Vec<PythonNodeFile> {
    let mut results = Vec::new();
    let mut parser = Parser::new();
    let lang = tree_sitter_python::LANGUAGE.into();
    if parser.set_language(&lang).is_err() {
        return results;
    }

    for entry in WalkDir::new(pkg_dir).max_depth(3).into_iter() {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if p.extension().map_or(true, |e| e != "py") {
            continue;
        }
        // Skip launch files
        if p
            .file_stem()
            .and_then(|s| s.to_str())
            .map_or(false, |s| s.ends_with(".launch"))
        {
            continue;
        }
        let rel = p.strip_prefix(pkg_dir).unwrap_or_else(|_| p).to_path_buf();
        let content = match std::fs::read_to_string(p) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => continue,
        };
        // Skip files that don't import rclpy (avoids false positives, improves performance)
        if !has_rclpy_import(&tree, &content) {
            continue;
        }
        let interfaces = extract_interfaces(&tree, &content);
        if !interfaces.is_empty() {
            results.push(PythonNodeFile {
                path: rel,
                interfaces,
            });
        }
    }
    results
}

fn extract_interfaces(tree: &Tree, src: &str) -> Vec<PyNodeInterface> {
    let mut interfaces = Vec::new();
    walk_call_nodes(tree.root_node(), src, &mut |node, src| {
        if let Some(iface) = extract_interface_from_call(node, src) {
            interfaces.push(iface);
        }
    });
    interfaces
}

fn walk_call_nodes<F>(node: Node, src: &str, f: &mut F)
where
    F: FnMut(&Node, &str),
{
    if node.kind() == "call" {
        f(&node, src);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_call_nodes(child, src, f);
    }
}

fn get_call_name(node: &Node, src: &str) -> Option<String> {
    let child = node.child(0)?;
    Some(src[child.byte_range()].trim().to_string())
}

fn get_string_arg(node: &Node, src: &str) -> Option<String> {
    let text = src[node.byte_range()].trim();
    let text = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
    Some(text.to_string())
}

/// Parsed args: positional strings + keyword map (arg_name -> value).
fn get_call_args_full(node: &Node, src: &str) -> Option<(Vec<String>, std::collections::HashMap<String, String>)> {
    let args_node = node.child_by_field_name("arguments")?;
    let mut positional = Vec::new();
    let mut keyword = std::collections::HashMap::new();
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        match child.kind() {
            "keyword_argument" => {
                // keyword_argument: (name) (value)
                let name_node = child.child_by_field_name("name")?;
                let name = src[name_node.byte_range()].trim().to_string();
                let value_node = child.child_by_field_name("value")?;
                if value_node.kind() == "string" {
                    if let Some(s) = get_string_arg(&value_node, src) {
                        keyword.insert(name, s);
                    }
                }
            }
            "string" => {
                if let Some(s) = get_string_arg(&child, src) {
                    positional.push(s);
                }
            }
            _ => {}
        }
    }
    Some((positional, keyword))
}

/// Check if the file imports rclpy (import rclpy or from rclpy).
fn has_rclpy_import(_tree: &Tree, src: &str) -> bool {
    src.contains("import rclpy") || src.contains("from rclpy")
}

fn extract_interface_from_call(node: &Node, src: &str) -> Option<PyNodeInterface> {
    let call_name = get_call_name(node, src)?;
    let kind = if call_name == "create_publisher" || call_name.ends_with("create_publisher") {
        PyNodeInterfaceKind::Publisher
    } else if call_name == "create_subscription" || call_name.ends_with("create_subscription") {
        PyNodeInterfaceKind::Subscriber
    } else if call_name == "create_service" || call_name.ends_with("create_service") {
        PyNodeInterfaceKind::Service
    } else if call_name == "create_client" || call_name.ends_with("create_client") {
        PyNodeInterfaceKind::Client
    } else if call_name == "create_action_client" || call_name.ends_with("create_action_client") {
        PyNodeInterfaceKind::ActionClient
    } else if call_name == "create_action_server" || call_name.ends_with("create_action_server") {
        PyNodeInterfaceKind::ActionServer
    } else {
        return None;
    };

    let (positional, keyword) = get_call_args_full(node, src)?;

    // Resolve type and name from positional + keyword args.
    // Keywords: topic, msg_type, srv_type, srv_name, action_type, action_name
    let mut interface_type = keyword
        .get("msg_type")
        .or_else(|| keyword.get("srv_type"))
        .or_else(|| keyword.get("action_type"))
        .cloned();
    let mut iface_name = keyword
        .get("topic")
        .or_else(|| keyword.get("srv_name"))
        .or_else(|| keyword.get("action_name"))
        .cloned();

    // Fall back to positional args when keywords missing
    if interface_type.is_none() || iface_name.is_none() {
        if positional.len() >= 2 {
            let a0 = positional.first()?.clone();
            let a1 = positional.get(1)?.clone();
            let (t, n) = if a0.contains('/') && !a1.contains('/') {
                (a0, a1)
            } else if a1.contains('/') && !a0.contains('/') {
                (a1, a0)
            } else {
                (a0, a1)
            };
            interface_type = interface_type.or(Some(t));
            iface_name = iface_name.or(Some(n));
        } else if positional.len() == 1 {
            interface_type = interface_type.or_else(|| Some(positional[0].clone()));
            iface_name = iface_name.or_else(|| Some(String::new()));
        }
    }

    let interface_type = interface_type.unwrap_or_else(|| String::new());
    let iface_name = iface_name.unwrap_or_else(|| String::new());
    if interface_type.is_empty() {
        return None;
    }

    let line = (node.start_position().row as u32) + 1;

    Some(PyNodeInterface {
        kind,
        name: iface_name,
        interface_type,
        line: Some(line),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(src: &str) -> Vec<PyNodeInterface> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(src, None).expect("parse");
        extract_interfaces(&tree, src)
    }

    #[test]
    fn keyword_args_publisher() {
        let src = r#"
import rclpy
from rclpy.node import Node

class MyNode(Node):
    def __init__(self):
        super().__init__('my_node')
        self.pub = self.create_publisher(msg_type="std_msgs/msg/String", topic="/foo", qos_profile=10)
"#;
        let ifaces = parse_and_extract(src);
        assert_eq!(ifaces.len(), 1);
        assert_eq!(ifaces[0].kind, PyNodeInterfaceKind::Publisher);
        assert_eq!(ifaces[0].name, "/foo");
        assert!(ifaces[0].line.is_some());
    }

    #[test]
    fn create_action_server() {
        let src = r#"
import rclpy

def main():
    rclpy.init()
    node = rclpy.create_node('fibonacci_server')
    node.create_action_server(action_type="my_interfaces/action/Fibonacci", action_name="fibonacci", execute_callback=execute)
"#;
        let ifaces = parse_and_extract(src);
        assert_eq!(ifaces.len(), 1);
        assert_eq!(ifaces[0].kind, PyNodeInterfaceKind::ActionServer);
        assert_eq!(ifaces[0].name, "fibonacci");
        assert!(ifaces[0].line.is_some());
    }

}
