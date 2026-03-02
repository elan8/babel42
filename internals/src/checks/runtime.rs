//! Runtime rules — topic/service publisher/subscriber consistency, type mismatches.

use crate::checks::model::{Finding, Location, Severity};
use crate::model::Project;
use std::collections::HashMap;

pub fn check_runtime(project: &Project) -> Vec<Finding> {
    let mut findings = Vec::new();
    let rg = &project.runtime_graph;

    let mut topic_pub_count: HashMap<usize, usize> = HashMap::new();
    let mut topic_sub_count: HashMap<usize, usize> = HashMap::new();
    let mut topic_pub_types: HashMap<usize, Vec<String>> = HashMap::new();
    let mut topic_sub_types: HashMap<usize, Vec<String>> = HashMap::new();

    for &(_, tidx) in &rg.topic_publishers {
        *topic_pub_count.entry(tidx).or_insert(0) += 1;
    }
    for &(_, tidx) in &rg.topic_subscribers {
        *topic_sub_count.entry(tidx).or_insert(0) += 1;
    }

    for (_, tidx) in &rg.topic_publishers {
        if let Some(t) = rg.topics.get(*tidx) {
            topic_pub_types
                .entry(*tidx)
                .or_default()
                .push(t.msg_type.clone());
        }
    }
    for (_, tidx) in &rg.topic_subscribers {
        if let Some(t) = rg.topics.get(*tidx) {
            topic_sub_types
                .entry(*tidx)
                .or_default()
                .push(t.msg_type.clone());
        }
    }

    for (tidx, topic) in rg.topics.iter().enumerate() {
        let pub_count = topic_pub_count.get(&tidx).copied().unwrap_or(0);
        let sub_count = topic_sub_count.get(&tidx).copied().unwrap_or(0);

        if sub_count > 0 && pub_count == 0 {
            findings.push(
                Finding::new(
                    "runtime/topic_no_publisher",
                    Severity::Warn,
                    format!(
                        "Topic '{}' has {} subscriber(s) but no publishers",
                        topic.name, sub_count
                    ),
                )
                .with_location(Location {
                    package: None,
                    file: None,
                    line: None,
                    context: Some(format!("msg_type: {}", topic.msg_type)),
                }),
            );
        }

        if pub_count > 0 && sub_count == 0 {
            findings.push(
                Finding::new(
                    "runtime/topic_no_subscriber",
                    Severity::Info,
                    format!(
                        "Topic '{}' has {} publisher(s) but no subscribers",
                        topic.name, pub_count
                    ),
                )
                .with_location(Location {
                    package: None,
                    file: None,
                    line: None,
                    context: Some(format!("msg_type: {}", topic.msg_type)),
                }),
            );
        }

        let pub_types = topic_pub_types.get(&tidx).cloned().unwrap_or_default();
        let sub_types = topic_sub_types.get(&tidx).cloned().unwrap_or_default();
        if !pub_types.is_empty() && !sub_types.is_empty() {
            let pub_set: std::collections::HashSet<String> = pub_types.iter().cloned().collect();
            let sub_set: std::collections::HashSet<String> = sub_types.iter().cloned().collect();
            if pub_set != sub_set {
                findings.push(
                    Finding::new(
                        "runtime/topic_type_mismatch",
                        Severity::Error,
                        format!(
                            "Topic '{}' has publisher/subscriber type mismatch: publishers use {:?}, subscribers use {:?}",
                            topic.name, pub_types, sub_types
                        ),
                    )
                    .with_location(Location {
                        package: None,
                        file: None,
                        line: None,
                        context: Some(topic.msg_type.clone()),
                    }),
                );
            }
        }
    }

    let mut service_server_types: HashMap<usize, Vec<String>> = HashMap::new();
    let mut service_client_types: HashMap<usize, Vec<String>> = HashMap::new();

    for (_, sidx) in &rg.service_servers {
        if let Some(svc) = rg.services.get(*sidx) {
            service_server_types
                .entry(*sidx)
                .or_default()
                .push(svc.srv_type.clone());
        }
    }
    for (_, sidx) in &rg.service_clients {
        if let Some(svc) = rg.services.get(*sidx) {
            service_client_types
                .entry(*sidx)
                .or_default()
                .push(svc.srv_type.clone());
        }
    }

    for (sidx, svc) in rg.services.iter().enumerate() {
        let server_types = service_server_types.get(&sidx).cloned().unwrap_or_default();
        let client_types = service_client_types.get(&sidx).cloned().unwrap_or_default();
        if !server_types.is_empty() && !client_types.is_empty() {
            let server_set: std::collections::HashSet<String> = server_types.iter().cloned().collect();
            let client_set: std::collections::HashSet<String> = client_types.iter().cloned().collect();
            if server_set != client_set {
                findings.push(
                    Finding::new(
                        "runtime/service_type_mismatch",
                        Severity::Error,
                        format!(
                            "Service '{}' has server/client type mismatch: servers use {:?}, clients use {:?}",
                            svc.name, server_types, client_types
                        ),
                    )
                    .with_location(Location {
                        package: None,
                        file: None,
                        line: None,
                        context: Some(svc.srv_type.clone()),
                    }),
                );
            }
        }
    }

    findings
}
