use std::{
    path::{Path, PathBuf},
    vec,
};

use id3::Tag;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{fields::CountField, tags::Id3TagId};

#[derive(Debug)]
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
        (Regex::new(r#"[\*\?\|:"<>]"#).unwrap(), "".to_string()),
        (Regex::new(r#"[\x00-\x1f]"#).unwrap(), "_".to_string()),
        (Regex::new(r#"[\/]"#).unwrap(), "-".to_string()),
        (Regex::new(r#"  +"#).unwrap(), " ".to_string()),
    ];
}

impl TrackTags {
    /// Constructes the cononical path that the track should be located at derived from it's tags.
    pub fn cononical_path(&self) -> PathBuf {
        let mut parts = vec![];

        // Construct track directory names
        //
        // {publisher}/[{release}] {album}/Disc {disc}/
        //
        //  - If publisher is None: '[+no-label]'
        //  - If album and release is None: '[+singles]'
        //  - Disc part not required if there is only 1 disc
        //

        // First directory is the publisher
        parts.push(
            self.publisher
                .as_deref()
                .unwrap_or("[+no-label]")
                .to_string(),
        );

        // Second directory is the album name and release number
        parts.push(match &self.album {
            Some(album) => {
                let release = self.release.as_deref().unwrap_or("--");
                format!("[{}] {}", release, album)
            }
            None => "[+singles]".to_string(),
        });

        //If the album has multiple discs include them as a directory
        if let Some(CountField::Valid(count)) = self.disc.as_ref() {
            if count.total > 1 {
                parts.push(format!("Disc {}", count.number));
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
        let mut filename = vec![];

        // If part of an album or EP include the track number
        if self.album.is_some() {
            if let Some(CountField::Valid(count)) = self.track.as_ref() {
                filename.push(format!("{:02}.", count.number));
            }
        }

        // If this track is a single and has a release number include it
        if self.album.is_none() {
            if let Some(release) = self.release.as_deref() {
                filename.push(format!("[{}]", release))
            }
        }

        // Include key of the track if available
        filename.push(format!("[{}]", self.key.as_deref().unwrap_or("--")));

        // Finally artist and title of the track
        filename.push(format!(
            "{} - {}",
            self.artist.as_deref().unwrap_or("<unknown artist>"),
            self.title.as_deref().unwrap_or("<unknown title>")
        ));

        parts.push(filename.join(" "));

        parts
            .into_iter()
            .map(|mut part| {
                for (regex, replacement) in PATH_REPLACEMENTS.iter() {
                    part = regex.replace_all(&part, replacement).trim().to_string();
                }
                part
            })
            .fold(PathBuf::new(), |mut acc, part| {
                acc.push(Path::new(&part));
                acc
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let simple_track = TrackTags::default();
        assert_eq!(
            simple_track.cononical_path().to_str().unwrap(),
            "Publisher/[RLS] Album/Disc 2/01. [10A] Artist - Title"
        );

        let no_publisher = TrackTags {
            publisher: None,
            ..Default::default()
        };
        assert_eq!(
            no_publisher.cononical_path().to_str().unwrap(),
            "[+no-label]/[RLS] Album/Disc 2/01. [10A] Artist - Title"
        );

        let no_release = TrackTags {
            release: None,
            ..Default::default()
        };
        assert_eq!(
            no_release.cononical_path().to_str().unwrap(),
            "Publisher/[--] Album/Disc 2/01. [10A] Artist - Title"
        );

        let single = TrackTags {
            album: None,
            disc: None,
            track: None,
            ..Default::default()
        };
        assert_eq!(
            single.cononical_path().to_str().unwrap(),
            "Publisher/[+singles]/[RLS] [10A] Artist - Title"
        );

        let single_no_release = TrackTags {
            album: None,
            disc: None,
            track: None,
            release: None,
            ..Default::default()
        };
        assert_eq!(
            single_no_release.cononical_path().to_str().unwrap(),
            "Publisher/[+singles]/[10A] Artist - Title"
        );

        let special_characters = TrackTags {
            title: Some(r#"What? P* | Real: <new/track> "cool" "#.to_string()),
            ..Default::default()
        };
        assert_eq!(
            special_characters.cononical_path().to_str().unwrap(),
            "Publisher/[RLS] Album/Disc 2/01. [10A] Artist - What P Real new-track cool"
        );
    }
}
