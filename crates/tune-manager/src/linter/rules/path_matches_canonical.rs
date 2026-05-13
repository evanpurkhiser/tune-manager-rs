use crate::{
    linter::{LintResult, RuleMetadata, TrackRule},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "path.matches-canonical",
    description: r#"
        Path must exactly match the canonical path generated from track tags.

        Valid:
        - Publisher/[RLS] Album/01. [10A] Artist - Title.mp3
        - Publisher/[+singles]/[10A] Artist - Title.mp3

        Invalid:
        - Publisher/[RLS] Album/99. [10A] Artist - Title.mp3 (wrong track number)
        - WrongPublisher/[RLS] Album/01. [10A] Artist - Title.mp3 (non-canonical directory)
    "#,
};

pub struct PathMatchesCanonicalRule;

impl TrackRule for PathMatchesCanonicalRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        let canonical = track.cononical_path();
        let actual = &track.metadata.file_path;

        if actual.ends_with(&canonical) {
            return LintResult::Passed;
        }

        self.error(format!(
            "Path does not match canonical path: {}",
            canonical.display()
        ))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::linter::{TrackRule, test_utils::make_track};

    use super::PathMatchesCanonicalRule;

    #[test]
    fn ok_case() {
        let track = make_track();
        assert!(PathMatchesCanonicalRule.check(&track).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.metadata.file_path =
            PathBuf::from("Publisher/[RLS] Album/Disc 2/99. [10A] Artist - Title.MP3");
        assert_eq!(PathMatchesCanonicalRule.check(&track).violations().len(), 1);
    }
}
