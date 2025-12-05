use std::{fs, path::PathBuf};
use tempfile::TempDir;

/// Gets the absolute path of a fixture file.
pub fn fixture_path(path: &str) -> PathBuf {
    let root = env!("CARGO_MANIFEST_DIR");
    format!("{}/fixtures/{}", root, path).into()
}

// Reads the contents of a fixture file as a string
pub fn read_fixture(path: &str) -> String {
    fs::read_to_string(fixture_path(path)).expect("Failed to load test fixtures")
}

/// Creates a temporary directory with a copy of a fixture file. Returns a tuple of (TempDir,
/// PathBuf) where the PathBuf is the path to the copied file. The TempDir will automatically clean
/// up the entire directory (including any other files) when dropped.
pub fn make_temp_fixture(file: &str) -> (TempDir, PathBuf) {
    let source_path = fixture_path(file);

    // Create temp directory
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");

    // Get the filename from the source path
    let filename = source_path
        .file_name()
        .expect("Source path should have a filename");

    // Create destination path in temp directory
    let dest_path = temp_dir.path().join(filename);

    // Copy the fixture file to the temp directory
    fs::copy(&source_path, &dest_path).expect("Failed to copy fixture to temp directory");

    (temp_dir, dest_path)
}
