pub mod result;
pub mod rule;
pub mod rules;
pub mod text_field;
pub mod violation;

#[cfg(test)]
pub mod test_utils;

pub use result::LintResult;
pub use rule::{RuleMetadata, TrackRule};
pub use text_field::TextField;
// Public surface used only via `linter::*` paths once the engine is wired.
// Re-export now so callers don't need to know the internal module split.
#[allow(unused_imports)]
pub use rules::track_only_rules;
#[allow(unused_imports)]
pub use violation::{Fix, RuleSeverity, RuleViolation};
