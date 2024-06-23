use std::{
    path::{Path, PathBuf},
    vec,
};

use id3::Tag;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{fields::CountField, tags::Id3TagId};

// track from metadata and tags using sqlx flatten
#[derive(Debug, sqlx::FromRow)]
pub struct Track {
    #[sqlx(flatten)]
    pub metadata: TrackMetadaata,
    #[sqlx(flatten)]
    pub tags: TrackTags,
}

impl From<(PathBuf, Tag)> for Track {
    fn from((entry, tag): (PathBuf, Tag)) -> Self {
        Self {
            metadata: entry.into(),
            tags: tag.into(),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct TrackMetadaata {
    pub file_path: PathBuf,
    pub mtime: u64,
}

impl From<PathBuf> for TrackMetadaata {
    fn from(path: PathBuf) -> Self {
        Self {
            file_path: path.to_owned(),
            mtime: path
                .metadata()
                .and_then(|m| m.modified())
                .expect("Failed to get file modified time")
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct TrackTags {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub remixer: Option<String>,
    pub publisher: Option<String>,
    pub release: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<String>,
    pub disc: Option<CountField>,
    pub track: Option<CountField>,
}

impl From<Tag> for TrackTags {
    fn from(tag: Tag) -> Self {
        type T = Id3TagId;
        Self {
            artist: T::Artist.from(&tag),
            title: T::Title.from(&tag),
            album: T::Album.from(&tag),
            remixer: T::Remixer.from(&tag),
            publisher: T::Publisher.from(&tag),
            release: T::Release.from(&tag),
            year: T::Year.from(&tag),
            genre: T::Genre.from(&tag),
            key: T::Key.from(&tag),
            bpm: T::Bpm.from(&tag),
            disc: T::Disc.from(&tag).map(|v| v.parse().unwrap()),
            track: T::Track.from(&tag).map(|v| v.parse().unwrap()),
        }
    }
}

lazy_static! {
    /// Patterns to replace (and the string to replace with)
    static ref PATH_REPLACEMENTS: Vec<(Regex, String)> = vec![
        (Regex::new(r#"[\*\?\|:"<>]|^\.|\.$"#).unwrap(), "".to_string()),
        (Regex::new(r#"[\x00-\x1f]"#).unwrap(), "_".to_string()),
        (Regex::new(r#"[\/]"#).unwrap(), "-".to_string()),
        (Regex::new(r#"  +"#).unwrap(), " ".to_string()),
    ];
}

impl Track {
    /// Constructes the cononical path that the track should be located at derived from it's tags.
    pub fn cononical_path(&self) -> PathBuf {
        let tags = &self.tags;
        let mut path_parts = vec![];

        // Construct track directory names
        //
        // {publisher}/[{release}] {album}/Disc {disc}/
        //
        //  - If publisher is None: '[+no-label]'
        //  - If album and release is None: '[+singles]'
        //  - Disc part not required if there is only 1 disc
        //

        // First directory is the publisher
        path_parts.push(
            tags.publisher
                .as_deref()
                .unwrap_or("[+no-label]")
                .to_string(),
        );

        // Second directory is the album name and release number
        path_parts.push(match &tags.album {
            Some(album) => {
                let release = tags.release.as_deref().unwrap_or("--");
                format!("[{}] {}", release, album)
            }
            None => "[+singles]".to_string(),
        });

        //If the album has multiple discs include them as a directory
        if let Some(CountField::Valid(count)) = tags.disc.as_ref() {
            if count.total > 1 {
                path_parts.push(format!("Disc {}", count.number));
            }
        }

        // Construct track filename
        //
        // {track.number}. [{release}] [{key}] {artist} - {title}
        //
        //  - Exclude track number (and trailing dot) if track is a single
        //  - Exclude key (with enclosing brackets) unless available
        //  - Exclude release number (with enclosing brackets) if track is a single
        //
        let mut file_parts = vec![];

        // If part of an album or EP include the track number
        if tags.album.is_some() {
            if let Some(CountField::Valid(count)) = tags.track.as_ref() {
                file_parts.push(format!("{:02}.", count.number));
            }
        }

        // If this track is a single and has a release number include it
        if tags.album.is_none() {
            if let Some(release) = tags.release.as_deref() {
                file_parts.push(format!("[{}]", release))
            }
        }

        // Include key of the track if available
        file_parts.push(format!("[{}]", tags.key.as_deref().unwrap_or("--")));

        // Finally artist and title of the track
        file_parts.push(format!(
            "{} - {}",
            tags.artist.as_deref().unwrap_or("<unknown artist>"),
            tags.title.as_deref().unwrap_or("<unknown title>")
        ));

        let mut filename = file_parts.join(" ").trim().to_string();

        // Fake extension will be replaced later using set_extensoon
        filename.push_str(".xxx");

        path_parts.push(filename);

        let mut path = PathBuf::new();

        for mut part in path_parts {
            for (regex, replacement) in PATH_REPLACEMENTS.iter() {
                part = regex.replace_all(&part, replacement).trim().to_string();
            }
            path.push(Path::new(&part));
        }

        path.set_extension(
            self.metadata
                .file_path
                .extension()
                .unwrap()
                .to_str()
                .unwrap(),
        );

        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl From<TrackTags> for Track {
        fn from(tags: TrackTags) -> Self {
            Self {
                metadata: Default::default(),
                tags,
            }
        }
    }

    impl Default for TrackMetadaata {
        fn default() -> Self {
            Self {
                file_path: PathBuf::from(
                    "Publisher/[RLS] Album/Disc 2/01. [10A] Artist - Title.mp3",
                ),
                mtime: 1234567890,
            }
        }
    }

    impl Default for TrackTags {
        fn default() -> Self {
            Self {
                artist: Some("Artist".to_string()),
                title: Some("Title".to_string()),
                album: Some("Album".to_string()),
                remixer: Some("Remixer".to_string()),
                publisher: Some("Publisher".to_string()),
                release: Some("RLS".to_string()),
                year: Some("2015".to_string()),
                genre: Some("Genre".to_string()),
                key: Some("10A".to_string()),
                bpm: Some("170".to_string()),
                disc: Some("2/4".parse().unwrap()),
                track: Some("1/10".parse().unwrap()),
            }
        }
    }

    #[test]
    fn test_cononical_path() {
        let track: Track = TrackTags::default().into();
        assert_eq!(
            track.cononical_path().to_str().unwrap(),
            "Publisher/[RLS] Album/Disc 2/01. [10A] Artist - Title.mp3"
        );

        let no_publisher: Track = TrackTags {
            publisher: None,
            ..Default::default()
        }
        .into();
        assert_eq!(
            no_publisher.cononical_path().to_str().unwrap(),
            "[+no-label]/[RLS] Album/Disc 2/01. [10A] Artist - Title.mp3"
        );

        let no_release: Track = TrackTags {
            release: None,
            ..Default::default()
        }
        .into();
        assert_eq!(
            no_release.cononical_path().to_str().unwrap(),
            "Publisher/[--] Album/Disc 2/01. [10A] Artist - Title.mp3"
        );

        let single: Track = TrackTags {
            album: None,
            disc: None,
            track: None,
            ..Default::default()
        }
        .into();
        assert_eq!(
            single.cononical_path().to_str().unwrap(),
            "Publisher/[+singles]/[RLS] [10A] Artist - Title.mp3"
        );

        let single_no_release: Track = TrackTags {
            album: None,
            disc: None,
            track: None,
            release: None,
            ..Default::default()
        }
        .into();
        assert_eq!(
            single_no_release.cononical_path().to_str().unwrap(),
            "Publisher/[+singles]/[10A] Artist - Title.mp3"
        );

        let special_characters: Track = TrackTags {
            title: Some(r#"What? P* | Real: <new/track> "cool" "#.to_string()),
            ..Default::default()
        }
        .into();
        assert_eq!(
            special_characters.cononical_path().to_str().unwrap(),
            "Publisher/[RLS] Album/Disc 2/01. [10A] Artist - What P Real new-track cool.mp3"
        );
    }
}
