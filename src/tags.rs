use id3::{Tag, TagLike};

/// The set of ID3 tags that are considered to be generally important to my music collection.
#[derive(Debug)]
pub enum Id3TagId {
    Artist,
    Title,
    Album,
    Remixer,
    Publisher,
    CatlogId,
    Bpm,
    Key,
    Year,
    Genre,
    Track,
    Disc,
}

impl Id3TagId {
    /// Get the string value of the specified Id3TagId.
    pub fn from(&self, tag: &Tag) -> Option<String> {
        tag.get(self.as_str())
            .and_then(|f| match self {
                Id3TagId::CatlogId => f
                    .content()
                    .comment()
                    .map(|c| c.text.as_str().trim_matches(char::from(0))),
                _ => f.content().text(),
            })
            .map(|v| v.to_string())
            .filter(|v| !v.is_empty())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Artist => "TPE1",
            Self::Title => "TIT2",
            Self::Album => "TALB",
            Self::Remixer => "TPE4",
            Self::Publisher => "TPUB",
            Self::CatlogId => "COMM",
            Self::Bpm => "TBPM",
            Self::Key => "TKEY",
            Self::Year => "TDRC",
            Self::Genre => "TCON",
            Self::Track => "TRCK",
            Self::Disc => "TPOS",
        }
    }
}
