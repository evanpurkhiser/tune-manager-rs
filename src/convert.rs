use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Converts the given file audio file (usually wav) an AIFF file, copying the audio stream
/// directly without transcoding.
pub fn to_aiff(input_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = input_path.with_extension("aiff");

    let input = input_path.to_str().ok_or("Invalid input path")?;
    let output = output_path.to_str().unwrap();

    let output = Command::new("ffmpeg")
        .args(["-i", input, "-c:a", "copy", output])
        .output()?;

    if output.status.success() {
        Ok(output_path)
    } else {
        Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into())
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
