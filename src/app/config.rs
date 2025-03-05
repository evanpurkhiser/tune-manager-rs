use std::{borrow::Cow, path::PathBuf, str::FromStr};

use figment::{
    Figment,
    providers::{Format, Serialized, Yaml},
};

use serde::{Deserialize, Serialize};

use crate::{app::cli, logging};

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct Config {
    /// The sentry DSN to use for error reporting.
    pub sentry_dsn: Option<String>,

    /// The environment to report to sentry errors to.
    pub sentry_env: Option<Cow<'static, str>>,

    /// The log level to filter logging to.
    pub log_level: logging::Level,

    /// The log format to output.
    pub log_format: logging::LogFormat,

    /// The location of the entire music catalog.
    pub catalog_path: PathBuf,

    /// The location of music that can be imported.
    pub import_path: PathBuf,

    /// The location where the catalog database and cached files (such as artwork) are stored.
    pub data_path: PathBuf,

    /// File types to consider when scanning the catalog and import path for music.
    pub music_file_types: Vec<String>,
}

struct MusicFileTypes(Vec<String>);

impl Default for MusicFileTypes {
    fn default() -> Self {
        let defaults = vec![
            "wav".to_owned(),
            "aiff".to_owned(),
            "mp3".to_owned(),
            "flac".to_owned(),
        ];
        Self(defaults)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sentry_dsn: None,
            sentry_env: None,
            log_level: logging::Level::Info,
            log_format: logging::LogFormat::Auto,
            catalog_path: PathBuf::from_str("./catalog").unwrap(),
            import_path: PathBuf::from_str("./new-music").unwrap(),
            data_path: PathBuf::from_str("./data").unwrap(),
            music_file_types: Default::default(),
        }
    }
}

impl Config {
    /// Load configuration from an optional configuration file and environment
    pub fn extract(app: &cli::CliApp) -> figment::Result<Config> {
        let mut builder = Figment::from(Serialized::defaults(Config::default()));

        if let Some(path) = &app.config {
            builder = builder.merge(Yaml::file(path));
        };

        // Override some values from the CliApp
        if let Some(log_level) = app.log_level {
            builder = builder.merge(Serialized::default("log_level", log_level))
        }
        if let Some(log_format) = app.log_format {
            builder = builder.merge(Serialized::default("log_format", log_format))
        }

        let config: Config = builder.extract()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use figment::Jail;
    use pretty_assertions::assert_eq;

    use crate::{app::cli, logging};

    use super::Config;

    #[test]
    fn test_simple() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "config.yaml",
                r#"
                catalog_path: /home/evan/music
                import_path: /home/evan/new-music
                data_path: /home/evan/tunes-importer-data
                "#,
            )?;

            let app = cli::CliApp {
                config: Some(PathBuf::from("config.yaml")),
                log_level: None,
                log_format: None,
                command: cli::Commands::Server,
            };

            let config = Config::extract(&app).expect("Invalid configuration");

            assert_eq!(
                config,
                Config {
                    sentry_dsn: None,
                    sentry_env: None,
                    log_level: logging::Level::Info,
                    log_format: logging::LogFormat::Auto,
                    catalog_path: PathBuf::from_str("/home/evan/music").unwrap(),
                    import_path: PathBuf::from_str("/home/evan/new-music").unwrap(),
                    data_path: PathBuf::from_str("/home/evan/tunes-importer-data").unwrap(),
                    music_file_types: Default::default(),
                }
            );
            Ok(())
        });
    }

    #[test]
    fn test_overrides() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "config.yaml",
                r#"
                log_format: json
                "#,
            )?;

            let app = cli::CliApp {
                config: Some(PathBuf::from("config.yaml")),
                log_level: Some(logging::Level::Trace),
                log_format: None,
                command: cli::Commands::Server,
            };

            let config = Config::extract(&app).expect("Invalid configuration");

            assert_eq!(
                config,
                Config {
                    log_level: logging::Level::Trace,
                    log_format: logging::LogFormat::Json,
                    ..Default::default()
                }
            );
            Ok(())
        });
    }
}
