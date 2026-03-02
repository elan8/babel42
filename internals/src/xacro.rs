//! Xacro/URDF.xacro file parser.
//!
//! Extracts includes, package references, ros2_control config, Gazebo plugins, etc.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XacroError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML parse error: {0}")]
    Xml(String),
}

/// Parsed information from a single xacro file.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct XacroFileInfo {
    /// Included files: (package, path) from xacro:include filename="$(find pkg)/path"
    pub includes: Vec<(String, String)>,
    /// Package names from $(find pkg), $(find-pkg pkg), package://pkg/
    pub package_refs: Vec<String>,
    /// xacro:property names
    pub properties: Vec<String>,
    /// xacro:arg names
    pub args: Vec<String>,
    /// xacro:macro names
    pub macros: Vec<String>,
    /// ros2_control hardware plugins (e.g. gazebo_ros2_control/GazeboSystem)
    pub ros2_control_plugins: Vec<String>,
    /// Joint names in ros2_control
    pub ros2_control_joints: Vec<String>,
    /// Gazebo plugin filenames (e.g. libgazebo_ros2_control.so)
    pub gazebo_plugins: Vec<String>,
}

/// Extract package names from $(find xxx), $(find-pkg xxx), package://xxx/
fn extract_package_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        // $(find pkg)
        if i + 7 <= bytes.len() && &bytes[i..i + 7] == b"$(find " {
            let start = i + 7;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b')' && end > start {
                refs.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
            }
            i = end + 1;
            continue;
        }
        // $(find-pkg pkg)
        if i + 11 <= bytes.len() && &bytes[i..i + 11] == b"$(find-pkg " {
            let start = i + 11;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b')' && end > start {
                refs.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
            }
            i = end + 1;
            continue;
        }
        // package://pkg/
        if i + 12 <= bytes.len() && &bytes[i..i + 12] == b"package://" {
            let start = i + 12;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b'/' && end > start {
                refs.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
    refs
}

/// Parse xacro:include filename="$(find pkg)/urdf/file.xacro" → (pkg, urdf/file.xacro)
fn parse_include_filename(filename: &str) -> Option<(String, String)> {
    let trimmed = filename.trim();
    if let Some(rest) = trimmed.strip_prefix("$(find ") {
        if let Some(end) = rest.find(')') {
            let pkg = rest[..end].trim().to_string();
            let path = rest[end + 1..].trim().trim_start_matches('/').to_string();
            return Some((pkg, path));
        }
    }
    if let Some(rest) = trimmed.strip_prefix("$(find-pkg ") {
        if let Some(end) = rest.find(')') {
            let pkg = rest[..end].trim().to_string();
            let path = rest[end + 1..].trim().trim_start_matches('/').to_string();
            return Some((pkg, path));
        }
    }
    None
}

/// Parse a xacro file and extract structural information.
pub fn parse_xacro_file(path: &Path) -> Result<XacroFileInfo, XacroError> {
    let content = fs::read_to_string(path)?;
    parse_xacro_str(&content).map_err(XacroError::Xml)
}

pub fn parse_xacro_str(content: &str) -> Result<XacroFileInfo, String> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut info = XacroFileInfo::default();
    let mut package_refs_set: HashSet<String> = HashSet::new();
    let mut elem_stack: Vec<String> = Vec::new(); // Track parent elements for context
    let mut inside_hardware_plugin = false; // <plugin> inside <hardware> has text content

    let mut buf = Vec::new();
    let mut attrs: Vec<(String, String)> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = String::from_utf8_lossy(name.local_name().as_ref()).into_owned();
                let prefix = name
                    .prefix()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).into_owned())
                    .unwrap_or_default();

                elem_stack.push(local.clone());
                inside_hardware_plugin = elem_stack.last() == Some(&"plugin".to_string())
                    && elem_stack.contains(&"hardware".to_string());

                attrs.clear();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
                    let v = String::from_utf8_lossy(a.value.as_ref()).into_owned();
                    attrs.push((k.clone(), v.clone()));
                    for r in extract_package_refs(&v) {
                        package_refs_set.insert(r);
                    }
                }

                match (prefix.as_str(), local.as_str()) {
                    ("xacro", "include") => {
                        for (k, v) in &attrs {
                            if k == "filename" {
                                if let Some((pkg, p)) = parse_include_filename(v) {
                                    info.includes.push((pkg, p));
                                }
                            }
                        }
                    }
                    ("xacro", "property") => {
                        for (k, v) in &attrs {
                            if k == "name" {
                                info.properties.push(v.clone());
                            }
                        }
                    }
                    ("xacro", "arg") => {
                        for (k, v) in &attrs {
                            if k == "name" {
                                info.args.push(v.clone());
                            }
                        }
                    }
                    ("xacro", "macro") => {
                        for (k, v) in &attrs {
                            if k == "name" {
                                info.macros.push(v.clone());
                            }
                        }
                    }
                    (_, "plugin") => {
                        if let Some((_, v)) = attrs.iter().find(|(k, _)| k == "filename") {
                            if elem_stack.iter().rev().take(5).any(|s| *s == "gazebo") {
                                info.gazebo_plugins.push(v.clone());
                            } else {
                                info.ros2_control_plugins.push(v.clone());
                            }
                        }
                    }
                    (_, "joint") => {
                        for (k, v) in &attrs {
                            if k == "name" {
                                info.ros2_control_joints.push(v.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if inside_hardware_plugin {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        info.ros2_control_plugins.push(text);
                    }
                }
            }
            Ok(Event::End(_)) => {
                elem_stack.pop();
                inside_hardware_plugin = elem_stack.last() == Some(&"plugin".to_string())
                    && elem_stack.contains(&"hardware".to_string());
            }
            Ok(Event::Empty(e)) => {
                let local = String::from_utf8_lossy(e.name().local_name().as_ref()).into_owned();
                let prefix = e
                    .name()
                    .prefix()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).into_owned())
                    .unwrap_or_default();
                attrs.clear();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
                    let v = String::from_utf8_lossy(a.value.as_ref()).into_owned();
                    attrs.push((k.clone(), v.clone()));
                    for r in extract_package_refs(&v) {
                        package_refs_set.insert(r);
                    }
                    match (prefix.as_str(), local.as_str()) {
                        ("xacro", "include") if k == "filename" => {
                            if let Some((pkg, p)) = parse_include_filename(&v) {
                                info.includes.push((pkg, p));
                            }
                        }
                        ("xacro", "property") if k == "name" => info.properties.push(v),
                        ("xacro", "arg") if k == "name" => info.args.push(v),
                        ("xacro", "macro") if k == "name" => info.macros.push(v),
                        (_, "plugin") if k == "filename" => {
                            if v.contains("gazebo") {
                                info.gazebo_plugins.push(v);
                            } else {
                                info.ros2_control_plugins.push(v);
                            }
                        }
                        (_, "joint") if k == "name" => info.ros2_control_joints.push(v),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    // Also scan full content for refs in text nodes (e.g. <parameters>$(find pkg)/...</parameters>)
    for r in extract_package_refs(content) {
        package_refs_set.insert(r);
    }
    info.package_refs = package_refs_set.into_iter().collect();
    info.package_refs.sort();
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_include_and_refs() {
        let s = r#"<?xml version="1.0"?>
<robot xmlns:xacro="http://www.ros.org/wiki/xacro">
    <xacro:include filename="$(find arduinobot_description)/urdf/arduinobot_gazebo.xacro" />
    <mesh filename="package://arduinobot_description/meshes/base.STL"/>
</robot>"#;
        let info = parse_xacro_str(s).unwrap();
        assert_eq!(info.includes.len(), 1);
        assert_eq!(info.includes[0].0, "arduinobot_description");
        assert!(info
            .package_refs
            .contains(&"arduinobot_description".to_string()));
    }

    #[test]
    fn parse_ros2_control_plugin_text() {
        let s = r#"<?xml version="1.0"?>
<robot xmlns:xacro="http://www.ros.org/wiki/xacro">
    <ros2_control name="RobotSystem">
        <hardware>
            <plugin>gazebo_ros2_control/GazeboSystem</plugin>
        </hardware>
    </ros2_control>
</robot>"#;
        let info = parse_xacro_str(s).unwrap();
        assert!(info
            .ros2_control_plugins
            .contains(&"gazebo_ros2_control/GazeboSystem".to_string()));
    }
}
