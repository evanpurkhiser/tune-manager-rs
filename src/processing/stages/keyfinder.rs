use std::path::PathBuf;

use crate::{
    keyfinder::{self, KeyfinderError},
    processing::concurrent::{
        ConcurrentProcessor, ConcurrentSender, SentItem, concurrent_processor_with_limit,
    },
};

/// Maximum number of concurrent keyfinder processes allowed
const KEYFINDER_CONCURRENCY_LIMIT: usize = 12;

#[derive(Debug, Default)]
pub struct KeyfinderInput {
    pub file_path: PathBuf,
}

#[derive(Debug)]
pub struct KeyfinderResult {
    pub detected_key: Option<String>,
}

type KeyfinderFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<KeyfinderResult, KeyfinderError>> + Send>,
>;
type KeyfinderProcessFn = fn(KeyfinderInput) -> KeyfinderFuture;

pub type KeyfinderProcessor = ConcurrentProcessor<
    KeyfinderInput,
    KeyfinderResult,
    KeyfinderError,
    KeyfinderProcessFn,
    KeyfinderFuture,
>;
pub type KeyfinderSender = ConcurrentSender<KeyfinderInput, KeyfinderResult, KeyfinderError>;
pub type KeyfinderSentItem = SentItem<KeyfinderResult, KeyfinderError>;

pub fn new_keyfinder_processor() -> KeyfinderProcessor {
    concurrent_processor_with_limit(
        Some(KEYFINDER_CONCURRENCY_LIMIT),
        |input: KeyfinderInput| {
            let executor = move || {
                keyfinder::detect_key(&input.file_path, keyfinder::KeyNotation::Camelot)
                    .map(|detected_key| KeyfinderResult { detected_key })
            };
            Box::pin(async move { tokio::task::spawn_blocking(executor).await.unwrap() })
        },
    )
}
