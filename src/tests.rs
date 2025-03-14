use std::{fs, path::PathBuf};

/// Get's the absolute path of a fixture file.
pub fn fixture_path(path: &str) -> PathBuf {
    let root = env!("CARGO_MANIFEST_DIR");
    format!("{}/fixtures/{}", root, path).into()
}

// Reads the contents of a fixture file as a string
pub fn read_fixture(path: &str) -> String {
    fs::read_to_string(fixture_path(path)).expect("Failed to load test fixtures")
}
