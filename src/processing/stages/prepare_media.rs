use std::path::{Path, PathBuf};

use id3::{self, Tag, TagLike, Version, frame};
use thiserror::Error;

use crate::{convert, media_hash};

const MEDIA_HASH_OWNER: &str = "tune-manager-rs";

enum ContainerResult {
    Valid,
    Converted(PathBuf),
}

#[derive(Error, Debug)]
pub enum ContainerError {
    #[error("Invalid input path")]
    BadPath,

    #[error("Cannot import file of type {0}")]
    UnsupportedType(String),

    #[error(transparent)]
    ConvertError(#[from] convert::ConvertError),
}

/// Ensures the provided file is either a mp3 or aiff file. wav files will be converted to aiff.
fn ensure_media_container(path: impl AsRef<Path>) -> Result<ContainerResult, ContainerError> {
    let ext = path
        .as_ref()
        .extension()
        .and_then(|s| s.to_ascii_lowercase().into_string().ok())
        .ok_or(ContainerError::BadPath)?;

    match ext.as_str() {
        "mp3" | "aiff" => Ok(ContainerResult::Valid),
        "wav" | "flac" | "m4a" => {
            let new_path = convert::to_aiff(&path)?;
            Ok(ContainerResult::Converted(new_path))
        }
        _ => Err(ContainerError::UnsupportedType(ext)),
    }
}

/// Ensures that the file is using a ID3v2.4 tag.
fn ensure_id3_version(path: impl AsRef<Path>) -> Result<(), id3::Error> {
    Tag::read_from_path(&path).and_then(|tag| tag.write_to_path(&path, Version::Id3v24))
}

#[derive(Error, Debug)]
pub enum MediaHashError {
    #[error(transparent)]
    HashingFailed(#[from] media_hash::MediaHashError),

    #[error("Failed to write Media Hash to media file")]
    WriteError(#[from] id3::Error),
}

/// Ensures that the media hash (UFID) is present in the metadata.
fn ensure_media_hash(path: impl AsRef<Path>, tag: &mut Tag) -> Result<Vec<u8>, MediaHashError> {
    let (media_hash, other_ufid): (Vec<_>, Vec<_>) = tag
        .unique_file_identifiers()
        .partition(|ufid| ufid.owner_identifier == MEDIA_HASH_OWNER);

    if media_hash.len() == 1 && other_ufid.is_empty() {
        return Ok(media_hash.first().map(|u| u.identifier.clone()).unwrap());
    }

    let identifier = media_hash::compute(&path)?;

    let content = frame::UniqueFileIdentifier {
        owner_identifier: MEDIA_HASH_OWNER.to_string(),
        identifier: identifier.clone(),
    };

    tag.remove_all_unique_file_identifiers();
    tag.add_frame(content);
    tag.write_to_path(path, Version::Id3v24)?;

    Ok(identifier)
}

#[derive(Error, Debug)]
pub enum PrepareMediaError {
    #[error(transparent)]
    Container(#[from] ContainerError),

    #[error(transparent)]
    MediaHash(#[from] MediaHashError),

    #[error(transparent)]
    Tag(#[from] id3::Error),
}

#[derive(Debug, Default)]
pub struct PrepareMediaResult {
    pub file_path: PathBuf,
    pub media_hash: Vec<u8>,
    pub tag: Tag,
}

pub fn run(path: impl AsRef<Path>) -> Result<PrepareMediaResult, PrepareMediaError> {
    let mut file_path: PathBuf = path.as_ref().into();

    match ensure_media_container(&file_path)? {
        ContainerResult::Valid => {}
        ContainerResult::Converted(path) => file_path = path,
    };

    ensure_id3_version(&file_path)?;

    let mut tag = Tag::read_from_path(&file_path)?;
    let media_hash = ensure_media_hash(&file_path, &mut tag)?;

    Ok(PrepareMediaResult {
        file_path,
        media_hash,
        tag,
    })
}
