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

use crate::processing::error::ProcessingError;

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

/// A trait for abstracting over different types of sent items from concurrent processors.
///
/// This trait provides a common interface for monitoring the status of work submitted
/// to any concurrent processor, regardless of the specific input/output types.
/// It enables generic monitoring functions that can work with any stage's sent items.
///
/// The trait is automatically implemented for all `SentItem<Output, Error>` types
/// through a blanket implementation, so stage-specific types like `PrepareMediaSentItem`,
/// `KeyfinderSentItem`, etc. automatically support this trait.
///
/// # Type Parameters
///
/// * `ItemStatus` - The status type returned by this sent item (e.g., `ItemStatus<Output, Error>`)
/// ```
pub trait SentItemLike {
    type SentItemStatus;

    async fn next_status(&mut self) -> Option<Self::SentItemStatus>;
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

impl<Output, Error> SentItemLike for SentItem<Output, Error> {
    type SentItemStatus = ItemStatus<Output, Error>;

    async fn next_status(&mut self) -> Option<Self::SentItemStatus> {
        self.next_status().await
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
    pub async fn start(self)
    where
        Error: ProcessingError,
    {
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

                    // Check if error causes skip
                    let status = match &result {
                        Err(e) if e.causes_skip() => ItemStatus::Skipped(e.to_string()),
                        _ => ItemStatus::Complete(result),
                    };

                    let _ = status_tx.send(status);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::oneshot;
    use tune_manager_derive::ProcessingError;

    // Test error type that implements ProcessingError
    #[derive(Debug, Clone, ProcessingError, thiserror::Error)]
    enum TestError {
        #[error("Error: {0}")]
        Regular(String),

        #[CausesSkip]
        #[error("Skipped: {0}")]
        Skip(String),
    }

    // Helper to collect all statuses from a SentItem
    async fn collect_statuses<Output, Error>(
        sent_item: &mut SentItem<Output, Error>,
    ) -> Vec<ItemStatus<Output, Error>> {
        let mut statuses = Vec::new();
        while let Some(status) = sent_item.next_status().await {
            statuses.push(status);
        }
        statuses
    }

    #[tokio::test]
    async fn test_basic_successful_processing() {
        let processor = concurrent_processor_with_limit(Some(1), |x: i32| async move {
            Ok::<i32, TestError>(x * 2)
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let result = sender.send(5).result().await;
        assert_eq!(result.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let processor = concurrent_processor_with_limit(Some(1), |x: i32| async move {
            if x < 0 {
                Err(TestError::Regular("negative number".to_string()))
            } else {
                Ok(x * 2)
            }
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let result = sender.send(-5).result().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TestError::Regular(ref r) if r == "negative number"));
    }

    #[tokio::test]
    async fn test_skip_error_handling() {
        let processor = concurrent_processor_with_limit(Some(1), |x: i32| async move {
            if x == 0 {
                Err(TestError::Skip("zero not allowed".to_string()))
            } else {
                Ok(x * 2)
            }
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let mut sent_item = sender.send(0);

        let statuses = collect_statuses(&mut sent_item).await;

        // Should receive Waiting, Running, then Skipped
        assert_eq!(statuses.len(), 3);
        assert!(matches!(statuses[0], ItemStatus::Waiting));
        assert!(matches!(statuses[1], ItemStatus::Running));
        assert!(matches!(&statuses[2], ItemStatus::Skipped(r) if r == "Skipped: zero not allowed"));
    }

    #[tokio::test]
    async fn test_send_skipped() {
        let processor = concurrent_processor_with_limit(Some(1), |x: i32| async move {
            Ok::<i32, TestError>(x * 2)
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let mut sent_item = sender.send_skipped("stage not configured".to_string());

        // Should only receive Skipped status
        let status = sent_item.next_status().await.unwrap();
        assert!(matches!(status, ItemStatus::Skipped(r) if r == "stage not configured"));

        // No more statuses
        assert!(sent_item.next_status().await.is_none());
    }

    #[tokio::test]
    async fn test_status_progression() {
        let processor = concurrent_processor_with_limit(Some(1), |x: i32| async move {
            // Yield to ensure status updates are sent
            tokio::task::yield_now().await;
            Ok::<i32, TestError>(x * 2)
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let mut sent_item = sender.send(5);

        let statuses = collect_statuses(&mut sent_item).await;

        // Should receive exactly: Waiting -> Running -> Complete
        assert_eq!(statuses.len(), 3);
        assert!(matches!(statuses[0], ItemStatus::Waiting));
        assert!(matches!(statuses[1], ItemStatus::Running));
        assert!(matches!(statuses[2], ItemStatus::Complete(Ok(10))));
    }

    #[tokio::test]
    async fn test_multiple_work_items() {
        let processor = concurrent_processor_with_limit(Some(4), |x: i32| async move {
            Ok::<i32, TestError>(x * 2)
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push((i, sender.send(i)));
        }

        // Verify all results
        for (input, handle) in handles {
            let result = handle.result().await.unwrap();
            assert_eq!(result, input * 2);
        }
    }

    #[tokio::test]
    async fn test_mixed_success_and_error() {
        let processor = concurrent_processor_with_limit(Some(2), |x: i32| async move {
            if x % 2 == 0 {
                Ok(x * 2)
            } else {
                Err(TestError::Regular(format!("odd number: {}", x)))
            }
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        let mut handles = Vec::new();
        for i in 0..10 {
            handles.push((i, sender.send(i)));
        }

        // Verify results
        for (input, handle) in handles {
            let result = handle.result().await;
            if input % 2 == 0 {
                assert_eq!(result.unwrap(), input * 2);
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[tokio::test]
    async fn test_cloneable_sender() {
        let processor = concurrent_processor_with_limit(Some(2), |x: i32| async move {
            Ok::<i32, TestError>(x * 2)
        });

        let sender1 = processor.get_sender();
        let sender2 = sender1.clone();
        let sender3 = sender2.clone();

        tokio::spawn(processor.start());

        // Send from different senders
        let result1 = sender1.send(1).result().await.unwrap();
        let result2 = sender2.send(2).result().await.unwrap();
        let result3 = sender3.send(3).result().await.unwrap();

        assert_eq!(result1, 2);
        assert_eq!(result2, 4);
        assert_eq!(result3, 6);
    }

    #[tokio::test]
    async fn test_order_preservation_within_limit() {
        // Track the order in which items complete
        let completion_order = Arc::new(Mutex::new(Vec::new()));
        let completion_order_clone = Arc::clone(&completion_order);

        let processor = concurrent_processor_with_limit(Some(1), move |x: i32| {
            let order = Arc::clone(&completion_order_clone);
            async move {
                order.lock().unwrap().push(x);
                Ok::<i32, TestError>(x)
            }
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        // Send items
        let mut handles = Vec::new();
        for i in 0..5 {
            handles.push(sender.send(i));
        }

        // Wait for completion
        for handle in handles {
            handle.result().await.unwrap();
        }

        // With concurrency limit of 1, order should be preserved
        let order = completion_order.lock().unwrap();
        assert_eq!(*order, vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_concurrency_limit() {
        test_concurrency_behavior(Some(2), 5, 1, 2).await;
    }

    #[tokio::test]
    async fn test_unlimited_concurrency() {
        test_concurrency_behavior(None, 20, 10, 20).await;
    }

    // Helper to test concurrency limits by tracking active task count
    async fn test_concurrency_behavior(
        limit: Option<usize>,
        num_items: usize,
        expected_min: usize,
        expected_max: usize,
    ) {
        let active_count = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let active_count_clone = Arc::clone(&active_count);
        let max_concurrent_clone = Arc::clone(&max_concurrent);

        let processor = concurrent_processor_with_limit(limit, move |rx: oneshot::Receiver<()>| {
            let active_count = Arc::clone(&active_count_clone);
            let max_concurrent = Arc::clone(&max_concurrent_clone);

            async move {
                // Increment active count
                let current = active_count.fetch_add(1, Ordering::SeqCst) + 1;

                // Update max if needed
                max_concurrent.fetch_max(current, Ordering::SeqCst);

                // Wait for signal to complete
                let _ = rx.await;

                // Decrement active count
                active_count.fetch_sub(1, Ordering::SeqCst);

                Ok::<i32, TestError>(0)
            }
        });

        let sender = processor.get_sender();
        tokio::spawn(processor.start());

        // Send items with oneshot receivers
        let items: Vec<_> = (0..num_items)
            .map(|_| {
                let (tx, rx) = oneshot::channel();
                (sender.send(rx), tx)
            })
            .collect();

        // Give tasks time to start
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // Check concurrency behavior
        let max = max_concurrent.load(Ordering::SeqCst);
        assert!(
            max >= expected_min,
            "Max concurrent was {}, expected >= {}",
            max,
            expected_min
        );
        assert!(
            max <= expected_max,
            "Max concurrent was {}, expected <= {}",
            max,
            expected_max
        );

        // Signal all tasks to complete and wait for them
        for (handle, tx) in items {
            let _ = tx.send(());
            handle.result().await.unwrap();
        }
    }
}
