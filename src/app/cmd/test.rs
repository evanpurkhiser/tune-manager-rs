use std::{collections::HashSet, io, sync::LazyLock};

use id3::Tag;
use walkdir::WalkDir;

use crate::{app::config::Config, track::Track};

static IGNORED_FILES: LazyLock<HashSet<&str>> = LazyLock::new(|| HashSet::from_iter([".DS_STORE"]));

pub fn run(config: &Config) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // connect to sqlx database and run migrator
            let pool = sqlx::sqlite::SqlitePool::connect("sqlite:./db.sqlite?mode=rwc")
                .await
                .expect("Failed to connect to sqlite database");

            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .expect("Error running DB migrations");

            let walker = WalkDir::new(&config.catalog_path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| !IGNORED_FILES.contains(&e.file_name().to_str().unwrap()))
                .filter(|e| e.file_type().is_file());

            let mut problematic_files = vec![];

            let items: Vec<(Track, Tag)> = walker
                .map(|e| (e.to_owned(), Tag::read_from_path(e.path())))
                .filter_map(|(entry, tag)| match tag {
                    Ok(t) => Some((entry.path().to_owned(), t)),
                    Err(_) => {
                        problematic_files.push(entry);
                        None
                    }
                })
                .map(|(entry, tag)| (Track::from((entry, tag.clone())), tag))
                .collect();

            for (track, _tag) in items {
                let real_path = track
                    .metadata
                    .file_path
                    .strip_prefix(&config.catalog_path)
                    .unwrap();

                let cononical_path = track.cononical_path();

                if real_path != cononical_path {
                    println!("\nPath mismatch",);
                    println!("current: {}", real_path.display());
                    println!("new:     {}", cononical_path.display());
                }
            }

            println!("There were some problems: {:?}", problematic_files);
        });

    Ok(())
}
