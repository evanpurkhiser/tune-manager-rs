use id3::Tag;

use crate::track::{TaggedFile, Track, TrackFields};

/// The thing a [`Rule`](crate::linter::Rule) is run against. Always carries
/// a [`Track`] — the parsed, structured view used by most rules. May
/// additionally carry the raw [`id3::Tag`] the track was read from, which
/// some rules need to inspect frame-level state (id3 version, presence of
/// non-allowlisted frames, etc.) that doesn't survive the conversion to
/// `TrackFields`.
///
/// Rules that need the raw tag should early-return
/// [`LintResult::Skipped`](crate::linter::LintResult::Skipped) when `id3`
/// is `None`.
pub struct LintTarget {
    pub track: Track,
    pub id3: Option<Tag>,
}

impl From<Track> for LintTarget {
    fn from(track: Track) -> Self {
        Self { track, id3: None }
    }
}

impl From<TaggedFile> for LintTarget {
    fn from(TaggedFile { path, tag }: TaggedFile) -> Self {
        let track = Track {
            file: path.into(),
            fields: TrackFields::from(&tag),
        };
        Self {
            track,
            id3: Some(tag),
        }
    }
}
