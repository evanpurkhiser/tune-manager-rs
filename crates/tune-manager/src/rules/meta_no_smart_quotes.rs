use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.no-smart-quotes",
    description: r#"
        Text metadata must not contain smart quote characters.

        Valid:
        - Don't Stop
        - Artist "Name"

        Invalid:
        - Don’t Stop (contains curly apostrophe)
        - Artist “Name” (contains curly quotes)
    "#,
};

const SMART_QUOTES: [char; 6] = ['“', '”', '‘', '’', '´', '`'];

pub struct MetaNoSmartQuotesRule;

impl TrackRule for MetaNoSmartQuotesRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let fields = [
            track.tags.artist.as_deref(),
            track.tags.title.as_deref(),
            track.tags.album.as_deref(),
            track.tags.remixer.as_deref(),
            track.tags.publisher.as_deref(),
        ];

        if fields
            .into_iter()
            .flatten()
            .any(|v| v.chars().any(|c| SMART_QUOTES.contains(&c)))
        {
            return vec![self.error("Smart quotes are not allowed")];
        }

        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::MetaNoSmartQuotesRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(MetaNoSmartQuotesRule.check(&make_track()).is_empty());
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.title = Some("Don’t Stop".to_string());
        assert_eq!(MetaNoSmartQuotesRule.check(&track).len(), 1);
    }
}
