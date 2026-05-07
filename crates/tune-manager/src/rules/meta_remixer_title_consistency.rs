use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "meta.remixer-title-consistency";
const DESCRIPTION: &str = indoc::indoc! {r#"
Remixer field and title remix note must agree.

Valid:
- title=Song (Remixer Remix), remixer=Remixer
- title=Song, remixer missing

Invalid:
- title=Song (Other Remix), remixer=Remixer (name mismatch)
- title=Song (Remixer Remix), remixer missing (missing remixer field)
"#};

static REMIX_NOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\([^\)]*remix\)").unwrap());

pub struct MetaRemixerTitleConsistencyRule;

impl TrackRule for MetaRemixerTitleConsistencyRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let title = track.tags.title.as_deref().unwrap_or_default();
        let remixer = track
            .tags
            .remixer
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let has_remix_note = REMIX_NOTE_RE.is_match(title);

        if remixer.is_none() && has_remix_note {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Title has remix signal but remixer field is empty",
            )];
        }

        if remixer.is_some() && !has_remix_note {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Remixer is set but title has no remix signal",
            )];
        }

        if let Some(remixer) = remixer
            && has_remix_note
            && !title
                .to_ascii_lowercase()
                .contains(&remixer.to_ascii_lowercase())
        {
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Remixer field does not match title remix note",
            )];
        }

        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::MetaRemixerTitleConsistencyRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let mut track = make_track();
        track.tags.title = Some("Song (Remixer Remix)".to_string());
        track.tags.remixer = Some("Remixer".to_string());
        assert!(MetaRemixerTitleConsistencyRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Song (Other Remix)".to_string());
        track.tags.remixer = Some("Remixer".to_string());
        assert_eq!(MetaRemixerTitleConsistencyRule.check(&track).len(), 1);
    }

    #[test]
    fn fail_inverse_case() {
        let mut track = make_track();
        track.tags.title = Some("Song".to_string());
        track.tags.remixer = Some("Remixer".to_string());
        assert_eq!(MetaRemixerTitleConsistencyRule.check(&track).len(), 1);
    }

    #[test]
    fn ignores_non_remix_mix_token() {
        let mut track = make_track();
        track.tags.title = Some("Song (Extended Mix)".to_string());
        track.tags.remixer = None;
        assert!(MetaRemixerTitleConsistencyRule.check(&track).is_empty());
    }
}
