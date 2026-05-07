use crate::{
    rule_metadata,
    linter::{RuleMetadata, RuleViolation, TrackRule},
    track::Track,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.publisher-catalog-pairing",
    description: r#"
        Publisher and catalog_id usually appear together. The two directions
        carry different weight:

        - A publisher without a catalog_id is a soft signal — the catalog
          number may legitimately be unknown. Warn so it surfaces for review.
        - A catalog_id without a publisher is a hard error — an orphan
          catalog number with no label is a data-integrity hole.

        Valid:
        - publisher=Label, catalog_id=RLS001
        - both missing

        Invalid:
        - publisher=Label, catalog_id missing  (warn)
        - catalog_id=RLS001, publisher missing  (error)
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
            (Some(_), None) => vec![self.warn("Publisher is present but catalog_id is missing")],
            (None, Some(_)) => vec![self.error("Catalog_id is present but publisher is missing")],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MetaPublisherCatalogPairingRule;
    use crate::linter::{RuleSeverity, TrackRule, test_utils::make_track};

    #[test]
    fn ok_both_present() {
        assert!(
            MetaPublisherCatalogPairingRule
                .check(&make_track())
                .is_empty()
        );
    }

    #[test]
    fn ok_both_missing() {
        let mut track = make_track();
        track.tags.publisher = None;
        track.tags.catalog_id = None;
        assert!(MetaPublisherCatalogPairingRule.check(&track).is_empty());
    }

    #[test]
    fn warn_publisher_without_catalog() {
        let mut track = make_track();
        track.tags.catalog_id = None;
        let violations = MetaPublisherCatalogPairingRule.check(&track);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, RuleSeverity::Warn);
    }

    #[test]
    fn error_catalog_without_publisher() {
        let mut track = make_track();
        track.tags.publisher = None;
        let violations = MetaPublisherCatalogPairingRule.check(&track);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, RuleSeverity::Error);
    }

    #[test]
    fn warn_whitespace_catalog() {
        let mut track = make_track();
        track.tags.catalog_id = Some("   ".to_string());
        let violations = MetaPublisherCatalogPairingRule.check(&track);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, RuleSeverity::Warn);
    }
}
