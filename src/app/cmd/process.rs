use std::{io, path::PathBuf};

use crate::{processing, app::config::Config};

pub fn run(file: PathBuf, config: &Config) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(processing::runner::process_file(file, config))
}

