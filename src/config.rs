use serde_derive::Deserialize;
use std::{ fs, error };
use toml;
use clap::Parser;
use lazy_static::lazy_static;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  #[arg(short, long, default_value = "config.toml")]
  config_file: String,
}

#[derive(Deserialize)]
pub struct ServerConfig {
  pub address: String,
  pub port: u16,
}

#[derive(Deserialize)]
pub struct TomlConfig {
  pub log_level: Option<String>,
  pub psql_url: String,
  pub server: ServerConfig,
}

impl TomlConfig {
  pub fn read_from_file(file_path: &str) -> Result<Self, Box<dyn error::Error>> {
    // Read the TOML file contents
    let contents = fs::read_to_string(file_path)?;

    // Deserialize the TOML into the Config struct
    let deserialized: TomlConfig = toml::de::from_str(&contents)?;

    Ok(deserialized)
  }
}

lazy_static! {
  pub static ref config: TomlConfig = TomlConfig::read_from_file(Args::parse().config_file.as_str()).expect(
    "Failed to load config"
  );
}
