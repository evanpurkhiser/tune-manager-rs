use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "path.matches-canonical";
const DESCRIPTION: &str = indoc::indoc! {r#"
Path must exactly match the canonical path generated from track tags.

Valid:
- Publisher/[RLS] Album/01. [10A] Artist - Title.mp3
- Publisher/[+singles]/[10A] Artist - Title.mp3

Invalid:
- Publisher/[RLS] Album/99. [10A] Artist - Title.mp3 (wrong track number)
- WrongPublisher/[RLS] Album/01. [10A] Artist - Title.mp3 (non-canonical directory)
"#};

pub struct PathMatchesCanonicalRule;

impl TrackRule for PathMatchesCanonicalRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let canonical = track.cononical_path();
        let actual = &track.metadata.file_path;

        if actual.ends_with(&canonical) {
            return vec![];
        }

        vec![violation(
            RULE_ID,
            RuleSeverity::Error,
            format!(
                "Path does not match canonical path: {}",
                canonical.display()
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::rules::{TrackRule, test_utils::make_track};

    use super::PathMatchesCanonicalRule;

    #[test]
    fn ok_case() {
        let track = make_track();
        let violations = PathMatchesCanonicalRule.check(&track);
        assert!(violations.is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.metadata.file_path =
            PathBuf::from("Publisher/[RLS] Album/Disc 2/99. [10A] Artist - Title.MP3");
        let violations = PathMatchesCanonicalRule.check(&track);
        assert_eq!(violations.len(), 1);
    }
}
