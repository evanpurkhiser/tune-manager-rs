use crate::{
    linter::violation::{RuleSeverity, RuleViolation},
    track::Track,
};

/// Static metadata describing a rule. Each rule defines a single `METADATA`
/// constant via the [`rule_metadata!`] macro and exposes it through
/// [`TrackRule::metadata`].
#[derive(Debug)]
pub struct RuleMetadata {
    pub id: &'static str,
    pub description: &'static str,
}

/// Construct a [`RuleMetadata`] in `static` position. The description is run
/// through [`indoc::indoc!`] so rule files can write naturally-indented prose
/// without escaping leading whitespace.
#[macro_export]
macro_rules! rule_metadata {
    (id: $id:literal, description: $description:literal $(,)?) => {
        $crate::linter::RuleMetadata {
            id: $id,
            description: ::indoc::indoc!($description),
        }
    };
}

pub trait TrackRule: Send + Sync {
    fn metadata(&self) -> &'static RuleMetadata;

    fn check(&self, track: &Track) -> Vec<RuleViolation>;

    /// Build an `Error`-severity violation tagged with this rule's id.
    fn error(&self, message: impl Into<String>) -> RuleViolation
    where
        Self: Sized,
    {
        RuleViolation {
            rule_id: self.metadata().id,
            severity: RuleSeverity::Error,
            message: message.into(),
            fix: None,
        }
    }

    /// Build a `Warn`-severity violation tagged with this rule's id.
    fn warn(&self, message: impl Into<String>) -> RuleViolation
    where
        Self: Sized,
    {
        RuleViolation {
            rule_id: self.metadata().id,
            severity: RuleSeverity::Warn,
            message: message.into(),
            fix: None,
        }
    }
}
