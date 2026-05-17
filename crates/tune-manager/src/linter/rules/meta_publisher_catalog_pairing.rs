use crate::{
    linter::{LintResult, LintTarget, Rule, RuleMetadata},
    rule_metadata,
};

static METADATA: RuleMetadata = rule_metadata! {
    id: "meta.publisher-catalog-pairing",
    description: r#"
        Publisher and catalog_id should be coupled.

        Valid:
        - publisher=Label, catalog_id=RLS001
        - both missing

        Invalid:
        - publisher=Label, catalog_id missing (warn — catalog number may legitimately be unknown)
        - catalog_id=RLS001, publisher missing (error — orphan catalog number with no label)
    "#,
};

pub struct MetaPublisherCatalogPairingRule;

impl Rule for MetaPublisherCatalogPairingRule {
    fn metadata(&self) -> &'static RuleMetadata {
        &METADATA
    }

    fn check(&self, target: &LintTarget) -> LintResult {
        let track = &target.track;
        let publisher = track
            .fields
            .publisher
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let catalog_id = track
            .fields
            .catalog_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());

        match (publisher, catalog_id) {
            (Some(_), None) => self
                .warn("Publisher is present but catalog_id is missing")
                .into(),
            (None, Some(_)) => self
                .error("Catalog_id is present but publisher is missing")
                .into(),
            _ => LintResult::Passed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MetaPublisherCatalogPairingRule;
    use crate::linter::{Rule, RuleSeverity, test_utils::make_track};

    #[test]
    fn ok_both_present() {
        assert!(
            MetaPublisherCatalogPairingRule
                .check(&make_track().into())
                .is_passed()
        );
    }

    #[test]
    fn ok_both_missing() {
        let mut track = make_track();
        track.fields.publisher = None;
        track.fields.catalog_id = None;
        assert!(
            MetaPublisherCatalogPairingRule
                .check(&track.into())
                .is_passed()
        );
    }

    #[test]
    fn warn_publisher_without_catalog() {
        let mut track = make_track();
        track.fields.catalog_id = None;
        let result = MetaPublisherCatalogPairingRule.check(&track.into());
        assert_eq!(result.violations().len(), 1);
        assert_eq!(result.violations()[0].severity, RuleSeverity::Warn);
    }

    #[test]
    fn error_catalog_without_publisher() {
        let mut track = make_track();
        track.fields.publisher = None;
        let result = MetaPublisherCatalogPairingRule.check(&track.into());
        assert_eq!(result.violations().len(), 1);
        assert_eq!(result.violations()[0].severity, RuleSeverity::Error);
    }

    #[test]
    fn warn_whitespace_catalog() {
        let mut track = make_track();
        track.fields.catalog_id = Some("   ".to_string());
        let result = MetaPublisherCatalogPairingRule.check(&track.into());
        assert_eq!(result.violations().len(), 1);
        assert_eq!(result.violations()[0].severity, RuleSeverity::Warn);
    }
}
