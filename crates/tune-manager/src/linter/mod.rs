pub mod engine;
pub mod lint_target;
pub mod result;
pub mod rule;
pub mod rules;
pub mod text_field;
pub mod violation;

#[cfg(test)]
pub mod test_utils;

#[allow(unused_imports)]
pub use engine::{AutofixOutcome, LintEngine};
pub use lint_target::LintTarget;
pub use result::{CheckOutcome, RuleResult};
pub use rule::{Rule, RuleMetadata};
pub use rules::all_rules;
pub use text_field::TextField;
#[allow(unused_imports)]
pub use violation::{Fix, RuleSeverity, RuleViolation};
