use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("Invald input path")]
    BadPath,

    #[error("Unable to execute ffmpeg command")]
    Command(#[from] io::Error),

    #[error("Conversion failed: {0}")]
    FFMpeg(String),
}

/// Converts the given file audio file (usually wav) an AIFF file, copying the audio stream
/// directly without transcoding.
pub fn to_aiff(input_path: &Path) -> Result<PathBuf, ConvertError> {
    let output_path = input_path.with_extension("aiff");

    let input = input_path.to_str().ok_or(ConvertError::BadPath)?;
    let output = output_path.to_str().unwrap();

    let output = Command::new("ffmpeg")
        .args(["-i", input, "-c:a", "copy", output])
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
        let aiff_file = to_aiff(target.as_ref()).unwrap();

        assert!(exists(&aiff_file)?);
        assert_eq!(
            media_hash::compute(target.as_ref()).unwrap(),
            media_hash::compute(&aiff_file).unwrap()
        );
        Ok(())
    }
}
