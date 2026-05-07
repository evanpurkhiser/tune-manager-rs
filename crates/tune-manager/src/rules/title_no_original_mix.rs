use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "title.no-original-mix",
    description: r#"
        Title must not include an Original Mix marker.

        Valid:
        - Song
        - Song (Artist Remix)

        Invalid:
        - Song (Original Mix) (original mix label should be removed)
        - Song (ORIGINAL MIX) (case-insensitive match)
    "#,
};

static ORIGINAL_MIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*\(original mix\)\s*").unwrap());

pub struct TitleNoOriginalMixRule;

impl TrackRule for TitleNoOriginalMixRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(title) = track.tags.title.as_deref() else {
            return vec![];
        };
        if !ORIGINAL_MIX_RE.is_match(title) {
            return vec![];
        }
        vec![
            self.error("Title should not include (Original Mix)")
                .with_fix(|track| {
                    if let Some(title) = track.tags.title.as_deref() {
                        track.tags.title = Some(strip_original_mix(title));
                    }
                }),
        ]
    }
}

fn strip_original_mix(title: &str) -> String {
    ORIGINAL_MIX_RE.replace_all(title, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::TitleNoOriginalMixRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(TitleNoOriginalMixRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Song (Original Mix)".to_string());
        assert_eq!(TitleNoOriginalMixRule.check(&track).len(), 1);
    }

    fn fixed_title(input: &str) -> String {
        let mut track = make_track();
        track.tags.title = Some(input.to_string());
        let violations = TitleNoOriginalMixRule.check(&track);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        track.tags.title.unwrap()
    }

    #[test]
    fn fix_trailing_suffix() {
        assert_eq!(fixed_title("Song (Original Mix)"), "Song");
    }

    #[test]
    fn fix_case_insensitive() {
        assert_eq!(fixed_title("Song (ORIGINAL MIX)"), "Song");
    }

    #[test]
    fn fix_preserves_surrounding_words() {
        assert_eq!(
            fixed_title("Song (Original Mix) [Extended]"),
            "Song [Extended]"
        );
    }

    #[test]
    fn fix_leading_suffix() {
        assert_eq!(fixed_title("(Original Mix) Song"), "Song");
    }
}
