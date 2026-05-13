use crate::{
    linter::{LintResult, Rule, RuleMetadata},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "album.requires-disc",
    description: r#"
        If album is present, disc must also be present.

        Valid:
        - album=Album, disc=1/1
        - album missing, disc missing

        Invalid:
        - album=Album, disc missing (album track must declare disc)
    "#,
};

pub struct AlbumRequiresDiscRule;

impl Rule for AlbumRequiresDiscRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        if track.tags.album.is_some() && track.tags.disc.is_none() {
            return self.error("Album is present but disc is missing").into();
        }
        LintResult::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::AlbumRequiresDiscRule;
    use crate::linter::{Rule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(AlbumRequiresDiscRule.check(&make_track()).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.disc = None;
        assert_eq!(AlbumRequiresDiscRule.check(&track).violations().len(), 1);
    }
}
