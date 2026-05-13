use crate::linter::violation::RuleViolation;

/// Outcome of running a single rule. `Skipped` distinguishes "rule had
/// nothing to say" (`Passed`) from "rule couldn't run because it lacked
/// the inputs it needs" — important for honest reporting once rules
/// take a shared `LintContext` and some rules only apply when certain
/// services or per-track data are available.
#[derive(Debug, Clone)]
pub enum LintResult {
    Passed,
    Violations(Vec<RuleViolation>),
    Skipped { reason: String },
}

impl LintResult {
    /// Build a `Skipped` result with a free-form reason.
    pub fn skipped(reason: impl Into<String>) -> Self {
        Self::Skipped {
            reason: reason.into(),
        }
    }

    /// Violations produced by this run. Returns an empty slice for
    /// `Passed` and `Skipped`.
    pub fn violations(&self) -> &[RuleViolation] {
        match self {
            Self::Violations(v) => v,
            _ => &[],
        }
    }

    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped { .. })
    }
}

/// Lets rule bodies that build a `Vec<RuleViolation>` end with
/// `violations.into()`. An empty vec collapses to `Passed`.
impl From<Vec<RuleViolation>> for LintResult {
    fn from(violations: Vec<RuleViolation>) -> Self {
        if violations.is_empty() {
            Self::Passed
        } else {
            Self::Violations(violations)
        }
    }
}

/// Lets rules that produce a single violation end with
/// `self.error("...").into()` (or `self.warn(...).into()`).
impl From<RuleViolation> for LintResult {
    fn from(violation: RuleViolation) -> Self {
        Self::Violations(vec![violation])
    }
}
