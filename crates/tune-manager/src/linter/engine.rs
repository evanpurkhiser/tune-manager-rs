use crate::linter::{LintTarget, Rule, RuleResult, all_rules};

const DEFAULT_MAX_ITERATIONS: u32 = 5;

/// Runs a set of [`Rule`](crate::linter::Rule)s against a
/// [`LintTarget`](crate::linter::LintTarget), optionally applying autofixes
/// in a bounded loop until either no fixable violations remain or
/// `max_iterations` is exceeded.
///
/// The iteration cap exists to break pathological oscillation: a fix from
/// rule A can re-trigger rule B, whose fix re-triggers A. Without a cap the
/// loop would not terminate.
pub struct LintEngine {
    rules: Vec<Box<dyn Rule>>,
    max_iterations: u32,
}

impl LintEngine {
    /// Construct an engine preloaded with [`all_rules`](crate::linter::all_rules)
    /// and the default iteration cap. Use [`with_rules`](Self::with_rules)
    /// to override the rule set (e.g. for tests).
    pub fn new() -> Self {
        Self {
            rules: all_rules(),
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }

    pub fn with_rules(mut self, rules: Vec<Box<dyn Rule>>) -> Self {
        self.rules = rules;
        self
    }

    pub fn with_max_iterations(mut self, n: u32) -> Self {
        self.max_iterations = n;
        self
    }

    /// Run every rule once against `target`. Read-only — no fixes are
    /// applied even if violations carry them.
    pub fn check(&self, target: &LintTarget) -> Vec<RuleResult> {
        self.rules
            .iter()
            .map(|rule| RuleResult {
                rule_id: rule.metadata().id,
                outcome: rule.check(target),
            })
            .collect()
    }

    /// Iteratively apply autofixes until no fixable violations remain or
    /// the iteration cap is reached. Mutates `target.track` in place.
    ///
    /// `target.id3`, if present, is *not* updated as fixes apply — fix
    /// closures only touch `Track`. Rules that consult the raw tag during
    /// the loop see the original frames, not a tag reflecting accumulated
    /// fixes.
    pub fn run_autofix(&self, target: &mut LintTarget) -> AutofixOutcome {
        for iteration in 0..self.max_iterations {
            let results = self.check(target);
            let fixes: Vec<_> = results
                .iter()
                .flat_map(|r| r.outcome.violations())
                .filter_map(|v| v.fix.as_ref())
                .collect();

            if fixes.is_empty() {
                return AutofixOutcome {
                    results,
                    iterations: iteration,
                    hit_max_iterations: false,
                };
            }

            fixes.iter().for_each(|fix| fix.apply(&mut target.track));
        }

        // Cap hit: re-check so the returned results reflect the post-fix
        // state from the final iteration.
        AutofixOutcome {
            results: self.check(target),
            iterations: self.max_iterations,
            hit_max_iterations: true,
        }
    }
}

impl Default for LintEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of [`LintEngine::run_autofix`].
#[derive(Debug)]
pub struct AutofixOutcome {
    /// Per-rule results after the loop terminated. Any remaining
    /// `Violations` could not be resolved — either because they had no
    /// `Fix`, or because the engine hit `max_iterations` before
    /// converging.
    pub results: Vec<RuleResult>,

    /// Number of fix-application rounds that ran. Zero means the initial
    /// check found nothing fixable.
    pub iterations: u32,

    /// True when the loop exited because it hit `max_iterations` with
    /// fixes still being produced — usually a sign of oscillating rules.
    pub hit_max_iterations: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linter::{
        LintTarget, Rule,
        test_utils::{
            AlwaysFailNoFix, AlwaysPass, AlwaysSkip, OscToA, OscToB, StepA, StepB, make_track,
        },
    };

    fn engine_with(rules: Vec<Box<dyn Rule>>) -> LintEngine {
        LintEngine::new().with_rules(rules)
    }

    fn target_with_title(title: &str) -> LintTarget {
        let mut track = make_track();
        track.fields.title = Some(title.to_string());
        track.into()
    }

    #[test]
    fn check_returns_one_result_per_rule_in_order() {
        let engine = engine_with(vec![
            Box::new(AlwaysPass),
            Box::new(AlwaysFailNoFix),
            Box::new(AlwaysSkip),
        ]);
        let results = engine.check(&make_track().into());
        assert_eq!(
            results.iter().map(|r| r.rule_id).collect::<Vec<_>>(),
            vec![
                "test.always-pass",
                "test.always-fail-no-fix",
                "test.always-skip"
            ]
        );
        assert!(results[0].outcome.is_passed());
        assert_eq!(results[1].outcome.violations().len(), 1);
        assert!(results[2].outcome.is_skipped());
    }

    #[test]
    fn autofix_zero_iterations_when_all_pass() {
        let engine = engine_with(vec![Box::new(AlwaysPass)]);
        let mut target = make_track().into();
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 0);
        assert!(!outcome.hit_max_iterations);
        assert!(outcome.results[0].outcome.is_passed());
    }

    #[test]
    fn autofix_zero_iterations_when_no_fixes_available() {
        let engine = engine_with(vec![Box::new(AlwaysFailNoFix)]);
        let mut target = make_track().into();
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 0);
        assert!(!outcome.hit_max_iterations);
        assert_eq!(outcome.results[0].outcome.violations().len(), 1);
    }

    #[test]
    fn autofix_converges_in_one_iteration_for_single_fix() {
        let engine = engine_with(vec![Box::new(StepA), Box::new(StepB)]);
        let mut target = target_with_title("middle");
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 1);
        assert!(!outcome.hit_max_iterations);
        assert_eq!(target.track.fields.title.as_deref(), Some("end"));
        assert!(outcome.results.iter().all(|r| r.outcome.is_passed()));
    }

    #[test]
    fn autofix_converges_across_chain_of_fixes() {
        // start -> middle (iter 0) -> end (iter 1) -> all pass (iter 2 check)
        let engine = engine_with(vec![Box::new(StepA), Box::new(StepB)]);
        let mut target = target_with_title("start");
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 2);
        assert!(!outcome.hit_max_iterations);
        assert_eq!(target.track.fields.title.as_deref(), Some("end"));
    }

    #[test]
    fn autofix_hits_max_iterations_on_oscillation() {
        let engine = engine_with(vec![Box::new(OscToA), Box::new(OscToB)]);
        let mut target = target_with_title("a");
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 5);
        assert!(outcome.hit_max_iterations);
        // Results reflect post-final-fix state, so a violation should remain.
        assert!(
            outcome
                .results
                .iter()
                .any(|r| !r.outcome.violations().is_empty())
        );
    }

    #[test]
    fn autofix_with_max_iterations_override() {
        let engine = engine_with(vec![Box::new(OscToA), Box::new(OscToB)]).with_max_iterations(1);
        let mut target = target_with_title("a");
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 1);
        assert!(outcome.hit_max_iterations);
    }

    #[test]
    fn autofix_carries_skipped_results_through() {
        let engine = engine_with(vec![Box::new(AlwaysSkip), Box::new(StepA)]);
        let mut target = target_with_title("start");
        let outcome = engine.run_autofix(&mut target);
        assert_eq!(outcome.iterations, 1);
        let skip = outcome
            .results
            .iter()
            .find(|r| r.rule_id == "test.always-skip")
            .unwrap();
        assert!(skip.outcome.is_skipped());
    }

    #[test]
    fn autofix_leaves_unfixable_violations_in_final_results() {
        let engine = engine_with(vec![Box::new(StepA), Box::new(AlwaysFailNoFix)]);
        let mut target = target_with_title("start");
        let outcome = engine.run_autofix(&mut target);
        // StepA fixes its own violation; AlwaysFailNoFix keeps failing
        // forever, but since it produces no fix the loop terminates.
        assert!(!outcome.hit_max_iterations);
        let step_a = outcome
            .results
            .iter()
            .find(|r| r.rule_id == "test.step-a")
            .unwrap();
        let no_fix = outcome
            .results
            .iter()
            .find(|r| r.rule_id == "test.always-fail-no-fix")
            .unwrap();
        assert!(step_a.outcome.is_passed());
        assert_eq!(no_fix.outcome.violations().len(), 1);
    }
}
