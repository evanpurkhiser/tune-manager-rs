use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("Invalid input path")]
    BadPath,

    #[error("Cannot convert {0} to aiff file")]
    UnsupportedType(String),

    #[error("Unable to execute ffmpeg command")]
    Command(#[from] io::Error),

    #[error("Conversion failed: {0}")]
    FFMpeg(String),
}

/// Converts the given file audio file (usually wav) an AIFF file, copying the audio stream
/// directly without transcoding.
pub fn to_aiff(input_path: impl AsRef<Path>) -> Result<PathBuf, ConvertError> {
    let file_path = input_path.as_ref();
    let original_ext = file_path
        .extension()
        .and_then(|s| s.to_ascii_lowercase().into_string().ok())
        .ok_or(ConvertError::BadPath)?;

    // Only supports converting wav to AIFF
    if !matches!(original_ext.as_str(), "wav" | "flac" | "m4a") {
        return Err(ConvertError::UnsupportedType(original_ext));
    }

    let output_path = file_path.with_extension("aiff");
    let input = file_path.to_str().ok_or(ConvertError::BadPath)?;
    let output = output_path.to_str().unwrap();

    let output_codec = match original_ext.as_str() {
        "wav" => "copy",
        _ => "pcm_s16be",
    };

    let output = Command::new("ffmpeg")
        .args(["-i", input, "-c:a", output_codec, output])
        .output()?;

    if output.status.success() {
        Ok(output_path)
    } else {
        Err(ConvertError::FFMpeg(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

#[cfg(test)]
mod test {
    use std::{
        env::{join_paths, temp_dir},
        fs::{copy, exists},
        io::Error,
    };

    use crate::{media_hash, tests::fixture_path};

    use super::to_aiff;

    #[test]
    fn test_to_aiff() -> Result<(), Error> {
        let dir = temp_dir();
        let target = join_paths([dir.to_str().unwrap(), "example.wav"]).unwrap();

        copy(fixture_path("example.wav"), &target)?;
        let aiff_file = to_aiff(&target).unwrap();

        assert!(exists(&aiff_file)?);
        assert_eq!(
            media_hash::compute(&target).unwrap(),
            media_hash::compute(&aiff_file).unwrap()
        );
        Ok(())
    }
}
