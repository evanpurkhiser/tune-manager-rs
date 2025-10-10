pub mod cli;
pub mod cmd;
pub mod config;

use std::io;

use clap::Parser;
use tracing::info;

use crate::logging::{self, LoggingConfig};

pub fn execute() -> io::Result<()> {
    let app = cli::CliApp::parse();
    let config = config::Config::extract(&app).expect("Configuration invalid");

    logging::init(LoggingConfig::from_config(&config));

    info!(config = ?config);

    match app.command {
        cli::Commands::Server => cmd::server::run(&config),
        cli::Commands::Test => cmd::test::run(&config),
        cli::Commands::Debug { command } => match command {
            cli::DebugCommands::Ai { path } => cmd::debug::ai::run(path),
        },
    }
}
