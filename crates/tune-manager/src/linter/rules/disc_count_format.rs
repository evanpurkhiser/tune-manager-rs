use crate::{
    fields::CountField,
    linter::{LintResult, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "disc.count-format",
    description: r#"
        Disc count must be parseable and internally valid when present.

        Valid:
        - 01/02
        - 1/1

        Invalid:
        - x/y (not parseable)
        - 3/2 (disc number greater than total)
    "#,
};

pub struct DiscCountFormatRule;

impl Rule for DiscCountFormatRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        match track.fields.disc.as_ref() {
            None => LintResult::Passed,
            Some(CountField::Invalid(_)) => self.error("Disc number format is invalid").into(),
            Some(CountField::Valid(c)) if c.number == 0 || c.total == 0 || c.number > c.total => {
                self.error("Disc number is out of valid range").into()
            }
            Some(CountField::Valid(_)) => LintResult::Passed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiscCountFormatRule;
    use crate::{
        fields::CountField,
        linter::{Rule, test_utils::make_track},
    };

    #[test]
    fn ok_case() {
        assert!(
            DiscCountFormatRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.disc = Some(CountField::Invalid("x".to_string()));
        assert_eq!(
            DiscCountFormatRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
