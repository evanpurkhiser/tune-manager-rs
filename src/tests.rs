use std::fs;

pub fn read_fixture(path: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let fixture_path = format!("{}/fixtures/{}", root, path);

    fs::read_to_string(fixture_path).expect("Failed to load test fixtures")
}
