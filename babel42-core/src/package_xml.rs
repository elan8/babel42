//! package.xml parsing (REP-140, REP-149).

use crate::model::{
    Author, BuildType, Dependency, DependencyRole, Maintainer, PackageManifest, PackageUrl,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackageXmlError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse XML: {0}")]
    Xml(String),
}

/// Parse package.xml into full PackageManifest.
pub fn parse_package_xml(path: &Path) -> Result<PackageManifest, PackageXmlError> {
    let content = fs::read_to_string(path)?;
    parse_package_xml_str(&content).map_err(PackageXmlError::Xml)
}

pub fn parse_package_xml_str(content: &str) -> Result<PackageManifest, String> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut format: u32 = 2;
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut description: Option<String> = None;
    let mut maintainers: Vec<Maintainer> = Vec::new();
    let mut license: Option<String> = None;
    let mut urls: Vec<PackageUrl> = Vec::new();
    let mut authors: Vec<Author> = Vec::new();
    let mut deps: Vec<Dependency> = Vec::new();
    let mut build_types: Vec<BuildType> = Vec::new();
    let mut member_of_groups: Vec<String> = Vec::new();
    let mut is_metapackage = false;

    let mut buf = Vec::new();
    let mut current_tag: Option<String> = None;
    let mut in_export = false;

    // Track current element's attributes
    let mut current_attrs: HashMap<String, String> = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let tag = String::from_utf8_lossy(name.as_ref()).into_owned();
                current_tag = Some(tag.clone());
                if tag == "export" {
                    in_export = true;
                }
                current_attrs.clear();
                for attr in e.attributes() {
                    if let Ok(a) = attr {
                        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                        let val = String::from_utf8_lossy(a.value.as_ref()).into_owned();
                        current_attrs.insert(key, val);
                    }
                }
                // format is attribute on <package format="2">
                if tag == "package" {
                    if let Some(f) = current_attrs.get("format") {
                        format = f.parse().unwrap_or(2);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let tag = String::from_utf8_lossy(name.as_ref()).into_owned();
                if tag == "export" {
                    in_export = false;
                }
                current_tag = None;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    continue;
                }
                match current_tag.as_deref() {
                    Some("name") => name = Some(text),
                    Some("version") => version = Some(text),
                    Some("description") => description = Some(text),
                    Some("license") => license = Some(text),
                    Some("maintainer") => {
                        maintainers.push(Maintainer {
                            email: current_attrs.get("email").cloned(),
                            name: text,
                        });
                    }
                    Some("author") => {
                        authors.push(Author {
                            email: current_attrs.get("email").cloned(),
                            name: text,
                        });
                    }
                    Some("url") => {
                        let url_type = current_attrs
                            .get("type")
                            .cloned()
                            .unwrap_or_else(|| "website".to_string());
                        urls.push(PackageUrl {
                            r#type: url_type,
                            url: text,
                        });
                    }
                    Some("build_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::Build,
                    }),
                    Some("buildtool_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::BuildTool,
                    }),
                    Some("build_export_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::BuildExport,
                    }),
                    Some("exec_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::Exec,
                    }),
                    Some("run_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::Exec,
                    }),
                    Some("test_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::Test,
                    }),
                    Some("doc_depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::Doc,
                    }),
                    Some("depend") => deps.push(Dependency {
                        name: text,
                        role: DependencyRole::Depend,
                    }),
                    Some("member_of_group") => member_of_groups.push(text),
                    Some("metapackage") if in_export => is_metapackage = true,
                    Some("build_type") if in_export => {
                        build_types.push(BuildType::from_str(&text));
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    let name = name.ok_or("missing <name>")?;
    let version = version.unwrap_or_else(|| "0.0.0".to_string());

    Ok(PackageManifest {
        format,
        name,
        version,
        description,
        maintainers,
        license,
        urls,
        authors,
        dependencies: deps,
        build_types,
        member_of_groups,
        is_metapackage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DependencyRole;

    #[test]
    fn parse_format3_arduinobot_msgs() {
        let xml = r#"<?xml version="1.0"?>
<package format="3">
  <name>arduinobot_msgs</name>
  <version>0.0.0</version>
  <description>Definition of Interfaces for the Arduinobot</description>
  <maintainer email="antonio.brandi@outlook.it">Antonio Brandi</maintainer>
  <license>Apache 2.0</license>
  <buildtool_depend>ament_cmake</buildtool_depend>
  <build_depend>rosidl_default_generators</build_depend>
  <depend>std_msgs</depend>
  <depend>action_msgs</depend>
  <exec_depend>rosidl_default_runtime</exec_depend>
  <member_of_group>rosidl_interface_packages</member_of_group>
  <export>
    <build_type>ament_cmake</build_type>
  </export>
</package>"#;
        let m = parse_package_xml_str(xml).unwrap();
        assert_eq!(m.format, 3);
        assert_eq!(m.name, "arduinobot_msgs");
        assert_eq!(m.maintainers.len(), 1);
        assert_eq!(m.maintainers[0].email.as_deref(), Some("antonio.brandi@outlook.it"));
        assert_eq!(m.member_of_groups, ["rosidl_interface_packages"]);
        assert_eq!(m.build_types.len(), 1);
        assert!(matches!(m.build_types[0], crate::model::BuildType::AmentCmake));
        let dep_names: Vec<_> = m.dependencies.iter().map(|d| &d.name).collect();
        assert!(dep_names.contains(&&"std_msgs".to_string()));
        assert!(m.dependencies.iter().any(|d| d.role == DependencyRole::BuildTool && d.name == "ament_cmake"));
    }

    #[test]
    fn parse_run_depend() {
        let xml = r#"<?xml version="1.0"?>
<package>
  <name>karto_sdk</name>
  <version>1.1.4</version>
  <description>Catkinized ROS packaging of the OpenKarto library</description>
  <maintainer email="mferguson@fetchrobotics.com">Michael Ferguson</maintainer>
  <license>LGPLv3</license>
  <buildtool_depend>ament_cmake</buildtool_depend>
  <build_depend>boost</build_depend>
  <run_depend>boost</run_depend>
  <run_depend>tbb</run_depend>
</package>"#;
        let m = parse_package_xml_str(xml).unwrap();
        assert_eq!(m.name, "karto_sdk");
        let run_deps: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.role == DependencyRole::Exec)
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(run_deps, ["boost", "tbb"]);
    }
}
