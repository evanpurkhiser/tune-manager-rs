use regex::Regex;
use std::sync::LazyLock;

use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "key.canonical-camelot",
    description: r#"
        Key must use canonical Camelot notation.

        Valid:
        - 01A
        - 12B

        Invalid:
        - 1A (not zero padded)
        - Am (not Camelot format)
    "#,
};

static KEY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(0[1-9]|1[0-2])[AB]$").unwrap());

pub struct KeyCanonicalCamelotRule;

impl TrackRule for KeyCanonicalCamelotRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let Some(key) = track.tags.key.as_deref() else {
            return vec![];
        };
        if KEY_RE.is_match(key.trim()) {
            return vec![];
        }
        vec![self.error("Key is not canonical Camelot (01A..12B)")]
    }
}

#[cfg(test)]
mod tests {
    use super::KeyCanonicalCamelotRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        let track = make_track();
        assert!(KeyCanonicalCamelotRule.check(&track).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.key = Some("1A".to_string());
        assert_eq!(KeyCanonicalCamelotRule.check(&track).len(), 1);
    }
}
