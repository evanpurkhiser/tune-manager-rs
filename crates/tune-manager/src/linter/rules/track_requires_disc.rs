use crate::{
    linter::{LintResult, RuleMetadata, TrackRule},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "track.requires-disc",
    description: r#"
        If track is present, disc must also be present.

        Valid:
        - track=01/10, disc=1/1
        - track missing, disc missing

        Invalid:
        - track=01/10, disc missing (track count requires disc context)
    "#,
};

pub struct TrackRequiresDiscRule;

impl TrackRule for TrackRequiresDiscRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        if track.tags.track.is_some() && track.tags.disc.is_none() {
            return self.error("Track is present but disc is missing").into();
        }
        LintResult::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::TrackRequiresDiscRule;
    use crate::linter::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(TrackRequiresDiscRule.check(&make_track()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.disc = None;
        assert_eq!(TrackRequiresDiscRule.check(&track).violations().len(), 1);
    }
}
