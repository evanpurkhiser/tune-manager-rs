use std::{mem::replace, sync::Arc};

use tracing::error;

use crate::processing::{
    concurrent::ItemStatus,
    stages::{ProducesRevision, StageStatus},
    state,
};

use super::{
    batch::{BatchState, Batches, ProcessingBatch, StatusEvent},
    callbacks::CallbackRegistry,
    stage_runner::TrackStageStatus,
};

/// Handle a status update from a processor
pub fn handle_track_status<F>(
    batches: &mut Batches,
    dispatch_next_stages: F,
    callback_registry: &Arc<CallbackRegistry>,
    track_status: TrackStageStatus,
) where
    F: FnOnce(&mut ProcessingBatch),
{
    let TrackStageStatus {
        batch_id,
        file_path,
        status,
    } = track_status;

    let Some(batch) = batches.get_mut(&batch_id) else {
        error!("Received status for unknown batch: {}", batch_id);
        return;
    };

    let track_key = file_path.clone();
    let Some(mut track_state) = batch.tracks.remove(&track_key) else {
        error!(
            "Received status for unknown track: {} in batch {}",
            file_path.display(),
            batch_id
        );
        return;
    };

    if let StageStatus::PrepareMedia(ItemStatus::Complete(Ok(result))) = status.as_ref() {
        track_state.file_path = result.file_path.clone();
        track_state.tag = Some(result.tag.clone());
    }

    // Get previous revision for stages that need it
    let last_revision = track_state.tag.as_ref().and_then(state::get_last_revision);

    // Extract revision from the completed stage
    let revision = status.produce_revision(last_revision.as_ref());

    // Only dispatch next stages if this stage completed successfully or was skipped
    let try_next_stages = status.is_success() || status.is_skipped();

    // Handle successful completion - update track tags with new revision
    if let Some(ref revision) = revision {
        track_state.tag.as_mut().map(|tag| {
            state::append_track_revision(tag, revision.clone()).expect("revision added");
            tag.write_to_path(file_path.clone(), id3::Version::Id3v24)
        });
    }

    track_state.set_stage_status(status.clone());

    batch.tracks.insert(track_key, track_state);

    // Emit track stage update event
    let track_update_event = StatusEvent::TrackStageUpdate {
        batch,
        file_path: file_path.clone(),
        status,
        revision: Box::new(revision),
    };
    callback_registry.invoke_all(&track_update_event);

    if try_next_stages {
        dispatch_next_stages(batch);
    }

    // Check if batch is complete and notify if so
    if batch.is_complete() {
        if let BatchState::Processing(completion_tx) =
            replace(&mut batch.state, BatchState::Complete)
        {
            let _ = completion_tx.send(());
        }

        // Emit batch completed event
        let batch_completed_event = StatusEvent::BatchCompleted { batch };
        callback_registry.invoke_all(&batch_completed_event);
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use id3::{Tag, TagLike};
    use tokio::sync::oneshot;

    use super::*;
    use crate::processing::{
        coordinator::batch::BatchConfig,
        stages::{ProcessingStage, keyfinder, prepare_media, test_helpers::*},
        state::TrackRevision,
    };

    /// Helper to create a batch with a single track
    fn make_batch_with_track(file_path: PathBuf) -> ProcessingBatch {
        let (tx, _rx) = oneshot::channel();
        let config = BatchConfig::dry();
        ProcessingBatch::new(vec![file_path], config, tx)
    }

    /// Helper to mark a stage as complete
    fn mark_stage_complete(
        track: &mut super::super::batch::TrackProcessingState,
        stage: ProcessingStage,
    ) {
        track.set_stage_status(make_status_completed(stage.clone()));
        track.stage_dispatched.insert(stage);
    }

    /// Helper to create a mock dispatcher function with a oneshot channel
    fn make_mock_dispatcher() -> (impl FnOnce(&mut ProcessingBatch), oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        let dispatch_fn = move |_batch: &mut ProcessingBatch| {
            let _ = tx.send(());
        };
        (dispatch_fn, rx)
    }

    /// Test context that holds common test setup
    struct TestContext {
        batches: Batches,
        batch_id: super::super::batch::BatchId,
        file_path: PathBuf,
        callback_registry: Arc<CallbackRegistry>,
    }

    impl TestContext {
        /// Create a new test context with standard setup
        fn new() -> Self {
            let file_path = PathBuf::from("/test/track.mp3");
            let mut batches = Batches::new();
            let batch = make_batch_with_track(file_path.clone());
            let batch_id = batches.add(batch);
            let callback_registry = Arc::new(CallbackRegistry::new());

            Self {
                batches,
                batch_id,
                file_path,
                callback_registry,
            }
        }

        /// Create a test context with a track that has a tag
        fn with_tag(tag: Tag) -> Self {
            let mut ctx = Self::new();
            let batch = ctx.batches.get_mut(&ctx.batch_id).unwrap();
            let track = batch.tracks.get_mut(&ctx.file_path).unwrap();
            track.tag = Some(tag);
            ctx
        }

        /// Handle a status update using empty dispatcher
        fn handle_status(&mut self, status: Arc<StageStatus>) {
            self.handle_status_with(status, |_| {});
        }

        /// Handle a status update with a custom dispatcher
        fn handle_status_with<F>(&mut self, status: Arc<StageStatus>, dispatch_fn: F)
        where
            F: FnOnce(&mut ProcessingBatch),
        {
            handle_track_status(
                &mut self.batches,
                dispatch_fn,
                &self.callback_registry,
                TrackStageStatus {
                    batch_id: self.batch_id.clone(),
                    file_path: self.file_path.clone(),
                    status,
                },
            );
        }

        /// Get a reference to the track state
        fn get_track(&self) -> &super::super::batch::TrackProcessingState {
            self.batches
                .get(&self.batch_id)
                .unwrap()
                .tracks
                .get(&self.file_path)
                .unwrap()
        }

        /// Get a reference to the track's tag
        fn get_track_tag(&self) -> &Tag {
            self.get_track().tag.as_ref().unwrap()
        }

        /// Get a reference to the batch
        fn get_batch(&self) -> &ProcessingBatch {
            self.batches.get(&self.batch_id).unwrap()
        }
    }

    #[test]
    fn test_successful_stage_completion() {
        let mut ctx = TestContext::new();
        let (mock_dispatch, mut dispatch_rx) = make_mock_dispatcher();

        ctx.handle_status_with(make_status_completed(ProcessingStage::PrepareMedia), mock_dispatch);

        // Verify dispatch was called for successful completion
        assert!(dispatch_rx.try_recv().is_ok());

        // Verify track state was updated
        let track = ctx.get_track();
        assert!(
            track
                .stage_status
                .iter()
                .any(|s| s.stage() == ProcessingStage::PrepareMedia)
        );
    }

    #[test]
    fn test_prepare_media_complete() {
        let mut ctx = TestContext::new();
        let converted_path = PathBuf::from("/test/track_converted.mp3");

        let mut tag = Tag::new();
        tag.set_title("Test Track");

        let result = prepare_media::PrepareMediaResult {
            file_path: converted_path.clone(),
            tag: tag.clone(),
            media_hash: vec![1, 2, 3],
        };

        ctx.handle_status(Arc::new(StageStatus::PrepareMedia(ItemStatus::Complete(Ok(
            result,
        )))));

        // Verify file_path was updated to converted path
        let track = ctx.get_track();
        assert_eq!(track.file_path, converted_path);

        // Verify tag was set
        let track_tag = ctx.get_track_tag();
        assert_eq!(track_tag.title(), Some("Test Track"));

        // Verify revision was added to tag
        let revision = state::get_last_revision(track_tag).unwrap();
        assert_eq!(revision.tags.title, Some("Test Track".to_string()));
    }

    #[test]
    fn test_skipped_stage_still_dispatches_next() {
        let mut ctx = TestContext::new();
        let (mock_dispatch, mut dispatch_rx) = make_mock_dispatcher();

        let status = Arc::new(StageStatus::Keyfinder(ItemStatus::Skipped(
            "Already has key".to_string(),
        )));

        ctx.handle_status_with(status, mock_dispatch);

        // Verify dispatch was called even though stage was skipped
        assert!(dispatch_rx.try_recv().is_ok());

        // Verify the stage was marked as skipped in track state
        let track = ctx.get_track();
        let keyfinder_status = track
            .stage_status
            .iter()
            .find(|s| s.stage() == ProcessingStage::Keyfinder);
        assert!(keyfinder_status.is_some());
        assert!(keyfinder_status.unwrap().is_skipped());
    }

    #[test]
    fn test_failed_stage_does_not_dispatch_next() {
        use prepare_media::{ContainerError, PrepareMediaError};

        let mut ctx = TestContext::new();
        let (mock_dispatch, mut dispatch_rx) = make_mock_dispatcher();

        let error = PrepareMediaError::Container(ContainerError::BadPath);
        let status = Arc::new(StageStatus::PrepareMedia(ItemStatus::Complete(Err(error))));

        ctx.handle_status_with(status, mock_dispatch);

        // Verify dispatch was NOT called for failed stage
        assert!(dispatch_rx.try_recv().is_err());

        // Verify track state was still updated with failure
        let track = ctx.get_track();
        assert!(
            track
                .stage_status
                .iter()
                .any(|s| s.stage() == ProcessingStage::PrepareMedia)
        );
    }

    #[test]
    fn test_batch_completion_detection() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let file_path = PathBuf::from("/test/track.mp3");

        // Setup with custom completion channel
        let mut batches = Batches::new();
        let (completion_tx, mut completion_rx) = oneshot::channel();
        let config = BatchConfig::dry();
        let mut batch = ProcessingBatch::new(vec![file_path.clone()], config, completion_tx);

        // Mark all stages as complete except AI
        let track = batch.tracks.get_mut(&file_path).unwrap();
        mark_stage_complete(track, ProcessingStage::PrepareMedia);
        mark_stage_complete(track, ProcessingStage::Keyfinder);
        mark_stage_complete(track, ProcessingStage::Beatport);

        // Add a tag with revision so AI can complete
        let mut tag = Tag::new();
        let revision = TrackRevision::new(Default::default());
        state::append_track_revision(&mut tag, revision).unwrap();
        track.tag = Some(tag);

        let batch_id = batches.add(batch);
        let callback_registry = Arc::new(CallbackRegistry::new());

        let batch_completed_called = Arc::new(AtomicBool::new(false));
        let batch_completed_clone = batch_completed_called.clone();

        // Register callback to detect batch completion (keep handle alive)
        let _handle = callback_registry.register(move |event: &StatusEvent| {
            if matches!(event, StatusEvent::BatchCompleted { .. }) {
                batch_completed_clone.store(true, Ordering::SeqCst);
            }
        });

        // Complete the final AI stage
        handle_track_status(
            &mut batches,
            |_| {},
            &callback_registry,
            TrackStageStatus {
                batch_id: batch_id.clone(),
                file_path: file_path.clone(),
                status: make_status_completed(ProcessingStage::Ai),
            },
        );

        // Verify batch is marked as complete
        let batch = batches.get(&batch_id).unwrap();
        assert!(matches!(batch.state, BatchState::Complete));

        // Verify completion channel was notified
        assert!(completion_rx.try_recv().is_ok());

        // Verify batch completed callback was invoked
        assert!(batch_completed_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_unknown_batch_id_logs_error() {
        let mut ctx = TestContext::new();
        let (mock_dispatch, mut dispatch_rx) = make_mock_dispatcher();

        // Create a random batch ID that doesn't exist
        let (tx, _rx) = oneshot::channel();
        let config = BatchConfig::dry();
        let unknown_batch = ProcessingBatch::new(vec![PathBuf::from("/fake/file.mp3")], config, tx);
        let unknown_batch_id = unknown_batch.id.clone();

        // Try to handle status for unknown batch
        handle_track_status(
            &mut ctx.batches,
            mock_dispatch,
            &ctx.callback_registry,
            TrackStageStatus {
                batch_id: unknown_batch_id,
                file_path: ctx.file_path.clone(),
                status: make_status_completed(ProcessingStage::PrepareMedia),
            },
        );

        // Verify dispatch was NOT called
        assert!(dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_unknown_track_logs_error() {
        let mut ctx = TestContext::new();
        let (mock_dispatch, mut dispatch_rx) = make_mock_dispatcher();
        let unknown_file = PathBuf::from("/test/unknown.mp3");

        // Try to handle status for unknown track
        handle_track_status(
            &mut ctx.batches,
            mock_dispatch,
            &ctx.callback_registry,
            TrackStageStatus {
                batch_id: ctx.batch_id.clone(),
                file_path: unknown_file.clone(),
                status: make_status_completed(ProcessingStage::PrepareMedia),
            },
        );

        // Original track should still be there
        let batch = ctx.get_batch();
        assert!(batch.tracks.contains_key(&ctx.file_path));
        assert!(!batch.tracks.contains_key(&unknown_file));

        // Verify dispatch was NOT called
        assert!(dispatch_rx.try_recv().is_err());
    }

    #[test]
    fn test_revision_chaining() {
        let mut ctx = TestContext::new();

        // First, complete PrepareMedia to create initial revision
        let mut tag = Tag::new();
        tag.set_title("Test Track");
        tag.set_artist("Test Artist");

        let prepare_result = prepare_media::PrepareMediaResult {
            file_path: ctx.file_path.clone(),
            tag: tag.clone(),
            media_hash: vec![1, 2, 3],
        };

        ctx.handle_status(Arc::new(StageStatus::PrepareMedia(ItemStatus::Complete(Ok(
            prepare_result,
        )))));

        // Verify initial revision was created
        let track_tag = ctx.get_track_tag();
        let initial_revision = state::get_last_revision(track_tag).unwrap();
        assert_eq!(initial_revision.tags.title, Some("Test Track".to_string()));
        assert_eq!(
            initial_revision.tags.artist,
            Some("Test Artist".to_string())
        );
        assert!(initial_revision.tags.key.is_none());

        // Now complete Keyfinder to chain a new revision
        let keyfinder_result = keyfinder::KeyfinderResult {
            detected_key: Some("Am".to_string()),
        };

        ctx.handle_status(Arc::new(StageStatus::Keyfinder(ItemStatus::Complete(Ok(
            keyfinder_result,
        )))));

        // Verify new revision was chained with the key
        let track_tag = ctx.get_track_tag();
        let latest_revision = state::get_last_revision(track_tag).unwrap();

        // Should have all previous fields plus the new key
        assert_eq!(latest_revision.tags.title, Some("Test Track".to_string()));
        assert_eq!(
            latest_revision.tags.artist,
            Some("Test Artist".to_string())
        );
        assert_eq!(latest_revision.tags.key, Some("Am".to_string()));
    }

    #[test]
    fn test_callback_system_all_status_transitions() {
        use std::sync::Mutex;

        // Setup context with a tag that has a revision for keyfinder
        let mut tag = Tag::new();
        tag.set_title("Test Track");
        let prev_revision = TrackRevision::new(Default::default());
        state::append_track_revision(&mut tag, prev_revision).unwrap();

        let mut ctx = TestContext::with_tag(tag);

        // Track all events in order
        let captured_events = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured_events.clone();

        // Register callback to capture all events
        let _handle = ctx.callback_registry.register(move |event: &StatusEvent| {
            if let StatusEvent::TrackStageUpdate { status, .. } = event
                && status.stage() == ProcessingStage::Keyfinder
            {
                let mut events = captured_clone.lock().unwrap();
                events.push(status.item_status());
            }
        });

        // Test Waiting status
        ctx.handle_status(Arc::new(StageStatus::Keyfinder(ItemStatus::Waiting)));

        // Test Running status
        ctx.handle_status(Arc::new(StageStatus::Keyfinder(ItemStatus::Running)));

        // Test Skipped status
        ctx.handle_status(Arc::new(StageStatus::Keyfinder(ItemStatus::Skipped(
            "Already has key".to_string(),
        ))));

        // Test Complete (Success) status
        let success_result = keyfinder::KeyfinderResult {
            detected_key: Some("Am".to_string()),
        };
        ctx.handle_status(Arc::new(StageStatus::Keyfinder(ItemStatus::Complete(Ok(
            success_result,
        )))));

        // Test Complete (Failed) status
        let error = crate::keyfinder::KeyfinderError::BadPath;
        ctx.handle_status(Arc::new(StageStatus::Keyfinder(ItemStatus::Complete(Err(
            error,
        )))));

        // Verify all status transitions were captured in order via callbacks
        let events = captured_events.lock().unwrap();
        assert_eq!(events.len(), 5);
        assert!(matches!(events[0], ItemStatus::Waiting));
        assert!(matches!(events[1], ItemStatus::Running));
        assert!(matches!(events[2], ItemStatus::Skipped(_)));
        assert!(matches!(events[3], ItemStatus::Complete(Ok(_))));
        assert!(matches!(events[4], ItemStatus::Complete(Err(_))));
    }
}
