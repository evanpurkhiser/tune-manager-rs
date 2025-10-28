use std::io;

use tracing::info;

use crate::app::config::Config;

pub fn run(_config: &Config) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { info!("From async land!") });

    Ok(())
}
