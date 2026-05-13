use crate::{
    linter::{
        result::LintResult,
        violation::{RuleSeverity, RuleViolation},
    },
    track::Track,
};

/// Static metadata describing a rule. Each rule defines a single `METADATA`
/// constant via the [`rule_metadata!`] macro and exposes it through
/// [`TrackRule::metadata`].
///
/// `description` is the rule itself — what's checked, what's valid, what's
/// invalid. `autofix_notes` describes what the rule's autofix does (when it
/// has one). They're kept separate so consumers like the AI cleanup pass
/// can include rule descriptions in their prompts without dragging in
/// autofix mechanics, which aren't actionable on the model's side.
#[derive(Debug)]
pub struct RuleMetadata {
    pub id: &'static str,
    pub description: &'static str,
    pub autofix_notes: Option<&'static str>,
}

/// Construct a [`RuleMetadata`] in `static` position. The description and
/// autofix notes are run through [`indoc::indoc!`] so rule files can write
/// naturally-indented prose without escaping leading whitespace.
#[macro_export]
macro_rules! rule_metadata {
    (id: $id:literal, description: $description:literal $(,)?) => {
        $crate::linter::RuleMetadata {
            id: $id,
            description: ::indoc::indoc!($description),
            autofix_notes: None,
        }
    };
    (
        id: $id:literal,
        description: $description:literal,
        autofix_notes: $autofix_notes:literal $(,)?
    ) => {
        $crate::linter::RuleMetadata {
            id: $id,
            description: ::indoc::indoc!($description),
            autofix_notes: Some(::indoc::indoc!($autofix_notes)),
        }
    };
}

pub trait Rule: Send + Sync {
    fn metadata(&self) -> &'static RuleMetadata;

    fn check(&self, track: &Track) -> LintResult;

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
