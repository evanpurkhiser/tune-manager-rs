use std::{io, path::Path, process::Command};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeyfinderError {
    #[error("Invalid input path")]
    BadPath,

    #[error("Unable to execute keyfinder-cli command")]
    Command(#[from] io::Error),

    #[error("Key detection failed: {0}")]
    Keyfinder(String),
}

#[derive(Debug, Clone)]
pub enum KeyNotation {
    Standard,
    OpenKey,
    Camelot,
}

impl KeyNotation {
    fn as_arg(&self) -> &'static str {
        match self {
            KeyNotation::Standard => "standard",
            KeyNotation::OpenKey => "openkey",
            KeyNotation::Camelot => "camelot",
        }
    }
}

/// Detects the musical key of the provided audio file using keyfinder-cli.
/// Returns the detected key as a string, or None if no key was detected (silence).
pub fn detect_key(
    input_path: impl AsRef<Path>,
    notation: KeyNotation,
) -> Result<Option<String>, KeyfinderError> {
    let file_path = input_path
        .as_ref()
        .to_str()
        .ok_or(KeyfinderError::BadPath)?;

    let output = Command::new("keyfinder-cli")
        .args(["-n", notation.as_arg(), file_path])
        .output()?;

    if !output.status.success() {
        return Err(KeyfinderError::Keyfinder(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    let key_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key_str.is_empty() {
        Ok(None)
    } else {
        Ok(Some(key_str))
    }
}


#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::tests::fixture_path;

    use super::{KeyNotation, detect_key};

    #[test]
    fn test_detect_key_standard() -> Result<(), Box<dyn Error>> {
        let key = detect_key(fixture_path("example.wav"), KeyNotation::Standard)?;
        assert!(key.is_some());
        assert_eq!(key.unwrap(), "C");
        Ok(())
    }

    #[test]
    fn test_detect_key_openkey() -> Result<(), Box<dyn Error>> {
        let key = detect_key(fixture_path("example.wav"), KeyNotation::OpenKey)?;
        assert!(key.is_some());
        assert_eq!(key.unwrap(), "1d");
        Ok(())
    }

    #[test]
    fn test_detect_key_camelot() -> Result<(), Box<dyn Error>> {
        let key = detect_key(fixture_path("example.wav"), KeyNotation::Camelot)?;
        assert!(key.is_some());
        assert_eq!(key.unwrap(), "8B");
        Ok(())
    }

    #[test]
    fn test_detect_key_aiff() -> Result<(), Box<dyn Error>> {
        let key = detect_key(fixture_path("example.aiff"), KeyNotation::Standard)?;
        assert!(key.is_some());
        assert_eq!(key.unwrap(), "C");
        Ok(())
    }

    #[test]
    fn test_detect_key_bad_file() {
        let result = detect_key(fixture_path("beatport_track.json"), KeyNotation::Standard);
        assert!(matches!(result, Err(super::KeyfinderError::Keyfinder(_))));
    }
}
