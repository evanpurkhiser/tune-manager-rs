use std::{io, path::Path};

use crate::keyfinder;

#[derive(Debug)]
pub struct KeyfinderResult {
    pub detected_key: Option<String>,
}

pub fn run(file_path: &Path) -> io::Result<KeyfinderResult> {
    let detected_key = keyfinder::detect_key(file_path, keyfinder::KeyNotation::Camelot)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Keyfinder failed: {}", e)))?;

    Ok(KeyfinderResult { detected_key })
}
