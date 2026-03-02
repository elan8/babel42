//! Check findings model — severity, location, fix hints.

use crate::project_config::Babel42Config;
use serde::{Deserialize, Serialize};

/// Severity level for a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// Location of a finding (package, file, optional line).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Location {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// A single finding from a check rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_hint: Option<String>,
}

impl Finding {
    pub fn new(rule_id: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            message: message.into(),
            location: None,
            fix_hint: None,
        }
    }

    pub fn with_location(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_fix_hint(mut self, fix_hint: impl Into<String>) -> Self {
        self.fix_hint = Some(fix_hint.into());
        self
    }
}

/// Which rule sets to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleSet {
    #[default]
    All,
    Dep,
    Launch,
    Runtime,
    Manifest,
}

impl RuleSet {
    pub fn includes_dep(&self) -> bool {
        matches!(self, RuleSet::All | RuleSet::Dep)
    }
    pub fn includes_launch(&self) -> bool {
        matches!(self, RuleSet::All | RuleSet::Launch)
    }
    pub fn includes_runtime(&self) -> bool {
        matches!(self, RuleSet::All | RuleSet::Runtime)
    }
    pub fn includes_manifest(&self) -> bool {
        matches!(self, RuleSet::All | RuleSet::Manifest)
    }
}

impl std::str::FromStr for RuleSet {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(RuleSet::All),
            "dep" => Ok(RuleSet::Dep),
            "launch" => Ok(RuleSet::Launch),
            "runtime" => Ok(RuleSet::Runtime),
            "manifest" => Ok(RuleSet::Manifest),
            _ => Err(format!("Unknown rule set: {}", s)),
        }
    }
}

/// Options for running checks.
#[derive(Debug, Clone)]
pub struct CheckOpts {
    /// Minimum severity to include (Info, Warn, or Error)
    pub min_severity: Severity,
    /// Which rule sets to run
    pub rule_set: RuleSet,
    /// Fail (exit 1) when findings >= this severity exist
    pub fail_on: FailOn,
    /// Optional project config from .babel42.yaml (skip_packages, etc.)
    pub config: Option<Babel42Config>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailOn {
    None,
    Info,
    Warn,
    Error,
}

impl std::str::FromStr for FailOn {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(FailOn::None),
            "info" => Ok(FailOn::Info),
            "warn" => Ok(FailOn::Warn),
            "error" => Ok(FailOn::Error),
            _ => Err(format!("Unknown fail-on: {}", s)),
        }
    }
}

impl FailOn {
    pub fn should_fail(&self, has_error: bool, has_warn: bool, has_info: bool) -> bool {
        match self {
            FailOn::None => false,
            FailOn::Error => has_error,
            FailOn::Warn => has_error || has_warn,
            FailOn::Info => has_error || has_warn || has_info,
        }
    }
}

impl Default for CheckOpts {
    fn default() -> Self {
        Self {
            min_severity: Severity::Warn,
            rule_set: RuleSet::All,
            fail_on: FailOn::Error,
            config: None,
        }
    }
}
