use crate::{
    fields::CountField,
    linter::{CheckOutcome, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "track.count-format",
    description: r#"
        Track count must be parseable and internally valid when present.

        Valid:
        - 01/10
        - 1/1

        Invalid:
        - abc (not parseable)
        - 12/10 (track number greater than total)
    "#,
};

pub struct TrackCountFormatRule;

impl Rule for TrackCountFormatRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> CheckOutcome {
        let track = &target.track;
        match track.fields.track.as_ref() {
            None => CheckOutcome::Passed,
            Some(CountField::Invalid(_)) => self.error("Track number format is invalid").into(),
            Some(CountField::Valid(c)) if c.number == 0 || c.total == 0 || c.number > c.total => {
                self.error("Track number is out of valid range").into()
            }
            Some(CountField::Valid(_)) => CheckOutcome::Passed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TrackCountFormatRule;
    use crate::{
        fields::CountField,
        linter::{Rule, test_utils::make_track},
    };

    #[test]
    fn ok_case() {
        assert!(
            TrackCountFormatRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.fields.track = Some(CountField::Invalid("x".to_string()));
        assert_eq!(
            TrackCountFormatRule
                .check(&track.into())
                .violations()
                .len(),
            1
        );
    }
}
