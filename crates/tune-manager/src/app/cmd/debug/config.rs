use std::io;

use crate::app::{
    cli::CliApp,
    config::{self, Config},
};

pub fn run(app: &CliApp, config: &Config) -> io::Result<()> {
    let config_path = config::get_config_path(app)
        .map_or_else(|| "<none>".to_string(), |p| p.to_string_lossy().to_string());

    println!("Config file: {}", config_path);
    println!();

    match serde_yaml::to_string(config) {
        Ok(yaml) => println!("{}", yaml),
        Err(e) => eprintln!("Error serializing config to YAML: {}", e),
    }

    Ok(())
}
