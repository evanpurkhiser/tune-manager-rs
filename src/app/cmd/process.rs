use std::{io, path::PathBuf};

use crate::{app::config::Config, processing};

pub fn run(path: PathBuf, config: &Config) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(processing::runner::process_path(path, config))
}
