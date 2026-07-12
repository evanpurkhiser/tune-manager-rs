use crate::track::Track;

/// Free-text tag fields. Used by rules that emit per-field violations
/// (e.g. special character normalization, name canonicalization) so the rule
/// can iterate fields uniformly and each fix can target one specific
/// field without the rule re-implementing the field plumbing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextField {
    Artist,
    Title,
    Album,
    Remixer,
    Publisher,
}

impl TextField {
    pub const ALL: [TextField; 5] = [
        TextField::Artist,
        TextField::Title,
        TextField::Album,
        TextField::Remixer,
        TextField::Publisher,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Artist => "artist",
            Self::Title => "title",
            Self::Album => "album",
            Self::Remixer => "remixer",
            Self::Publisher => "publisher",
        }
    }

    pub fn get(self, track: &Track) -> Option<&str> {
        match self {
            Self::Artist => track.fields.artist.as_deref(),
            Self::Title => track.fields.title.as_deref(),
            Self::Album => track.fields.album.as_deref(),
            Self::Remixer => track.fields.remixer.as_deref(),
            Self::Publisher => track.fields.publisher.as_deref(),
        }
    }

    pub fn set(self, track: &mut Track, value: String) {
        match self {
            Self::Artist => track.fields.artist = Some(value),
            Self::Title => track.fields.title = Some(value),
            Self::Album => track.fields.album = Some(value),
            Self::Remixer => track.fields.remixer = Some(value),
            Self::Publisher => track.fields.publisher = Some(value),
        }
    }
}
