//! Validation report types.

use serde::{Deserialize, Serialize};

/// Severity classification.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Soft, advisory; safe to commit.
    Info,
    /// Likely an issue but commit is allowed.
    Warning,
    /// Hard violation; commit should be rejected.
    Error,
}

/// One validation finding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationFinding {
    /// Severity.
    pub severity: Severity,
    /// Short machine-readable code, e.g. `"cardinality.required"`.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional pointer to the offending entity.
    pub focus: Option<String>,
}

impl ValidationFinding {
    /// Build an error.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: Severity::Error, code: code.into(), message: message.into(), focus: None }
    }

    /// Build a warning.
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, code: code.into(), message: message.into(), focus: None }
    }

    /// Attach a focus pointer.
    pub fn focus(mut self, focus: impl Into<String>) -> Self {
        self.focus = Some(focus.into());
        self
    }
}

/// Aggregate report.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Findings.
    pub findings: Vec<ValidationFinding>,
}

impl ValidationReport {
    /// Push a finding.
    pub fn push(&mut self, finding: ValidationFinding) {
        self.findings.push(finding);
    }

    /// Append another report.
    pub fn extend(&mut self, other: ValidationReport) {
        self.findings.extend(other.findings);
    }

    /// `true` when there are no `Error` findings.
    pub fn is_clean(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Iterator over errors.
    pub fn errors(&self) -> impl Iterator<Item = &ValidationFinding> {
        self.findings.iter().filter(|f| f.severity == Severity::Error)
    }
}
