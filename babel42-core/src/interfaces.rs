//! Parsers for .msg, .srv, .action interface files.

use crate::model::{ActionDefinition, FieldDef, MsgDefinition, SrvDefinition};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InterfaceError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Parse a single field line: `type name` or `type[] name` or `type[N] name`
fn parse_field_line(line: &str) -> Option<FieldDef> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut parts = line.split_whitespace();
    let type_part = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() {
        return None; // extra tokens
    }

    let (field_type, array_len) = if type_part.ends_with(']') {
        let open = type_part.rfind('[')?;
        let base = &type_part[..open];
        let bracket = &type_part[open + 1..type_part.len() - 1];
        let len = if bracket.is_empty() {
            Some(0) // unbounded []
        } else {
            bracket.parse().ok().map(|n: u32| n)
        };
        (base.to_string(), len)
    } else {
        (type_part.to_string(), None)
    };

    Some(FieldDef {
        field_type,
        name: name.to_string(),
        array_len: array_len.map(|n| if n == 0 { 0 } else { n }),
    })
}

/// Parse .msg file (list of fields).
pub fn parse_msg_file(path: &Path) -> Result<MsgDefinition, InterfaceError> {
    let content = fs::read_to_string(path)?;
    parse_msg_str(&content, path).map_err(InterfaceError::Parse)
}

pub fn parse_msg_str(content: &str, path: &Path) -> Result<MsgDefinition, String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let fields: Vec<FieldDef> = content
        .lines()
        .filter_map(parse_field_line)
        .collect();

    Ok(MsgDefinition { name, fields })
}

/// Parse .srv file (request --- response).
pub fn parse_srv_file(path: &Path) -> Result<SrvDefinition, InterfaceError> {
    let content = fs::read_to_string(path)?;
    parse_srv_str(&content, path).map_err(InterfaceError::Parse)
}

pub fn parse_srv_str(content: &str, path: &Path) -> Result<SrvDefinition, String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let sections: Vec<&str> = content.splitn(2, "---").collect();
    let request_section = sections.get(0).copied().unwrap_or("");
    let response_section = sections.get(1).copied().unwrap_or("");

    let request: Vec<FieldDef> = request_section.lines().filter_map(parse_field_line).collect();
    let response: Vec<FieldDef> = response_section.lines().filter_map(parse_field_line).collect();

    Ok(SrvDefinition {
        name,
        request,
        response,
    })
}

/// Parse .action file (goal --- result --- feedback).
pub fn parse_action_file(path: &Path) -> Result<ActionDefinition, InterfaceError> {
    let content = fs::read_to_string(path)?;
    parse_action_str(&content, path).map_err(InterfaceError::Parse)
}

pub fn parse_action_str(content: &str, path: &Path) -> Result<ActionDefinition, String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let sections: Vec<&str> = content.split("---").collect();
    let goal_section = sections.get(0).copied().unwrap_or("");
    let result_section = sections.get(1).copied().unwrap_or("");
    let feedback_section = sections.get(2).copied().unwrap_or("");

    let goal: Vec<FieldDef> = goal_section.lines().filter_map(parse_field_line).collect();
    let result: Vec<FieldDef> = result_section.lines().filter_map(parse_field_line).collect();
    let feedback: Vec<FieldDef> = feedback_section.lines().filter_map(parse_field_line).collect();

    Ok(ActionDefinition {
        name,
        goal,
        result,
        feedback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_field_primitive() {
        let f = parse_field_line("int32 x").unwrap();
        assert_eq!(f.field_type, "int32");
        assert_eq!(f.name, "x");
        assert_eq!(f.array_len, None);
    }

    #[test]
    fn parse_field_unbounded_array() {
        let f = parse_field_line("int32[] sequence").unwrap();
        assert_eq!(f.field_type, "int32");
        assert_eq!(f.name, "sequence");
        assert_eq!(f.array_len, Some(0));
    }

    #[test]
    fn parse_field_fixed_array() {
        let f = parse_field_line("float64[3] pos").unwrap();
        assert_eq!(f.field_type, "float64");
        assert_eq!(f.name, "pos");
        assert_eq!(f.array_len, Some(3));
    }

    #[test]
    fn parse_field_ignores_comments() {
        assert!(parse_field_line("# comment").is_none());
        assert!(parse_field_line("  ").is_none());
    }

    #[test]
    fn parse_srv_addtwoints() {
        let srv = r#"# Request
int64 a
int64 b
---
# Response
int64 sum"#;
        let def = parse_srv_str(srv, Path::new("AddTwoInts.srv")).unwrap();
        assert_eq!(def.name, "AddTwoInts");
        assert_eq!(def.request.len(), 2);
        assert_eq!(def.request[0].name, "a");
        assert_eq!(def.response.len(), 1);
        assert_eq!(def.response[0].name, "sum");
    }

    #[test]
    fn parse_action_fibonacci() {
        let action = r#"# Goal
int32 order
---
# Result
int32[] sequence
---
# Feedback
int32[] partial_sequence"#;
        let def = parse_action_str(action, Path::new("Fibonacci.action")).unwrap();
        assert_eq!(def.name, "Fibonacci");
        assert_eq!(def.goal.len(), 1);
        assert_eq!(def.goal[0].name, "order");
        assert_eq!(def.result.len(), 1);
        assert_eq!(def.result[0].array_len, Some(0));
        assert_eq!(def.feedback.len(), 1);
    }
}
