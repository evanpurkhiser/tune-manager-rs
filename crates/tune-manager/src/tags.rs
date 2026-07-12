use id3::{Tag, TagLike};

/// The set of ID3 tags that are considered to be generally important to my music collection.
#[derive(Debug)]
pub enum Id3TagId {
    Artist,
    Title,
    Album,
    Remixer,
    Publisher,
    CatalogId,
    Bpm,
    Key,
    Year,
    Genre,
    Track,
    Disc,
    MediaHash,
}

impl Id3TagId {
    /// Get the string value of the specified Id3TagId.
    pub fn read(&self, tag: &Tag) -> Option<String> {
        tag.get(self.as_str())
            .and_then(|f| match self {
                // CatalogId is stored as a comment
                Id3TagId::CatalogId => f
                    .content()
                    .comment()
                    .map(|c| c.text.as_str().trim_matches(char::from(0)).to_string()),
                // The mediahash is stored as binary data in the UFID frame
                Id3TagId::MediaHash => f
                    .content()
                    .unique_file_identifier()
                    .map(|ufid| hex::encode(&ufid.identifier)),
                _ => f.content().text().map(str::to_string),
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
            Self::CatalogId => "COMM",
            Self::Bpm => "TBPM",
            Self::Key => "TKEY",
            Self::Year => "TDRC",
            Self::Genre => "TCON",
            Self::Track => "TRCK",
            Self::Disc => "TPOS",
            Self::MediaHash => "UFID",
        }
    }
}
