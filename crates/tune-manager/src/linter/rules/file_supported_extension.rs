use std::collections::HashSet;

use crate::{
    rule_metadata,
    linter::{RuleMetadata, RuleViolation, TrackRule},
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

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let supported: HashSet<&str> = ["mp3", "aiff"].into_iter().collect();
        let ext = track
            .metadata
            .file_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());

        match ext {
            Some(ext) if supported.contains(ext.as_str()) => vec![],
            _ => vec![self.error("File extension is not supported")],
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
        let violations = FileSupportedExtensionRule.check(&track);
        assert!(violations.is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.metadata.file_path = PathBuf::from("x/test.ogg");
        let violations = FileSupportedExtensionRule.check(&track);
        assert_eq!(violations.len(), 1);
    }
}
