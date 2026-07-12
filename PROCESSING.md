let's make a process command.

What i'm thinking is that we can import a whole directory of folders using a
new "processing" module. Each track goes through multiple processing stages. I
think we'll want to use tokio with a Unbounded stream for processing each track
stage, since we'll want everything to happen in parallel. We'll want an enum
that defines each of the stages, the enum can probably contain all of the data
we need to execute that stage. so for example the first stage would just be the
file path, it will ensure it's an AIFF or mp3 file, has id3 tags, and has a
media hash written to the tags. stages after that would receive the Tags object
for the already opened file. We'll probably want to use something like foreach
concurrent.

Here'swhat im thinking for stages

- PrepareMedia - ensures aiff / mp3, ensure id3 2.4 tags, compute and write
  media hash. This can happen fully in parallel.

- Keyfinder - determine the key of the track using the keyfinder module. Can be
  run in parallel.

- Beatport - Fetch additional metadata about a track if it's existing ID3 tags
  include the beatport URL as the WOAF tag. Can be done in parallel. We may
  need a way to ensure we don't make too many beatport requests all at once

- AI - Takes a GROUP of tracks and processes them through the OpenAI LLM to
  have it do the cleanup for us. We won't want to send too many tracks to the
  LLM at once, so we'll probably need this processing to be smart, where it
  sneds groups by folder.

  This step should first figure out how to batch the files. For example if it's
  only top-level files and we reach the limit per request (lets say 50) it
  should chunk into 50. If there are folders however we should count the total
  files in a tree of folders, if it's less than 50 that folder is a batch. If
  it's more however, then we would do the same recursive logic of looking for
  folders in that directory.

Any of the steps that are calling external programs, like converting the files,
getting the media hash, the keyfinder program, will all need to have probably
some global concurrency limit. So even if we import 1000 files at once, we're
only running these executables on maybe 10 or 20 at once. We'll 33333333333


We're also going to want to track the state of all files, probably through
message passing to some global struct that tracks all the files and what
they're currently doing, each stage can probably be "not started", "waiting",
"processing", "complete". Waiting would be if we queued the stage to run, but
it hasn't been picked up by the executor yet.

## Architecture Details

### State Management with GEOB Frames

Instead of external state files, we'll use two GEOB (General Encapsulated Object) frames within each file's ID3 tags to store processing state and metadata history:

#### Processing State GEOB (`tune-manager-processing-state`)
```json
{
  "completed_stages": ["PrepareMedia", "Keyfinder"],
  "last_updated": "2025-10-11T05:30:00Z",
  "version": "1.0"
}
```

#### Metadata History GEOB (`tune-manager-metadata-history`)
```json
{
  "history": [
    {
      "stage": "Original",
      "timestamp": "2025-10-11T05:00:00Z",
      "track": { /* serialized Track with original metadata */ }
    },
    {
      "stage": "Keyfinder",
      "timestamp": "2025-10-11T05:15:00Z",
      "track": { /* Track with key field added */ }
    }
  ]
}
```

**Benefits:**
- **Self-contained files** - Each file carries its own processing history
- **No external state files** to manage or lose
- **Atomic updates** - State changes with the file itself
- **Resumable anywhere** - Files can be moved and processing can resume
- **Easy inspection** - Can check processing state by reading file tags

**Idempotent Stages:** Stages are only added to `completed_stages` once fully finished. No "in_progress" tracking means stages can be interrupted and safely re-run. All stages should be idempotent (safe to run multiple times), though they may not be pure functions (e.g., PrepareMedia modifies file containers and tags).

### Coordinator Pattern

The processing coordinator owns all tag writing, eliminating concurrency complexity:

```rust
// Coordinator manages this loop
for file in files {
    let mut tag = Tag::read_from_path(&file)?;
    let completed_stages = read_completed_stages(&tag);
    let current_metadata = read_latest_metadata_history(&tag);

    for stage in determine_next_stages(&completed_stages) {
        // Stage gets read-only data, returns new Track + result details (idempotent)
        let (updated_track, stage_result) = process_stage(stage, &current_metadata)?;

        // Coordinator updates both GEOB frames atomically
        append_metadata_history(&mut tag, &stage, updated_track);
        append_stage_result(&mut tag, &stage, stage_result);
        add_completed_stage(&mut tag, stage.stage_name());
        tag.write_to_path(&file)?;
    }
}
```

**Stage Function Signature:**
```rust
// Stages are idempotent functions - safe to run multiple times
fn process_stage(
    stage: ProcessingStage,
    current_metadata: &Track,
) -> Result<(Track, StageResult), ProcessError>
```

**Advantages:**
- **No RwLocks needed** - coordinator is the single writer
- **Stages are idempotent** - safe to interrupt and restart
- **Atomic updates** - each stage completion is a single tag write
- **Simpler error handling** - no lock poisoning or deadlock concerns
- **Crash recovery** - no "in_progress" state means processing can always resume safely

### Safe Metadata Management

All metadata changes are stored in history GEOB - actual ID3 fields are never modified during processing:

**Processing Flow:**
1. Read current metadata from history GEOB (not actual ID3 fields)
2. Stage processes and returns modified Track object
3. Append new history entry with stage identifier
4. Keep original metadata intact until explicit "commit"

**User Commands:**
- `tune-manager process <dir>` - Run processing pipeline, store in GEOB
- `tune-manager status <files>` - Show processing state and pending changes
- `tune-manager diff <files>` - Show what each stage changed
- `tune-manager commit <files>` - Apply latest history to actual ID3 fields
- `tune-manager rollback <files> <stage>` - Revert to specific history entry

**Benefits:**
- **Never lose original metadata** - complete rollback capability
- **Change tracking** - clear audit trail of what each stage did
- **Safe experimentation** - test new processing logic without risk
- **Batch operations** - preview all changes before committing

### Module Structure

```
src/processing/
├── mod.rs              # Public API and ProcessingStage enum
├── coordinator.rs      # Main processing loop and task scheduling
├── state.rs            # GEOB processing state management
├── history.rs          # GEOB metadata history management
├── stages/
│   ├── mod.rs          # Stage implementations
│   ├── prepare_media.rs # Convert containers, ID3v2.4, media hash
│   ├── keyfinder.rs    # Musical key detection
│   ├── beatport.rs     # Metadata from Beatport API
│   └── ai.rs           # OpenAI batch processing
└── dependencies.rs     # Stage dependency resolution
```

### Dependency Management

Stages can declare dependencies on other stages:

```rust
#[derive(Debug, Clone)]
pub enum ProcessingStage {
    PrepareMedia {
        file_path: PathBuf
    },
    Keyfinder {
        file_path: PathBuf,
        notation: KeyNotation,  // Standard, OpenKey, Camelot
    },
    Beatport {
        file_path: PathBuf,
        credentials: BeatportCredentials,
    },
    AI {
        batch: Vec<PathBuf>,
        client: Client,
        batch_size: usize,
    },
    MetadataCleanup {
        file_path: PathBuf
    },
}

// Structured return types for each stage
#[derive(Debug, Clone)]
pub enum StageResult {
    PrepareMedia(PrepareMediaResult),
    Keyfinder(KeyfinderResult),
    Beatport(BeatportResult),
    AI(AIResult),
    MetadataCleanup(MetadataCleanupResult),
}

#[derive(Debug, Clone)]
pub struct PrepareMediaResult {
    pub converted_from: Option<String>,  // Original format if converted
    pub media_hash: Vec<u8>,
    pub id3_version_updated: bool,
}

#[derive(Debug, Clone)]
pub struct KeyfinderResult {
    pub detected_key: Option<String>,
    pub notation: KeyNotation,
    pub confidence: Option<f32>,  // Future enhancement
}

#[derive(Debug, Clone)]
pub struct BeatportResult {
    pub track_info: Option<BeatportTrackInfo>,
    pub url_found: bool,
    pub api_success: bool,
}

#[derive(Debug, Clone)]
pub struct AIResult {
    pub processed_tracks: Vec<PathBuf>,
    pub tokens_used: u32,
    pub changes_made: Vec<String>,  // Summary of changes
}

#[derive(Debug, Clone)]
pub struct MetadataCleanupResult {
    pub fields_cleaned: Vec<String>,
    pub duplicates_removed: usize,
}

impl ProcessingStage {
    fn dependencies(&self) -> Vec<String> {
        match self {
            PrepareMedia { .. } => vec![],
            Keyfinder { .. } | Beatport { .. } | AI { .. } => vec!["PrepareMedia".to_string()],
            MetadataCleanup { .. } => vec!["Keyfinder".to_string(), "Beatport".to_string()],
        }
    }

    fn stage_name(&self) -> &'static str {
        match self {
            PrepareMedia { .. } => "PrepareMedia",
            Keyfinder { .. } => "Keyfinder",
            Beatport { .. } => "Beatport",
            AI { .. } => "AI",
            MetadataCleanup { .. } => "MetadataCleanup",
        }
    }
}
```

The coordinator checks dependencies before scheduling stages, ensuring proper execution order while maximizing parallelism.

### Concurrency & Resource Management

- **Semaphore-based limits** for external programs (convert, keyfinder, etc.)
- **Tokio streams** for async processing pipeline
- **Per-stage concurrency limits** - configurable via CLI
- **Batch handling** - AI stage collects files and processes in groups
- **Global resource coordination** - prevents overwhelming system resources

### Integration with Existing Code

- **Extend importer.rs** - Move `process_file()` logic into `ProcessingStage::PrepareMedia`
- **Use existing modules** - `convert`, `media_hash`, `keyfinder`, `beatport`, `ai`
- **Leverage Track type** - serialize/deserialize for metadata history
- **Build on error patterns** - compose existing error types

This architecture provides a complete versioned metadata management system with dependency resolution, atomic operations, full audit trails, and safe rollback capabilities.
