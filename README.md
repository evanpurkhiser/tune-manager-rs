# tune-manager

[![Tests](https://github.com/evanpurkhiser/tune-manager-rs/actions/workflows/test.yml/badge.svg)](https://github.com/evanpurkhiser/tune-manager-rs/actions/workflows/test.yml)

A personal tool for managing my DJ music collection — importing new music and
keeping the library well-organized.

## Importing new music

New music lands in a configured import directory. From there, importing is a
stage-then-commit flow: a pipeline does the automated work, hand-edits clean
up whatever's left, lint rules verify the results along the way, and only then
are tracks accepted into the catalog.

Crucially, none of the automated or manual edits touch the real ID3 tags until
the very end. Each step appends a `TrackRevision` to a staging area embedded
in the file's tags (a GEOB frame), so every change is recorded and reversible.
The "latest revision" is the working draft; the original on-disk tags are
preserved as the first revision.

### `import prepare`

Walks the import directory and runs each track through the import processing
pipeline. Stages run in order, and each one appends a revision to the staging
area on success:

- **PrepareMedia** — ensures the file is AIFF or MP3, upgrades tags to
  ID3v2.4, and computes the media hash.
- **Keyfinder** — detects the musical key via `keyfinder-cli`.
- **Beatport** — if the track has a Beatport URL in its `WOAF` tag, fetches
  metadata from Beatport.
- **AI** — sends batches of tracks (grouped by folder) to an LLM to clean up
  and normalize metadata. The prompt includes a CSV of the batch's tracks
  alongside any outstanding lint violations, giving the model a directed task
  rather than open-ended cleanup.

Stages are idempotent and resumable: completed stages are recorded in the
file's tags, so re-running `prepare` only does outstanding work.

### `import check`

Reports the status of everything in the import directory, running the lint
rules against each track's latest staged revision. This is the gate for
`accept` — all tracks must pass check before any of them can be imported.

### `import edit`

Hand-edits one or more tracks. Accepts a list of short track IDs and a flag
for each editable tag attribute:

```
import edit <ids...> --publisher="Some Label" --catalog-id="ABC123"
```

Edits are recorded as a new revision in the staging area, the same as a
pipeline stage. There's no special "manual" status — an edit is just another
checkpoint in the track's history.

### `import diff`

Shows a diff between the original tags (the first revision) and the latest
revision. Defaults to all tracks; accepts an optional list of IDs to scope
the output. With `--full-history`, shows a diff at every revision so you can
see what each stage and each manual edit contributed.

### `import rewind`

Restores a track to an earlier checkpoint from its edit history. Useful when
an edit (manual or from a stage) went the wrong direction and the cleanest
fix is to roll back rather than re-edit forward.

### `import reset`

Wipes all tune-manager-managed state from a track: the processing-state and
track-revisions GEOB frames, plus the media hash. The track is left as if it
had never been touched by `prepare`, ready to be processed from scratch.

### `import accept`

Commits the import. For each track in the selection:

1. Writes the latest staged revision into the real ID3 tags.
2. Moves the file into the catalog at its canonical path.

The selection can be a list of short track IDs, a sub-path inside the import
directory (e.g. accept just one album), or — with no arguments — everything
in the import directory:

```
import accept                     # everything in the import dir
import accept "Some Album/"       # everything under a sub-path
import accept <id1> <id2> ...     # specific tracks
```

`accept` is all-or-nothing within the selection: every track in the selection
must pass lint, and they're all imported together. If anything fails lint,
nothing moves.

### Track identifiers

Tracks are referenced by a short prefix of their media hash — a hash of the
audio content itself, so it's stable across tag edits. The length is computed
as the shortest prefix that's unique across every track in the import
directory, then padded out to the longest such prefix so a single length
works uniformly across all commands.
