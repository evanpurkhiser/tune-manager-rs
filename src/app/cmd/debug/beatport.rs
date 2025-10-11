use std::io;

use tracing::{error, info};

use crate::beatport::{BeatportCredentials, BeatportSource};

pub fn run(username: String, password: String) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_async(username, password))
}

async fn run_async(username: String, password: String) -> io::Result<()> {
    info!("Authenticating with Beatport using username: {}", username);

    let credentials = BeatportCredentials { username, password };

    match BeatportSource::new().authenticate(credentials).await {
        Ok(authenticated_source) => {
            info!("Beatport authentication successful!");
            info!("Token: {}", authenticated_source.token());
        }
        Err(e) => {
            error!("Beatport authentication failed: {}", e);
        }
    }

    Ok(())
}
