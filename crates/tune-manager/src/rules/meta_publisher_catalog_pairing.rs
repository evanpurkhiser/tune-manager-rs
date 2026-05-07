use crate::{
    rule_metadata,
    rules::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.publisher-catalog-pairing",
    description: r#"
        Publisher and catalog_id must be coupled.

        Valid:
        - publisher=Label, catalog_id=RLS001
        - publisher=Label, catalog_id=--

        Invalid:
        - publisher=Label, catalog_id missing (expected -- when unknown)
        - catalog_id=RLS001, publisher missing
    "#,
};

pub struct MetaPublisherCatalogPairingRule;

impl TrackRule for MetaPublisherCatalogPairingRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, track: &Track) -> Vec<RuleViolation> {
        let publisher = track
            .tags
            .publisher
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let catalog_id = track
            .tags
            .catalog_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());

        match (publisher, catalog_id) {
            (Some(_), None) => {
                vec![self.error("Publisher is present but catalog_id is missing (expected --)")]
            }
            (Some(_), Some("--")) => vec![],
            (None, Some(_)) => vec![self.error("Catalog_id is present but publisher is missing")],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MetaPublisherCatalogPairingRule;
    use crate::rules::{TrackRule, test_utils::make_track};

    #[test]
    fn ok_case() {
        assert!(
            MetaPublisherCatalogPairingRule
                .check(&make_track())
                .is_empty()
        );
    }

    #[test]
    fn fail_case() {
        let mut track = make_track();
        track.tags.catalog_id = None;
        assert_eq!(MetaPublisherCatalogPairingRule.check(&track).len(), 1);
    }

    #[test]
    fn ok_sentinel_case() {
        let mut track = make_track();
        track.tags.catalog_id = Some("--".to_string());
        assert!(MetaPublisherCatalogPairingRule.check(&track).is_empty());
    }

    #[test]
    fn fail_whitespace_catalog() {
        let mut track = make_track();
        track.tags.catalog_id = Some("   ".to_string());
        assert_eq!(MetaPublisherCatalogPairingRule.check(&track).len(), 1);
    }
}
