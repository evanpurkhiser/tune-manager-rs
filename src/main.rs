#![allow(dead_code)]

use std::{collections::HashSet, path::Path};

use id3::Tag;
use walkdir::WalkDir;

use crate::track::TrackTags;

mod fields;
mod tags;
mod track;
mod utils;

static ROOT: &str = "/Users/evan/Music/Tracks/";

const IGNORED_FILES: &[&str] = &[".DS_Store"];

fn main() {
    let root = Path::new(ROOT);

    let ignored: HashSet<&&str> = HashSet::from_iter(IGNORED_FILES.iter());

    println!("{:?}", ignored);

    let walker = WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !ignored.contains(&e.file_name().to_str().unwrap()))
        .filter(|e| e.file_type().is_file());

    let mut problematic_files = vec![];

    let items: Vec<TrackTags> = walker
        .map(|e| (e.to_owned(), Tag::read_from_path(e.path())))
        .filter_map(|(e, t)| t.map_err(|_| problematic_files.push(e)).ok())
        .map(TrackTags::from)
        .collect();

    for track in items {
        println!("Track is {:#?}", track.cononical_path());
    }

    println!("There were some problems: {:?}", problematic_files);
}
