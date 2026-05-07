use crate::{
    fields::CountField,
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
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

impl TrackRule for TrackCountFormatRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        match track.tags.track.as_ref() {
            None => vec![],
            Some(CountField::Invalid(_)) => vec![self.error("Track number format is invalid")],
            Some(CountField::Valid(c)) if c.number == 0 || c.total == 0 || c.number > c.total => {
                vec![self.error("Track number is out of valid range")]
            }
            Some(CountField::Valid(_)) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TrackCountFormatRule;
    use crate::{
        fields::CountField,
        rules::{TrackRule, test_utils::make_track},
    };

    #[test]
    fn ok_case() {
        assert!(TrackCountFormatRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.track = Some(CountField::Invalid("x".to_string()));
        assert_eq!(TrackCountFormatRule.check(&track).len(), 1);
    }
}
