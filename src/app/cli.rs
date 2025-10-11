use std::path::PathBuf;

use crate::logging;
use clap::{Parser, Subcommand, ValueHint};

pub const VERSION: &str = env!("TUNE_MANAGER_VERSION");
pub const ABOUT: &str = "A tool for managing your DJ music collection.";

#[derive(Parser, Debug)]
#[clap(
    name = "tune-manager",
    version = VERSION,
    about = ABOUT,
    disable_help_subcommand = true,
    subcommand_required = true,
)]
pub struct CliApp {
    #[clap(
        short,
        long,
        global = true,
        help = "The path to the config file.",
        value_hint = ValueHint::FilePath
    )]
    pub config: Option<PathBuf>,

    #[clap(
        short,
        long,
        global = true,
        value_name = "LEVEL",
        help = "Set the log level filter.",
        value_enum
    )]
    pub log_level: Option<logging::Level>,

    #[clap(
        long,
        global = true,
        value_name = "FORMAT",
        help = "Set the logging output format.",
        value_enum
    )]
    pub log_format: Option<logging::LogFormat>,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[clap(
        about = "Start the server",
        after_help = "Runs the tune manager server until it is shutdown"
    )]
    Server,

    #[clap(about = "Debug utilities", subcommand_required = true)]
    Debug {
        #[clap(subcommand)]
        command: DebugCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum DebugCommands {
    #[clap(about = "Process tracks with AI and print results")]
    Ai {
        #[clap(help = "Path to directory containing music files", value_hint = ValueHint::DirPath)]
        path: PathBuf,
    },
    #[clap(about = "Test Beatport authentication and fetch track details from file")]
    Beatport {
        #[clap(long, help = "Beatport username")]
        username: String,
        #[clap(long, help = "Beatport password")]
        password: String,
        #[clap(long, help = "Path to media file to read Beatport URL from", value_hint = ValueHint::FilePath)]
        file: Option<PathBuf>,
    },
}
