use std::{io, path::PathBuf, sync::Arc};

use id3::Tag;
use thiserror::Error;
use tokio::sync::OnceCell;
use tracing::debug;
use tune_manager_derive::ProcessingError;

use crate::{
    app::config::BeatportConfig,
    processing::{
        concurrent::{
            self, ConcurrentProcessor, ConcurrentSender, SentItem, concurrent_processor_with_limit,
        },
        stages::ProducesRevision,
        state::TrackRevision,
    },
    services::beatport::{
        Authenticated, BeatportApiError, BeatportCredentials, BeatportSource, BeatportTrackInfo,
        try_extract_track_id, try_extract_url,
    },
};

/// Maximum number of concurrent beatport API requests allowed
const BEATPORT_CONCURRENCY_LIMIT: usize = 12;

#[derive(ProcessingError, Error, Debug)]
pub enum BeatportError {
    #[CausesSkip]
    #[error("Beatport not configured")]
    NotConfigured,

    #[CausesSkip]
    #[error("No Beatport URL found in tag")]
    NoUrl,

    #[error("Invalid Beatport URL format")]
    InvalidUrl,

    #[error("Beatport API error")]
    Api(#[from] BeatportApiError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug)]
pub struct BeatportInput {
    pub file_path: PathBuf,
    pub tag: Tag,
}

#[derive(Debug)]
pub struct BeatportResult {
    pub track_info: Option<BeatportTrackInfo>,
}

type BeatportFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<BeatportResult, BeatportError>> + Send>,
>;
type BeatportProcessFn = Box<dyn Fn(BeatportInput) -> BeatportFuture + Send + Sync>;

pub type BeatportProcessor = ConcurrentProcessor<
    BeatportInput,
    BeatportResult,
    BeatportError,
    BeatportProcessFn,
    BeatportFuture,
>;
pub type BeatportSender = ConcurrentSender<BeatportInput, BeatportResult, BeatportError>;
pub type BeatportSentItem = SentItem<BeatportResult, BeatportError>;
pub type ItemStatus = concurrent::ItemStatus<BeatportResult, BeatportError>;

pub fn new_beatport_processor(beatport_config: Option<&BeatportConfig>) -> BeatportProcessor {
    let credentials = beatport_config.map(|config| {
        Arc::new(BeatportCredentials {
            username: config.username.clone(),
            password: config.password.clone(),
        })
    });

    let authenticated_client = Arc::new(OnceCell::<BeatportSource<Authenticated>>::new());

    concurrent_processor_with_limit(
        Some(BEATPORT_CONCURRENCY_LIMIT),
        Box::new(move |input: BeatportInput| {
            let client_cell = authenticated_client.clone();
            let creds = credentials.clone();
            Box::pin(async move { process_beatport_input(input, client_cell, creds).await })
        }),
    )
}

async fn process_beatport_input(
    input: BeatportInput,
    client_cell: Arc<OnceCell<BeatportSource<Authenticated>>>,
    credentials: Option<Arc<BeatportCredentials>>,
) -> Result<BeatportResult, BeatportError> {
    let Some(credentials) = credentials else {
        return Err(BeatportError::NotConfigured);
    };

    let Some(url) = try_extract_url(&input.tag) else {
        return Err(BeatportError::NoUrl);
    };

    debug!("Found Beatport URL: {}", url);

    let Some(track_id) = try_extract_track_id(&url) else {
        debug!("Could not extract track ID from Beatport URL");
        return Err(BeatportError::InvalidUrl);
    };

    debug!("Extracted Beatport track ID: {}", track_id);

    let authenticated_source = client_cell
        .get_or_try_init(move || async move {
            debug!("Authenticating with Beatport");
            BeatportSource::new()
                .authenticate(credentials.as_ref())
                .await
        })
        .await
        .map_err(BeatportError::Api)?;

    let track_info = authenticated_source
        .fetch_track_info(track_id)
        .await
        .map_err(BeatportError::Api)?;

    Ok(BeatportResult {
        track_info: Some(track_info),
    })
}

impl ProducesRevision for BeatportResult {
    fn produce_revision(&self, last_revision: Option<&TrackRevision>) -> Option<TrackRevision> {
        let last_revision = last_revision?;
        let mut revision = last_revision.clone();
        if let Some(ref track_info) = self.track_info {
            track_info.update_track_tags(&mut revision.tags);
        }
        Some(TrackRevision::new(revision.tags))
    }
}
