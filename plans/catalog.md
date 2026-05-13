# Catalog: Collection Management

## Goal

Build collection management tooling for `tune-manager-rs` that:

- syncs the full catalog's file metadata into SQLite;
- keeps catalog rows in lock-step with filesystem changes;
- enforces collection conventions through lint rules and optional fixes.

This doc captures the initial design and the first pass at codifying your collection rules from the current Rust repo plus the legacy `tune-manager` implementation.

## Core Concepts

### 1) Catalog sync

`sync` scans the music catalog and upserts track metadata into a local SQLite database.

- Input: files under `catalog_path` matching `music_file_types`.
- Source of truth: ID3 tags + filesystem metadata (`file_path`, `mtime`).
- Identity:
  - current Rust system already writes a stable media hash into `UFID` (`owner=tune-manager-rs`);
  - that hash should be persisted and used for robust matching even across path changes.
- Output: one canonical row per track with the latest metadata snapshot.

### 2) Lint

`lint` evaluates track metadata/paths against rules and reports violations.

- Modes:
  - report-only (default);
  - fix mode (`--fix`) for deterministic/autosafe transforms.
- Result shape:
  - `rule_id`, severity, file/path, current value, expected value, optional fix.

### 3) Canonical path

Collection layout is derived from tags and acts as both organization and policy surface.

Current canonical logic (already implemented in Rust) is:

- Directory: `{publisher-or-[+no-label]}/[{catalog_id-or--}] {album}` or `[+singles]`.
- Optional disc folder for multi-disc releases: `Disc N`.
- Filename:
  - album track: `{track_number}. [key-or--] {artist} - {title}`;
  - single: optional `[catalog_id]`, then `[key-or--] {artist} - {title}`.
- Path sanitization removes illegal characters and normalizes slash/control chars.

## Rules Inventory (v1)

These are the rules we should document first, then implement incrementally.

### A. Structural and sync integrity

#### `file.supported-extension`
   - Track only: Yes.
   - File extension must be in supported set (`mp3`, `aiff`; conversion pipeline may ingest `wav/flac/m4a`).

#### `file.readable_id3`
   - Track only: No.
   - File must have readable ID3 tags.

#### `file.id3_version`
   - Track only: No.
   - Normalize/write tags as ID3v2.4.

#### `ufid.media_hash_present`
   - Track only: No.
   - `UFID` media hash for `tune-manager-rs` must exist.

#### `catalog.unique_media_hash`
   - Track only: No.
   - No duplicate rows for same media hash.

#### `catalog.media_hash_present`
   - Track only: No.
   - `media_hash` must be present and non-empty on every catalog row.
   - Severity: `error`.
   - Autofix: no (requires tag re-read/backfill during sync).

#### `catalog.path_in_root`
   - Track only: No.
   - Stored path must resolve within catalog root.

### B. Canonical path rules

#### `path.matches-canonical`
   - Track only: Yes.
   - Actual path must match `Track::cononical_path` output.

TODO: Add focused tests for `Track::cononical_path` normalization behavior to
ensure it continues to cover safe character sanitization, whitespace
normalization, and lowercase extension normalization.

### C. Metadata completeness and formatting

#### `meta.required-fields-present`
    - Track only: Yes.
    - Required metadata fields must be present and non-empty (initially: `artist`, `title`).
    - Severity: `error`.
    - Autofix: no.

#### `meta.text-trimmed`
    - Track only: Yes.
    - Text-valued tag fields must have no leading or trailing whitespace.
    - Covers every string-valued field: artist, title, album, remixer,
      publisher, catalog_id, year, genre, key, bpm.
    - Severity: `error`.
    - Autofix: yes. One violation per affected field, fix trims that field.

#### `artist.known_value`
    - Track only: No.
    - Warn when artist is not present in the known catalog artist set.
    - Intended for import/lint workflows to catch new or misspelled artist names against the current collection baseline.
    - Severity: `warn`.
    - Autofix: no (requires manual review or explicit canonicalization mapping).

#### `genre.known_value`
    - Track only: No.
    - Warn when genre is not present in the known catalog genre set.
    - Intended for import/lint workflows to catch new or misspelled genre names against the current collection baseline.
    - Severity: `warn`.
    - Autofix: no (or case-only fix if a canonical match is available).

#### `key.canonical-camelot`
    - Track only: Yes.
    - Key must use canonical Camelot notation: `01A`..`12B`.
    - Non-canonical variants (for example `1A`) are violations.

#### `track.count-format`
    - Track only: Yes.
    - Track should parse as count field (`NN/TT`) when present; invalid raw values are violations.

#### `disc.count-format`
    - Track only: Yes.
    - Disc should parse as count field (`NN/TT`) when present; invalid raw values are violations.

#### `album.requires-disc`
    - Track only: Yes.
    - If album is present, disc must also be present.
    - Severity: `warn`.
    - Autofix: no.

#### `disc.requires-track`
    - Track only: Yes.
    - If disc is present, track must also be present.
    - Severity: `warn`.
    - Autofix: no.

#### `track.requires-disc`
    - Track only: Yes.
    - If track is present, disc must also be present.
    - Severity: `warn`.
    - Autofix: no.

#### `catalog_id.mapping_consistency`
   - Track only: No.
   - Catalog number mapping should be consistent (`COMM`-backed catalog ID in Rust; legacy called this release).

#### `meta.publisher-catalog-pairing`
    - Track only: Yes.
    - Warn when `publisher` is present without `catalog_id`. Catalog numbers
      are often genuinely unknown, so this is a soft signal for review rather
      than a hard failure.
    - Error when `catalog_id` is present without `publisher`. An orphan
      catalog number with no label is a data-integrity hole.
    - Autofix: no. Resolution requires looking up the missing field.

#### `publisher.known_value`
    - Track only: No.
    - Warn when publisher is not present in the known catalog publisher set.
    - Intended for import/lint workflows to catch new or misspelled label/publisher names against the current collection baseline.
    - Severity: `warn`.
    - Autofix: no (requires manual review).

#### `meta.disallowed-characters`
    - Track only: Yes.
    - Reject characters that should not appear in text fields. Currently
      covers smart quotes (normalize to plain ASCII quotes/apostrophes).

#### `title.mix-suffix-style`
    - Track only: Yes.
    - Title mix/edit/version suffixes must follow one canonical style.

#### `title.no-original-mix`
    - Track only: Yes.
    - Title must not include an `(Original Mix)` suffix.
    - Severity: `warn`.
    - Autofix: yes (remove the `(Original Mix)` suffix when present).

#### `title.no-featuring-token`
    - Track only: Yes.
    - Title must not include featuring-artist tokens (`ft`, `feat`, `featuring`, etc.).
    - Featuring artists belong in the artist field.
    - Severity: `warn`.
    - Autofix: no.

#### `meta.remixer-title-consistency`
    - Track only: Yes.
    - Remix signals in title and remixer tag must be consistent with each other.

#### `meta.no-extraneous-id3-tags`
    - Track only: No.
    - ID3 tags outside the managed allowlist must not be present.
    - Only frames owned/managed by tune-manager should exist after normalization.
    - Severity: `warn` (can be elevated to `error` after migration).
    - Autofix: yes (remove non-allowlisted frames).

### D. Artist consistency rules (priority area)

Legacy code already hints at artist tokenization with separators:

- `,`
- `vs`
- `&`
- `Ft.`

Rules to encode:

#### `artist.separator-standardization`
    - Track only: Yes.
    - Token-level: reject non-canonical connector tokens (for example `and`, `vs.`).
    - Pairs with `artist.separator-structure`, which handles arrangement and count-aware separator choice.

#### `artist.feat-standardization`
    - Track only: Yes.
    - Standardize featuring token (for example exactly `Ft.`) and avoid mixed variants (`ft`, `feat`, `Feat.`, etc.).

#### `artist.name-canonicalization`
    - Track only: No.
    - Enforce one spelling/casing per known artist (for example avoid `DJ X` vs `Dj X` vs `dj x`).

#### `artist.remixer-consistency`
    - Track only: No.
    - Keep remixer names aligned with canonical artist names.

#### `artist.separator-structure`
    - Track only: Yes.
    - Validates the arrangement of separators between artist segments.
    - Hygiene: no dangling separators, duplicate separators, or inconsistent spacing.
    - Any mix of canonical separators (`,`, `vs`, `&`) is allowed; choice is left to the writer (e.g. `Technikore vs Dougal & Gammer`, `Aly & Fila, Lostly`).
    - Replaces the earlier `artist.split-token-hygiene`.

### E. Optional quality rules (later)

#### `genre.taxonomy`
    - Track only: No.
    - Restrict/normalize genre to approved vocabulary.

#### `year.format`
    - Track only: Yes.
    - Year should be numeric and consistent width.

#### `bpm.numeric`
    - Track only: Yes.
    - BPM should be parseable numeric value.

#### `artwork.present`
    - Track only: No.
    - Require cover art presence (configurable).

#### `artwork.square`
    - Track only: No.
    - Embedded artwork must be square (width == height).
    - Severity: `warn`.
    - Autofix: no.

#### `artwork.min_dimensions`
    - Track only: No.
    - Embedded artwork must meet minimum dimensions (default `500x500`, configurable).
    - Severity: `warn`.
    - Autofix: no.

#### `artwork.mime_allowed`
    - Track only: No.
    - Embedded artwork MIME type must be in an allowed set (for example `image/jpeg`, `image/png`).

## Command Surface (proposed)

### `catalog sync`

- Full scan + upsert.
- Flags:
  - `--path <dir>` override catalog root;
  - `--delete-missing` remove DB rows for missing files;
  - `--since <timestamp>` optional incremental mode.

### `catalog lint`

- Evaluate rules against DB snapshot and filesystem state.
- Flags:
  - `--rule <id>` include only selected rules (repeatable);
  - `--exclude-rule <id>`;
  - `--fix` apply autofixes;
  - `--format text|json`.

### `catalog path`

- Utility for path policy validation.
- Subcommands:
  - `check` (report mismatches);
  - `rewrite` (move to canonical path).

## Data Model Notes (SQLite)

Initial table should include at least:

- `id` (PK), `file_path`, `mtime`, `media_hash`;
- tag fields: `artist`, `title`, `album`, `remixer`, `publisher`, `catalog_id`, `year`, `genre`, `key`, `bpm`, `disc`, `track`;
- optional derived fields for lint performance:
  - `canonical_path`,
  - `artist_tokens` (or separate token table),
  - `last_linted_at`.

Indexes:

- unique `file_path`;
- unique `media_hash`;
- indexes for high-cardinality lookup fields (`artist`, `publisher`, `catalog_id`, `key`).

## Implementation Phases

1. Ship `catalog sync` with reliable upsert/delete semantics and media hash persistence.
2. Ship `catalog lint` engine with report output and rule IDs.
3. Implement first deterministic rules:
   - `path.matches_canonical`
   - `artist.feat-standardization`
   - `artist.separator-standardization`
   - `artist.separator-structure`
   - `key.canonical-camelot`
4. Add fix mode for safe rewrites.
5. Add configurable rule packs and per-rule allowlists/overrides.

## Open Questions

- Exact canonical collaboration token: should primary joiner be `&`, and should `vs` be preserved only where intentional?
- Featuring token is canonical `Ft.`.
- Should singles with no album always live under `[+singles]`, including tracks that still carry album tags?
- Should path rewrites be immediate in fix mode or emitted as a staged plan first?

## Session Decisions

- `catalog.media_hash_present` is approved and should be enforced as an integrity rule.
- `meta.publisher-catalog-pairing` is approved. The `--` sentinel for unknown
  catalog numbers is dropped — missing catalog with publisher present is a
  warn, not a sentinel-triggering error.
