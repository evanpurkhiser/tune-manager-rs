//! Concurrent processing framework for handling async operations with configurable concurrency.
//!
//! This module provides a generic concurrent processor pattern that handles async functions
//! through tokio streams and channels. It supports configurable concurrency limits and yields
//! status updates as work moves through the processing pipeline (Waiting → Running → Complete).
//! Designed for I/O-bound or CPU-intensive async operations where progress visibility is important.

use std::future::Future;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

/// Status of an item being processed.
///
/// Represents the current state of work submitted to a concurrent processor.
#[derive(Debug)]
pub enum ItemStatus<Output, Error> {
    /// The work is queued but not yet started
    Waiting,
    /// The work is currently being processed
    Running,
    /// The work was skipped (e.g., stage not configured)
    Skipped(String),
    /// The work has completed with a result
    Complete(Result<Output, Error>),
}

type ProcessorMessage<Input, Output, Error> =
    (Input, mpsc::UnboundedSender<ItemStatus<Output, Error>>);

/// A concurrent processor that handles async operations with configurable concurrency.
///
/// This processor wraps an async function and provides a concurrent execution framework
/// using tokio streams and channels. Work items are sent through a cloneable sender,
/// processed concurrently with a configurable limit, and results are returned
/// through async channels.
///
/// # Type Parameters
///
/// * `Input` - The input type for the processing function
/// * `Output` - The output type returned by successful processing
/// * `Error` - The error type returned by failed processing
/// * `ProcessFn` - The processing function type that takes Input and returns a Future
///
/// # Example
///
/// ```rust,ignore
/// // Create a processor for async file operations with concurrency limit of 4
/// let processor = concurrent_processor_with_limit(4, |path: PathBuf| async move {
///     tokio::fs::read_to_string(path).await.map_err(|e| e.into())
/// });
///
/// // Get a sender (can be cloned and shared)
/// let sender = processor.get_sender();
///
/// // Start processing in background
/// tokio::spawn(processor.start());
///
/// // Send work and await results
/// let result = sender.send(PathBuf::from("file.txt")).result().await;
/// ```
pub struct ConcurrentProcessor<Input, Output, Error, ProcessFn, Fut>
where
    ProcessFn: Fn(Input) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Output, Error>> + Send + 'static,
    Input: Send + 'static,
    Output: Send + 'static,
    Error: Send + 'static,
{
    tx: Arc<mpsc::UnboundedSender<ProcessorMessage<Input, Output, Error>>>,
    rx: mpsc::UnboundedReceiver<ProcessorMessage<Input, Output, Error>>,
    process_fn: ProcessFn,
    concurrency_limit: Option<usize>,
}

/// A cloneable sender for submitting work to a concurrent processor.
///
/// This sender can be cloned and shared across multiple threads or tasks.
/// Each call to `send()` submits work to the processor and returns a handle
/// that can be awaited for the result.
///
/// # Type Parameters
///
/// * `Input` - The input type accepted by the processor
/// * `Output` - The output type returned by successful processing
/// * `Error` - The error type returned by failed processing
pub struct ConcurrentSender<Input, Output, Error> {
    tx: Arc<mpsc::UnboundedSender<ProcessorMessage<Input, Output, Error>>>,
}

impl<Input, Output, Error> Clone for ConcurrentSender<Input, Output, Error> {
    fn clone(&self) -> Self {
        ConcurrentSender {
            tx: Arc::clone(&self.tx),
        }
    }
}

/// A handle representing work that has been sent to a concurrent processor.
///
/// This handle can be used to monitor the status of the processing operation,
/// including whether it's waiting, running, or complete.
///
/// # Type Parameters
///
/// * `Output` - The output type returned by successful processing
/// * `Error` - The error type returned by failed processing
pub struct SentItem<Output, Error> {
    status_rx: mpsc::UnboundedReceiver<ItemStatus<Output, Error>>,
}

impl<Input, Output, Error> ConcurrentSender<Input, Output, Error> {
    /// Send work to the concurrent processor.
    ///
    /// This method submits input to the processor for processing and returns
    /// a handle that can be used to monitor the processing status.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data to be processed
    ///
    /// # Returns
    ///
    /// A [`SentItem`] handle that can be used to monitor processing status.
    ///
    /// # Panics
    ///
    /// Panics if the processor has been dropped and the channel is closed.
    pub fn send(&self, input: Input) -> SentItem<Output, Error> {
        let (status_tx, status_rx) = mpsc::unbounded_channel();

        // Send initial Waiting status
        let _ = status_tx.send(ItemStatus::Waiting);

        self.tx.send((input, status_tx)).unwrap();
        SentItem { status_rx }
    }

    /// Create a SentItem that immediately reports as skipped.
    ///
    /// This is useful for stages that are not configured or don't need to run.
    /// The returned SentItem will immediately report a Skipped status.
    ///
    /// # Returns
    ///
    /// A [`SentItem`] handle that immediately reports Skipped status.
    pub fn send_skipped(&self, reason: String) -> SentItem<Output, Error> {
        let (status_tx, status_rx) = mpsc::unbounded_channel();

        // Send Skipped status and close the channel
        let _ = status_tx.send(ItemStatus::Skipped(reason));
        drop(status_tx);

        SentItem { status_rx }
    }
}

impl<Output, Error> SentItem<Output, Error> {
    /// Wait for the next status update.
    ///
    /// This method returns each status in sequence: Waiting -> Running -> Complete.
    /// It will never skip status updates.
    ///
    /// # Returns
    ///
    /// The next [`ItemStatus`], or `None` if the processor was dropped.
    pub async fn next_status(&mut self) -> Option<ItemStatus<Output, Error>> {
        self.status_rx.recv().await
    }

    /// Wait for the processing to complete and return the final result.
    ///
    /// This is a convenience method that consumes all status updates until
    /// it receives [`ItemStatus::Complete`], then returns the result.
    ///
    /// # Returns
    ///
    /// The final result of the processing operation.
    ///
    /// # Panics
    ///
    /// Panics if the processor was dropped before completing.
    pub async fn result(mut self) -> Result<Output, Error> {
        while let Some(status) = self.next_status().await {
            if let ItemStatus::Complete(result) = status {
                return result;
            }
        }
        panic!("Processor was dropped before completing")
    }
}

impl<Input, Output, Error, ProcessFn, Fut> ConcurrentProcessor<Input, Output, Error, ProcessFn, Fut>
where
    ProcessFn: Fn(Input) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Output, Error>> + Send + 'static,
    Input: Send + 'static,
    Output: Send + 'static,
    Error: Send + 'static,
{
    /// Create a new concurrent processor with the given processing function and concurrency limit.
    ///
    /// # Arguments
    ///
    /// * `process_fn` - An async function that takes input and returns a Future<Output = Result<Output, Error>>
    /// * `concurrency_limit` - Optional limit on the number of concurrent operations (None for unlimited)
    ///
    /// # Returns
    ///
    /// A new [`ConcurrentProcessor`] instance ready to process work.
    pub fn new(process_fn: ProcessFn, concurrency_limit: Option<usize>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        ConcurrentProcessor {
            tx: Arc::new(tx),
            rx,
            process_fn,
            concurrency_limit,
        }
    }

    /// Get a cloneable sender for submitting work to this processor.
    ///
    /// The returned sender can be cloned and shared across multiple threads
    /// or tasks. Each sender can submit work independently.
    ///
    /// # Returns
    ///
    /// A [`ConcurrentSender`] that can be used to submit work to this processor.
    pub fn get_sender(&self) -> ConcurrentSender<Input, Output, Error> {
        ConcurrentSender {
            tx: Arc::clone(&self.tx),
        }
    }

    /// Start the concurrent processor.
    ///
    /// This method consumes the processor and starts processing work items
    /// concurrently. It will run until all senders are dropped and no more
    /// work is available.
    ///
    /// # Important
    ///
    /// This method should typically be called within `tokio::spawn()` to run
    /// the processor in the background.
    pub async fn start(self) {
        let stream: UnboundedReceiverStream<_> = self.rx.into();
        let process_fn = Arc::new(self.process_fn);

        stream
            .for_each_concurrent(self.concurrency_limit, move |(input, status_tx)| {
                let process_fn = Arc::clone(&process_fn);
                async move {
                    // Update status to Running
                    let _ = status_tx.send(ItemStatus::Running);

                    // Process the input
                    let result = process_fn(input).await;

                    // Update status to Complete with result
                    let _ = status_tx.send(ItemStatus::Complete(result));
                }
            })
            .await;
    }
}

/// Create a concurrent processor with a specific concurrency limit and automatic type inference.
///
/// This function creates a `ConcurrentProcessor` with a configurable concurrency limit.
/// The concurrency limit controls how many operations can run simultaneously.
///
/// # Arguments
///
/// * `concurrency_limit` - Optional maximum number of concurrent operations (None for unlimited)
/// * `process_fn` - An async function that takes input and returns a Future<Output = Result<Output, Error>>
///
/// # Returns
///
/// A new [`ConcurrentProcessor`] instance configured with the specified concurrency limit.
///
/// # Example
///
/// ```rust,ignore
/// // Limit to 4 concurrent file operations
/// let processor = concurrent_processor_with_limit(Some(4), |path: PathBuf| async move {
///     tokio::task::spawn_blocking(move || {
///         std::fs::read_to_string(path).map_err(|e| e.into())
///     }).await.unwrap()
/// });
/// ```
pub fn concurrent_processor_with_limit<Input, Output, Error, ProcessFn, Fut>(
    concurrency_limit: Option<usize>,
    process_fn: ProcessFn,
) -> ConcurrentProcessor<Input, Output, Error, ProcessFn, Fut>
where
    ProcessFn: Fn(Input) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Output, Error>> + Send + 'static,
    Input: Send + 'static,
    Output: Send + 'static,
    Error: Send + 'static,
{
    ConcurrentProcessor::new(process_fn, concurrency_limit)
}
