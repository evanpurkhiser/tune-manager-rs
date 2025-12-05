pub mod cli;
pub mod cmd;
pub mod config;

use std::io;

use clap::Parser;
use tracing::debug;

use crate::logging::{self, LoggingConfig};

pub fn execute() -> io::Result<()> {
    let app = cli::CliApp::parse();
    let config = config::Config::extract(&app).expect("Configuration invalid");

    logging::init(LoggingConfig::from_config(&config));

    debug!(config = ?config);

    match &app.command {
        cli::Commands::Server => cmd::server::run(&config),
        cli::Commands::Debug { command } => match command {
            cli::DebugCommands::Ai { path } => cmd::debug::ai::run(path.clone()),
            cli::DebugCommands::Beatport {
                username,
                password,
                file,
            } => cmd::debug::beatport::run(username.clone(), password.clone(), file.clone()),
            cli::DebugCommands::Config => cmd::debug::config::run(&app, &config),
        },
        cli::Commands::Process { path } => cmd::process::run(path.clone(), &config),
    }
}
