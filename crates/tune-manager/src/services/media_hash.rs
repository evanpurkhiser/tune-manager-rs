use std::{io, path::Path, process::Command};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MediaHashError {
    #[error("Invalid input path")]
    BadPath,

    #[error("Unable to execute ffmpeg command")]
    Command(#[from] io::Error),

    #[error("Hashing failed: {0}")]
    FFMpeg(String),

    #[error("Unable to decode mediahash from ffmpeg")]
    HashDecode(#[from] hex::FromHexError),
}

/// Computes a MD5 hash of the audio stream of the provided input file. The hash will not change as
/// the media files metadata is updated.
pub fn compute(input_path: impl AsRef<Path>) -> Result<Vec<u8>, MediaHashError> {
    let file_path = input_path
        .as_ref()
        .to_str()
        .ok_or(MediaHashError::BadPath)?;
    let output = Command::new("ffmpeg")
        .args(["-i", file_path, "-c:a", "copy", "-f", "md5", "-"])
        .output()?;

    if output.status.success() {
        let md5_str = String::from_utf8_lossy(&output.stdout)
            .trim()
            .strip_prefix("MD5=")
            .unwrap()
            .to_string();
        Ok(hex::decode(md5_str)?)
    } else {
        Err(MediaHashError::FFMpeg(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{services::media_hash::compute, tests::fixture_path};

    // Both fixtures have equal audio data

    #[test]
    fn test_wav_media_hash() -> Result<(), Box<dyn Error>> {
        let hash = compute(fixture_path("example.wav"))?;
        assert_eq!(hex::encode(hash), "4092e62ffa902b289811c30f3d8d3794");
        Ok(())
    }
    #[test]
    fn test_aiff_media_hash() -> Result<(), Box<dyn Error>> {
        let hash = compute(fixture_path("example.aiff"))?;
        assert_eq!(hex::encode(hash), "4092e62ffa902b289811c30f3d8d3794");
        Ok(())
    }
}
