use crate::{
    fields::CountField,
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
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

impl TrackRule for DiscCountFormatRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        match track.tags.disc.as_ref() {
            None => vec![],
            Some(CountField::Invalid(_)) => vec![self.error("Disc number format is invalid")],
            Some(CountField::Valid(c)) if c.number == 0 || c.total == 0 || c.number > c.total => {
                vec![self.error("Disc number is out of valid range")]
            }
            Some(CountField::Valid(_)) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DiscCountFormatRule;
    use crate::{
        fields::CountField,
        rules::{TrackRule, test_utils::make_track},
    };

    #[test]
    fn ok_case() {
        assert!(DiscCountFormatRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.disc = Some(CountField::Invalid("x".to_string()));
        assert_eq!(DiscCountFormatRule.check(&track).len(), 1);
    }
}
