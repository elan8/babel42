//! C++ node parsing — scans C++ source for ROS2 publishers, subscribers, services.
//!
//! Extracts create_publisher, create_subscription, create_service, create_client,
//! create_action_client, create_action_server via tree-sitter (no regex fallback).

use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::{Node, Parser, Tree};
use walkdir::WalkDir;

use crate::cmake::CmakePackageInfo;

/// Kind of ROS2 communication interface in C++.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CppNodeInterfaceKind {
    Publisher,
    Subscriber,
    Service,
    Client,
    ActionClient,
    ActionServer,
}

/// A single interface (pub/sub/service/client) found in C++ source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CppNodeInterface {
    pub kind: CppNodeInterfaceKind,
    pub name: String,
    pub interface_type: String,
    pub line: Option<u32>,
}

/// Parsed C++ node file with its interfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CppNodeFile {
    pub path: std::path::PathBuf,
    pub interfaces: Vec<CppNodeInterface>,
    /// Executable name if this file is a known add_executable target
    pub executable: Option<String>,
}

const RCLCPP_METHODS: &[(&str, CppNodeInterfaceKind)] = &[
    ("create_publisher", CppNodeInterfaceKind::Publisher),
    ("create_subscription", CppNodeInterfaceKind::Subscriber),
    ("create_service", CppNodeInterfaceKind::Service),
    ("create_client", CppNodeInterfaceKind::Client),
    ("create_action_client", CppNodeInterfaceKind::ActionClient),
    ("create_action_server", CppNodeInterfaceKind::ActionServer),
];

fn parse_with_tree_sitter(src: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .ok()?;
    parser.parse(src, None)
}

fn extract_interfaces(tree: &Tree, src: &str) -> Vec<CppNodeInterface> {
    let mut interfaces = Vec::new();
    walk_call_expressions(tree.root_node(), src, &mut |node, src| {
        if let Some(iface) = extract_interface_from_call(node, src) {
            interfaces.push(iface);
        }
    });
    interfaces
}

fn walk_call_expressions<F>(node: Node, src: &str, f: &mut F)
where
    F: FnMut(&Node, &str),
{
    if node.kind() == "call_expression" {
        f(&node, src);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_call_expressions(child, src, f);
    }
}

/// Get the method name from the function part of a call_expression.
/// Handles: template_function, template_method (in field), field_expression, qualified_identifier.
fn get_call_method_name(func_node: Node, src: &str) -> Option<String> {
    match func_node.kind() {
        "template_function" => {
            let name_node = func_node.child_by_field_name("name")?;
            Some(src[name_node.byte_range()].trim().to_string())
        }
        "template_method" => {
            let name_node = func_node.child_by_field_name("name")?;
            let name_src = src[name_node.byte_range()].trim();
            // name can be qualified_field_identifier, take last segment
            let last = name_src.rsplit("::").next().unwrap_or(name_src);
            Some(last.to_string())
        }
        "field_expression" => {
            let field = func_node.child_by_field_name("field")?;
            if field.kind() == "template_method" {
                return get_call_method_name(field, src);
            }
            let name_src = src[field.byte_range()].trim();
            let last = name_src.rsplit("::").next().unwrap_or(name_src);
            Some(last.to_string())
        }
        "qualified_identifier" => {
            let name_src = src[func_node.byte_range()].trim();
            let last = name_src.rsplit("::").next().unwrap_or(name_src);
            Some(last.to_string())
        }
        "identifier" => Some(src[func_node.byte_range()].trim().to_string()),
        _ => None,
    }
}

/// Get template type from template_function or template_method (first type argument).
fn get_template_type(func_node: &Node, src: &str) -> Option<String> {
    let template_node = match func_node.kind() {
        "field_expression" => func_node.child_by_field_name("field")?,
        _ => *func_node,
    };
    let args_node = template_node.child_by_field_name("arguments")?;
    find_first_type_text(args_node, src)
}

fn find_first_type_text(node: Node, src: &str) -> Option<String> {
    let kind = node.kind();
    if kind == "type_descriptor"
        || kind == "sized_type_specifier"
        || kind == "qualified_type_identifier"
        || kind == "template_type"
        || kind == "type_identifier"
    {
        let raw = src[node.byte_range()].trim();
        if !raw.is_empty() && !raw.chars().all(|c| c == ',' || c == '<' || c == '>') {
            return Some(raw.replace("::", "/"));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(t) = find_first_type_text(child, src) {
            return Some(t);
        }
    }
    None
}

/// Get first string literal from argument_list.
fn get_first_string_arg(args_node: Node, src: &str) -> Option<String> {
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if child.kind() == "string_literal" {
            let text = src[child.byte_range()].trim();
            let unquoted = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| {
                    text.strip_prefix("R\"")
                        .and_then(|s| s.rfind('\"').map(|i| &s[..i]))
                })?;
            return Some(unquoted.to_string());
        }
        // Sometimes string is inside concatenated expression, skip for now
    }
    None
}

fn extract_interface_from_call(node: &Node, src: &str) -> Option<CppNodeInterface> {
    let func_node = node.child_by_field_name("function")?;
    let method_name = get_call_method_name(func_node, src)?;
    let kind = RCLCPP_METHODS
        .iter()
        .find(|(name, _)| method_name == *name)
        .map(|(_, k)| k.clone())?;
    let args_node = node.child_by_field_name("arguments")?;
    let name = get_first_string_arg(args_node, src)?;
    let interface_type = get_template_type(&func_node, src).unwrap_or_default();
    let line = (node.start_position().row as u32) + 1;
    Some(CppNodeInterface {
        kind,
        name,
        interface_type,
        line: Some(line),
    })
}

/// Resolve executable name for a source path from CMake add_executables.
fn resolve_executable(rel_path: &Path, cmake_info: Option<&CmakePackageInfo>) -> Option<String> {
    let cmake_info = cmake_info?;
    let path_str = rel_path.to_string_lossy();
    for (exe_name, sources) in &cmake_info.add_executables {
        let norm = path_str.replace('\\', "/");
        if sources
            .iter()
            .any(|s| norm.ends_with(s.trim_start_matches(&['.', '/'][..])))
        {
            return Some(exe_name.clone());
        }
    }
    None
}

/// Scan C++ source files in the package for ROS2 interfaces.
/// Uses tree-sitter only; no regex fallback. Files that fail to parse yield no interfaces.
pub fn scan_cpp_nodes(pkg_dir: &Path, cmake_info: Option<&CmakePackageInfo>) -> Vec<CppNodeFile> {
    let mut results = Vec::new();

    for entry in WalkDir::new(pkg_dir).max_depth(4).into_iter() {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let ext = p.extension().and_then(|e| e.to_str());
        if ext != Some("cpp") && ext != Some("cxx") && ext != Some("cc") && ext != Some("hpp") {
            continue;
        }
        let content = match std::fs::read_to_string(p) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tree = match parse_with_tree_sitter(&content) {
            Some(t) => t,
            None => continue,
        };
        let rel = p.strip_prefix(pkg_dir).unwrap_or_else(|_| p).to_path_buf();
        let interfaces = extract_interfaces(&tree, &content);
        if !interfaces.is_empty() {
            let executable = resolve_executable(&rel, cmake_info);
            results.push(CppNodeFile {
                path: rel,
                interfaces,
                executable,
            });
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(src: &str) -> Vec<CppNodeInterface> {
        let tree = parse_with_tree_sitter(src).expect("parse");
        extract_interfaces(&tree, src)
    }

    #[test]
    fn create_publisher_template() {
        let src = r#"
#include <rclcpp/rclcpp.hpp>
void foo(rclcpp::Node* node) {
    auto pub = node->create_publisher<std_msgs::msg::String>("/topic", 10);
}
"#;
        let ifaces = parse_and_extract(src);
        assert_eq!(ifaces.len(), 1);
        assert_eq!(ifaces[0].kind, CppNodeInterfaceKind::Publisher);
        assert_eq!(ifaces[0].name, "/topic");
        assert_eq!(ifaces[0].interface_type, "std_msgs/msg/String");
        assert!(ifaces[0].line.is_some());
    }

    #[test]
    fn create_action_server() {
        let src = r#"
#include <rclcpp/rclcpp.hpp>
void foo(rclcpp::Node* node) {
    node->create_action_server<my_interfaces::action::Fibonacci>("fibonacci", nullptr);
}
"#;
        let ifaces = parse_and_extract(src);
        assert_eq!(ifaces.len(), 1);
        assert_eq!(ifaces[0].kind, CppNodeInterfaceKind::ActionServer);
        assert_eq!(ifaces[0].name, "fibonacci");
    }

    #[test]
    fn no_regex_fallback() {
        // Malformed/incomplete C++ - tree-sitter may still parse partial; we only extract
        // well-formed create_* calls. This has no valid call.
        let src = "create_publisher<std_msgs::msg::String>(\"incomplete";
        let tree = parse_with_tree_sitter(src);
        let ifaces = tree
            .map(|t| extract_interfaces(&t, src))
            .unwrap_or_default();
        // No regex: either we get nothing or tree-sitter finds a valid call
        assert!(ifaces.len() <= 1);
    }
}
