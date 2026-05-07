use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "artist.separator-structure",
    description: r#"
        Artist separators must form a clean structure with no malformed
        whitespace or duplicated separators.

        Hygiene:
        - No leading or trailing whitespace or separators.
        - No duplicate or doubled separators.
        - No double-spaces.

        Any mix of canonical separators (`,`, `vs`, `&`) is allowed; choice of
        separator is left to the writer.

        Valid:
        - A & B
        - A vs B
        - A, B, C
        - Technikore vs Dougal & Gammer
        - Aly & Fila, Lostly

        Invalid:
        - A & & B (duplicate separator)
        - A ,  B (bad spacing)
    "#,
};

static HYGIENE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\s,)|(,\s*,)|(&\s*&)|(\s{2,})|(^[,&\s]+)|([,&\s]+$)").unwrap());

// TODO: apply this rule to the `remixer` field too. The cleanest path is a
// shared `ArtistField` selector enum and registering this rule twice.
pub struct ArtistSeparatorStructureRule;

impl TrackRule for ArtistSeparatorStructureRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(artist) = track.tags.artist.as_deref() else {
            return vec![];
        };
        if HYGIENE_RE.is_match(artist) {
            return vec![self.error("Artist separator hygiene is invalid")];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::ArtistSeparatorStructureRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    fn check_with(artist: &str) -> Vec<crate::rules::RuleViolation> {
        let mut track = make_track();
        track.tags.artist = Some(artist.to_string());
        ArtistSeparatorStructureRule.check(&track)
    }

    #[test]
    fn ok_single_artist() {
        assert!(check_with("A").is_empty());
    }

    #[test]
    fn ok_two_with_ampersand() {
        assert!(check_with("A & B").is_empty());
    }

    #[test]
    fn ok_two_with_vs() {
        assert!(check_with("A vs B").is_empty());
    }

    #[test]
    fn ok_two_with_comma() {
        assert!(check_with("A, B").is_empty());
    }

    #[test]
    fn ok_three_with_commas() {
        assert!(check_with("A, B, C").is_empty());
    }

    #[test]
    fn ok_three_mixed_amp_and_comma() {
        assert!(check_with("Aly & Fila, Lostly").is_empty());
    }

    #[test]
    fn ok_three_mixed_vs_and_amp() {
        assert!(check_with("Technikore vs Dougal & Gammer").is_empty());
    }

    #[test]
    fn fail_hygiene_double_space() {
        assert_eq!(check_with("A ,  B").len(), 1);
    }

    #[test]
    fn fail_hygiene_doubled_separator() {
        assert_eq!(check_with("A & & B").len(), 1);
    }

    #[test]
    fn fail_hygiene_leading_separator() {
        assert_eq!(check_with(", A & B").len(), 1);
    }
}
