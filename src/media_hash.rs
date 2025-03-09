use std::{path::Path, process::Command};

/// Computes a MD5 hash of the audio stream of the provided input file. The hash will not change as
/// the media files metadata is updated.
pub fn compute(input_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let file_path = input_path.to_str().ok_or("Invalid input path")?;
    let output = Command::new("ffmpeg")
        .args(["-i", file_path, "-c:a", "copy", "-f", "md5", "-"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim()
            .strip_prefix("MD5=")
            .unwrap()
            .to_string())
    } else {
        Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&output.stderr)).into())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{media_hash::compute, tests::fixture_path};

    // Both fixtures have equal audio data

    #[test]
    fn test_wav_media_hash() -> Result<(), Box<dyn Error>> {
        let hash = compute(&fixture_path("example.wav"))?;
        assert_eq!(&hash, "4092e62ffa902b289811c30f3d8d3794");
        Ok(())
    }
    #[test]
    fn test_aiff_media_hash() -> Result<(), Box<dyn Error>> {
        let hash = compute(&fixture_path("example.aiff"))?;
        assert_eq!(&hash, "4092e62ffa902b289811c30f3d8d3794");
        Ok(())
    }
}
