use crate::{
    rules::{RuleSeverity, RuleViolation, TrackRule, violation},
    track::Track,
};

const RULE_ID: &str = "meta.no-smart-quotes";
const DESCRIPTION: &str = indoc::indoc! {r#"
Text metadata must not contain smart quote characters.

Valid:
- Don't Stop
- Artist "Name"

Invalid:
- Don’t Stop (contains curly apostrophe)
- Artist “Name” (contains curly quotes)
"#};

const SMART_QUOTES: [char; 6] = ['“', '”', '‘', '’', '´', '`'];

pub struct MetaNoSmartQuotesRule;

impl TrackRule for MetaNoSmartQuotesRule {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        DESCRIPTION
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
            return vec![violation(
                RULE_ID,
                RuleSeverity::Warn,
                "Smart quotes are not allowed",
            )];
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
