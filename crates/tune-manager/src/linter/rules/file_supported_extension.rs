use std::collections::HashSet;

use crate::{
    linter::{LintResult, RuleMetadata, TrackRule},
    rule_metadata,
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "file.supported-extension",
    description: r#"
        File extension must be in the supported audio file set.

        Valid:
        - Artist - Title.mp3
        - Artist - Title.aif

        Invalid:
        - Artist - Title.ogg (unsupported extension)
        - Artist - Title (missing extension)
    "#,
};

pub struct FileSupportedExtensionRule;

impl TrackRule for FileSupportedExtensionRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> LintResult {
        let supported: HashSet<&str> = ["mp3", "aiff"].into_iter().collect();
        let ext = track
            .metadata
            .file_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());

        match ext {
            Some(ext) if supported.contains(ext.as_str()) => LintResult::Passed,
            _ => self.error("File extension is not supported").into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::linter::{TrackRule, test_utils::make_track};

    use super::FileSupportedExtensionRule;

    #[test]
    fn ok_case() {
        let track = make_track();
        assert!(FileSupportedExtensionRule.check(&track).is_passed());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.metadata.file_path = PathBuf::from("x/test.ogg");
        assert_eq!(
            FileSupportedExtensionRule.check(&track).violations().len(),
            1
        );
    }
}
