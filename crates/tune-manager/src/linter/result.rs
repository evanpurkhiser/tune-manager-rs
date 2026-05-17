use crate::linter::violation::RuleViolation;

/// Outcome of running a single rule's [`check`](crate::linter::Rule::check).
/// `Skipped` distinguishes "rule had nothing to say" (`Passed`) from
/// "rule couldn't run because it lacked the inputs it needs" — important
/// for honest reporting once rules take a shared context and some rules
/// only apply when certain services or per-track data are available.
///
/// `CheckOutcome` itself does not carry the rule's id; the engine pairs
/// it with the originating rule via [`RuleResult`].
#[derive(Debug, Clone)]
pub enum CheckOutcome {
    Passed,
    Violations(Vec<RuleViolation>),
    Skipped { reason: String },
}

impl CheckOutcome {
    /// Build a `Skipped` outcome with a free-form reason.
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
impl From<Vec<RuleViolation>> for CheckOutcome {
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
impl From<RuleViolation> for CheckOutcome {
    fn from(violation: RuleViolation) -> Self {
        Self::Violations(vec![violation])
    }
}

/// A single rule's [`CheckOutcome`] paired with the rule that produced
/// it. The engine returns `Vec<RuleResult>` so callers can attribute
/// every outcome — pass, skip, or violation — back to its rule without
/// requiring [`RuleViolation`] to carry the id redundantly.
#[derive(Debug, Clone)]
pub struct RuleResult {
    pub rule_id: &'static str,
    pub outcome: CheckOutcome,
}
