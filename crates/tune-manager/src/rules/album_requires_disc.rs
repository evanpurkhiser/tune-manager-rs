use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "album.requires-disc";
const DESCRIPTION: &str = indoc::indoc! {r#"
If album is present, disc must also be present.

Valid:
- album=Album, disc=1/1
- album missing, disc missing

Invalid:
- album=Album, disc missing (album track must declare disc)
"#};

pub struct AlbumRequiresDiscRule;

impl TrackRule for AlbumRequiresDiscRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        if track.tags.album.is_some() && track.tags.disc.is_none() {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Album is present but disc is missing",
            )];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::AlbumRequiresDiscRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(AlbumRequiresDiscRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.disc = None;
        assert_eq!(AlbumRequiresDiscRule.check(&track).len(), 1);
    }
}
