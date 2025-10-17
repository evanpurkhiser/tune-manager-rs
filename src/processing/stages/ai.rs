use std::{
    io,
    sync::{Arc, OnceLock},
};

use async_openai::{Client, config::OpenAIConfig};
use thiserror::Error;
use tracing::debug;

use crate::{
    ai,
    app::config::AiConfig,
    processing::concurrent::{
        ConcurrentProcessor, ConcurrentSender, SentItem, concurrent_processor_with_limit,
    },
    track::{Track, TrackRevision},
};

/// Maximum number of concurrent AI API requests allowed
const AI_CONCURRENCY_LIMIT: usize = 4;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("OpenAI not configured")]
    NotConfigured,

    #[error("AI processing error: {0}")]
    Processing(#[from] ai::AiError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug)]
pub struct AiInput {
    pub tracks: Vec<Track>,
}

#[derive(Debug)]
pub struct AiResult {
    pub responses: Vec<ai::TrackResponse>,
}

type AiFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<AiResult, AiError>> + Send>>;
type AiProcessFn = Box<dyn Fn(AiInput) -> AiFuture + Send + Sync>;

pub type AiProcessor = ConcurrentProcessor<AiInput, AiResult, AiError, AiProcessFn, AiFuture>;
pub type AiSender = ConcurrentSender<AiInput, AiResult, AiError>;
pub type AiSentItem = SentItem<AiResult, AiError>;

pub fn new_ai_processor(ai_config: Option<&AiConfig>) -> AiProcessor {
    let token = ai_config.map(|config| Arc::new(config.token.clone()));
    let client = Arc::new(OnceLock::<Client<OpenAIConfig>>::new());

    concurrent_processor_with_limit(
        Some(AI_CONCURRENCY_LIMIT),
        Box::new(move |input: AiInput| {
            let client_cell = client.clone();
            let token = token.clone();
            Box::pin(async move { process_ai_input(input, client_cell, token).await })
        }),
    )
}

async fn process_ai_input(
    input: AiInput,
    client_cell: Arc<OnceLock<Client<OpenAIConfig>>>,
    token: Option<Arc<String>>,
) -> Result<AiResult, AiError> {
    let Some(token) = token else {
        return Err(AiError::NotConfigured);
    };

    let ai_client = client_cell.get_or_init(|| {
        debug!("Initializing OpenAI client");
        Client::with_config(OpenAIConfig::new().with_api_key(token.as_ref()))
    });

    let response = ai::process_tracks(ai_client, input.tracks)
        .await
        .map_err(AiError::Processing)?;

    Ok(AiResult {
        responses: response.tracks,
    })
}

pub fn produce_revision(last_revision: &TrackRevision, result: &AiResult) -> TrackRevision {
    let mut revision = last_revision.clone();
    if let Some(track_response) = result.responses.first() {
        track_response.update_track_tags(&mut revision.tags);
    }
    TrackRevision::new(revision.tags)
}
