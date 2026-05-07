use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    linter::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.remixer-title-consistency",
    description: r#"
        Remixer field and title remix note must agree.

        Valid:
        - title=Song (Remixer Remix), remixer=Remixer
        - title=Song, remixer missing

        Invalid:
        - title=Song (Other Remix), remixer=Remixer (name mismatch)
        - title=Song (Remixer Remix), remixer missing (missing remixer field)

        Autofix: when the title carries a remix note we trust the title as
        authoritative — the remixer field is rewritten by extracting the
        artist name from the title's `(Artist Remix)` group. We do not go
        the other direction (writing a remix note into the title from the
        remixer field) because that requires deciding suffix style and
        placement, which isn't always clear cut.
    "#,
};

static REMIX_ARTIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\(([^\)]+?)\s+remix\)").unwrap());

pub struct MetaRemixerTitleConsistencyRule;

impl TrackRule for MetaRemixerTitleConsistencyRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let title = track.tags.title.as_deref().unwrap_or_default();
        let remixer = track
            .tags
            .remixer
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let title_artist = REMIX_ARTIST_RE
            .captures(title)
            .map(|c| c[1].trim().to_string());

        match (remixer, title_artist.as_deref()) {
            (None, Some(_)) => vec![
                self.error("Title has remix signal but remixer field is empty")
                    .with_fix(extract_remixer_from_title),
            ],
            (Some(_), None) => {
                vec![self.error("Remixer is set but title has no remix signal")]
            }
            (Some(remixer), Some(t_artist)) if remixer != t_artist => vec![
                self.error("Remixer field does not match title remix note")
                    .with_fix(extract_remixer_from_title),
            ],
            _ => vec![],
        }
    }
}

fn extract_remixer_from_title(track: &mut Track) {
    let Some(title) = track.tags.title.as_deref() else {
        return;
    };
    if let Some(caps) = REMIX_ARTIST_RE.captures(title) {
        track.tags.remixer = Some(caps[1].trim().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::MetaRemixerTitleConsistencyRule;
    use crate::linter::{TrackRule, test_utils::make_track};

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

    #[test]
    fn fail_case_difference() {
        let mut track = make_track();
        track.tags.title = Some("Song (REMIXER Remix)".to_string());
        track.tags.remixer = Some("Remixer".to_string());
        assert_eq!(MetaRemixerTitleConsistencyRule.check(&track).len(), 1);
    }

    #[test]
    fn fix_extracts_artist_when_remixer_empty() {
        let mut track = make_track();
        track.tags.title = Some("Song (Remixer Remix)".to_string());
        track.tags.remixer = None;
        let violations = MetaRemixerTitleConsistencyRule.check(&track);
        assert_eq!(violations.len(), 1);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        assert_eq!(track.tags.remixer.as_deref(), Some("Remixer"));
    }

    #[test]
    fn fix_overwrites_mismatched_remixer() {
        let mut track = make_track();
        track.tags.title = Some("Song (Other Remix)".to_string());
        track.tags.remixer = Some("Remixer".to_string());
        let violations = MetaRemixerTitleConsistencyRule.check(&track);
        assert_eq!(violations.len(), 1);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        assert_eq!(track.tags.remixer.as_deref(), Some("Other"));
    }

    #[test]
    fn no_fix_when_title_has_no_remix_note() {
        // When remixer is set but title has no remix note we don't autofix:
        // adding a remix note into the title is the ambiguous direction.
        let mut track = make_track();
        track.tags.title = Some("Song".to_string());
        track.tags.remixer = Some("Remixer".to_string());
        let violations = MetaRemixerTitleConsistencyRule.check(&track);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix.is_none());
    }

    #[test]
    fn fix_extracts_multi_word_artist() {
        let mut track = make_track();
        track.tags.title = Some("Song (Some Artist Remix)".to_string());
        track.tags.remixer = None;
        let violations = MetaRemixerTitleConsistencyRule.check(&track);
        violations[0].fix.as_ref().unwrap().apply(&mut track);
        assert_eq!(track.tags.remixer.as_deref(), Some("Some Artist"));
    }
}
