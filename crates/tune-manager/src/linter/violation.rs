use std::sync::Arc;

use crate::track::Track;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSeverity {
    /// A hard violation that must be resolved before a track is allowed to
    /// belong in the catalog.
    Error,
    /// A soft signal that may not require action — surfaced for review but
    /// does not block acceptance.
    Warn,
}

/// A single violation produced by a rule. The originating rule is tracked
/// by [`RuleResult`](crate::linter::RuleResult), which wraps the
/// [`CheckOutcome`](crate::linter::CheckOutcome) carrying these violations
/// — so this struct intentionally does not carry `rule_id`.
#[derive(Debug, Clone)]
pub struct RuleViolation {
    pub severity: RuleSeverity,
    pub message: String,
    /// Optional autofix that mutates a [`Track`] to resolve this violation.
    /// Absent when the violation cannot be safely fixed without human input.
    pub fix: Option<Fix>,
}

impl RuleViolation {
    /// Attach an autofix closure to this violation. The closure receives the
    /// current track state (which may already reflect prior fixes from the
    /// same pass) and mutates it in place.
    pub fn with_fix<F>(mut self, fix: F) -> Self
    where
        F: Fn(&mut Track) + Send + Sync + 'static,
    {
        self.fix = Some(Fix(Arc::new(fix)));
        self
    }
}

/// A deferred mutation that resolves a [`RuleViolation`]. Wrapped so the
/// engine can chain multiple fixes against the same field without needing
/// each rule to know what others might have rewritten — every closure reads
/// the current track state when it runs.
#[derive(Clone)]
pub struct Fix(Arc<dyn Fn(&mut Track) + Send + Sync>);

impl Fix {
    pub fn apply(&self, track: &mut Track) {
        (self.0)(track);
    }
}

impl std::fmt::Debug for Fix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Fix(<closure>)")
    }
}
