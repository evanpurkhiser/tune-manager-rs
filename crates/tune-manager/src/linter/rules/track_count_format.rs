use crate::{
    fields::CountField,
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
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

    fn check(&self, track: &Track) -> LintResult {
        match track.tags.track.as_ref() {
            None => LintResult::Passed,
            Some(CountField::Invalid(_)) => self.error("Track number format is invalid").into(),
            Some(CountField::Valid(c)) if c.number == 0 || c.total == 0 || c.number > c.total => {
                self.error("Track number is out of valid range").into()
            }
            Some(CountField::Valid(_)) => LintResult::Passed,
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
        assert!(TrackCountFormatRule.check(&make_track()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.track = Some(CountField::Invalid("x".to_string()));
        assert_eq!(TrackCountFormatRule.check(&track).violations().len(), 1);
    }
}
