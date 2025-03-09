use ffmpeg_next::{format, media};
use std::{error::Error, io::Write, path::Path};

/// Computes a MD5 hash of the audio stream of the provided input file. The hash will not change as
/// the media files metadata is updated.
pub fn compute(input_path: &Path) -> Result<String, Box<dyn Error>> {
    ffmpeg_next::init()?;

    let mut input_context = format::input(input_path).expect("Input file opened");

    // Find the first audio stream
    let input_stream = input_context
        .streams()
        .find(|s| s.parameters().medium() == media::Type::Audio)
        .ok_or("No audio stream found")?;

    let audio_stream_index = input_stream.index();
    let mut hasher = md5::Context::new();

    // Read packets and hash their data
    for (_, packet) in input_context.packets() {
        if packet.stream() == audio_stream_index {
            hasher.write_all(packet.data().ok_or("Bad packet read")?)?;
        }
    }

    Ok(format!("{:x}", hasher.compute()))
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
