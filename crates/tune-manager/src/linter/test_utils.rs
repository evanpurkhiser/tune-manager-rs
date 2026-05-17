use std::path::PathBuf;

use crate::{
    fields::CountField,
    track::{Track, TrackFields, TrackFile},
};

pub fn make_track() -> Track {
    Track {
        file: TrackFile {
            file_path: PathBuf::from("Publisher/[RLS] Album/Disc 2/01. [10A] Artist - Title.mp3"),
            mtime: 1,
        },
        fields: TrackFields {
            media_hash: Some("abc123".to_string()),
            artist: Some("Artist".to_string()),
            title: Some("Title".to_string()),
            album: Some("Album".to_string()),
            remixer: Some("Remixer".to_string()),
            publisher: Some("Publisher".to_string()),
            catalog_id: Some("RLS".to_string()),
            year: Some("2015".to_string()),
            genre: Some("Genre".to_string()),
            key: Some("10A".to_string()),
            bpm: Some("170".to_string()),
            disc: Some("2/4".parse::<CountField>().unwrap()),
            track: Some("1/10".parse::<CountField>().unwrap()),
        },
    }
}
