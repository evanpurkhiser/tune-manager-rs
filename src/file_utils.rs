use std::{collections::HashSet, path::Path, sync::LazyLock};

use walkdir::{DirEntry, WalkDir};

static IGNORED_FILES: LazyLock<HashSet<&str>> = LazyLock::new(|| HashSet::from_iter([".DS_STORE"]));

/// Creates a walkdir iterator that recursively walks a directory while filtering out ignored files
pub fn walk_music_files<P: AsRef<Path>>(path: P) -> impl Iterator<Item = DirEntry> {
    WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !IGNORED_FILES.contains(&e.file_name().to_str().unwrap_or("")))
        .filter(|e| e.file_type().is_file())
}

