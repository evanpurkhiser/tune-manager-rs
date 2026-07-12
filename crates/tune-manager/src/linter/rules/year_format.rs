use regex::Regex;
use std::sync::LazyLock;

use crate::{
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "year.format",
    description: r#"
        Year must be a four-digit numeric value.

        Valid:
        - 2015
        - 1999

        Invalid:
        - 15 (too short)
        - 20A5 (non-numeric)
    "#,
};

static YEAR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}$").unwrap());

pub struct YearFormatRule;

impl Rule for YearFormatRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        let Some(year) = track.fields.year.as_deref() else {
            return CheckOutcome::Passed;
        };

        if YEAR_RE.is_match(year) {
            return CheckOutcome::Passed;
        }
        self.error("Year must be in YYYY format").into()
    }
}

#[cfg(test)]
mod tests {
    use super::YearFormatRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(YearFormatRule.check(&make_track().into()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.year = Some("15".to_string());
        assert_eq!(YearFormatRule.check(&track.into()).violations().len(), 1);
    }

    #[test]
    fn fail_case_with_whitespace() {
        let mut track = make_track();
        track.fields.year = Some(" 2015 ".to_string());
        assert_eq!(YearFormatRule.check(&track.into()).violations().len(), 1);
    }
}
