use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.split-token-hygiene",
    description: r#"
        Artist connector syntax must be clean with no dangling or duplicated
        separators.

        Valid:
        - A & B
        - A, B & C

        Invalid:
        - A ,  B (bad spacing)
        - A & & B (duplicate separator)
    "#,
};

static BAD_HYGIENE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\s,)|(,\s*,)|(&\s*&)|(\s{2,})|(^[,&\s]+)|([,&\s]+$)").unwrap());

pub struct ArtistSplitTokenHygieneRule;

impl TrackRule for ArtistSplitTokenHygieneRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(artist) = track.tags.artist.as_deref() else {
            return vec![];
        };
        if BAD_HYGIENE_RE.is_match(artist) {
            return vec![self.error("Artist connector hygiene is invalid")];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::ArtistSplitTokenHygieneRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.tags.artist = Some("A & B".to_string());
        assert!(ArtistSplitTokenHygieneRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.artist = Some("A ,  B".to_string());
        assert_eq!(ArtistSplitTokenHygieneRule.check(&track).len(), 1);
    }
}
