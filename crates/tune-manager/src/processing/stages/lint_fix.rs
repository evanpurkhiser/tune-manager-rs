use std::{path::PathBuf, sync::Arc};

use id3::Tag;
use thiserror::Error;
use tracing::info;
use tune_manager_derive::ProcessingError;

use crate::{
    linter::{LintEngine, LintTarget, RuleResult},
    processing::{
        concurrent::{
            self, ConcurrentProcessor, ConcurrentSender, SentItem, concurrent_processor_with_limit,
        },
        stages::ProducesRevision,
        state::{self, TrackRevision},
    },
    track::{Track, TrackFields},
};

/// Errors produced by the lint-fix stage.
///
/// The engine itself is infallible — rules can only `Pass`, produce
/// `Violations`, or `Skip` themselves. The stage's only failure mode is
/// missing input state: there is no prior revision on the incoming tag,
/// which means earlier stages didn't produce one. That shouldn't happen
/// in practice (`PrepareMedia` writes the first revision), so we surface
/// it as a skip rather than a hard error.
#[derive(ProcessingError, Error, Debug)]
pub enum LintFixError {
    #[CausesSkip]
    #[error("no prior track revision on tag")]
    NoRevision,
}

#[derive(Debug)]
pub struct LintFixInput {
    pub file_path: PathBuf,
    pub tag: Tag,
}

/// Outcome of the lint-fix stage.
#[derive(Debug)]
pub struct LintFixResult {
    /// Post-fix track fields, used by `ProducesRevision` as the new
    /// revision.
    pub fields: TrackFields,
    /// Every rule's final outcome after the autofix loop terminated.
    pub results: Vec<RuleResult>,
    /// True when the autofix loop bailed out without converging —
    /// usually a sign of oscillating rules.
    pub hit_max_iterations: bool,
}

type LintFixFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<LintFixResult, LintFixError>> + Send>,
>;
type LintFixProcessFn = Box<dyn Fn(LintFixInput) -> LintFixFuture + Send + Sync>;

pub type LintFixProcessor =
    ConcurrentProcessor<LintFixInput, LintFixResult, LintFixError, LintFixProcessFn, LintFixFuture>;
pub type LintFixSender = ConcurrentSender<LintFixInput, LintFixResult, LintFixError>;
pub type LintFixSentItem = SentItem<LintFixResult, LintFixError>;
pub type ItemStatus = concurrent::ItemStatus<LintFixResult, LintFixError>;

pub fn new_lint_fix_processor() -> LintFixProcessor {
    let engine = Arc::new(LintEngine::new());

    concurrent_processor_with_limit(
        None,
        Box::new(move |input: LintFixInput| {
            let engine = engine.clone();
            Box::pin(async move { process_lint_fix(input, engine).await })
        }),
    )
}

async fn process_lint_fix(
    input: LintFixInput,
    engine: Arc<LintEngine>,
) -> Result<LintFixResult, LintFixError> {
    let LintFixInput { file_path, tag } = input;

    // The latest revision is the source of truth for the current track
    // state — prior stages (Keyfinder, Beatport) have chained their
    // updates into it. Linting the tag's raw frame data instead would
    // see pre-update values.
    let Some(last_revision) = state::get_last_revision(&tag) else {
        return Err(LintFixError::NoRevision);
    };

    let mut target = LintTarget {
        track: Track {
            file: file_path.into(),
            fields: last_revision.fields,
        },
        id3: Some(tag),
    };

    let outcome = engine.run_autofix(&mut target);

    if outcome.hit_max_iterations {
        info!(
            file = ?target.track.file.file_path,
            iterations = outcome.iterations,
            "lint autofix hit max iterations — rules may be oscillating"
        );
    }

    Ok(LintFixResult {
        fields: target.track.fields,
        results: outcome.results,
        hit_max_iterations: outcome.hit_max_iterations,
    })
}

impl ProducesRevision for LintFixResult {
    fn produce_revision(&self, _last_revision: Option<&TrackRevision>) -> Option<TrackRevision> {
        Some(TrackRevision::new(self.fields.clone()))
    }
}
