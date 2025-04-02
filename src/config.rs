use serde_derive::{ Serialize, Deserialize };
use std::{ fs, error, env::current_dir };
use rand::Rng;
use hex;
use toml;
use clap::Parser;
use lazy_static::lazy_static;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
  #[arg(short, long, default_value = "config.toml")]
  pub config_file: String,
  #[arg(long)]
  /// Dump sample config file to config.toml
  pub dump_config: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
  pub address: String,
  pub port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct ASCompilerConf {
  pub image: String,
  pub src_dir: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuthConf {
  pub enabled: bool,
  pub id: Option<String>,
  pub timeout_blocks: Option<u64>,
  pub hive_rpc: Option<String>,
  pub key: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TomlConfig {
  pub log_level: Option<String>,
  pub psql_url: String,
  pub mongo_url: String,
  pub auth: AuthConf,
  pub server: ServerConfig,
  pub ascompiler: ASCompilerConf,
}

impl TomlConfig {
  pub fn read_from_file(file_path: &str) -> Result<Self, Box<dyn error::Error>> {
    // Read the TOML file contents
    let contents = fs::read_to_string(file_path)?;

    // Deserialize the TOML into the Config struct
    let deserialized: TomlConfig = toml::de::from_str(&contents)?;

    Ok(deserialized)
  }

  pub fn dump_config_file() {
    let default_conf = TomlConfig {
      log_level: Some(String::from("info")),
      psql_url: String::from("postgres://postgres:mysecretpassword@127.0.0.1:5432/postgres"),
      mongo_url: String::from("mongodb://localhost:27017"),
      auth: AuthConf {
        enabled: true,
        id: Some(String::from("vsc_cv_login")),
        timeout_blocks: Some(20),
        hive_rpc: Some(String::from("https://techcoderx.com")),
        key: Some(hex::encode(rand::rng().random::<[u8; 32]>())),
      },
      server: ServerConfig { address: String::from("127.0.0.1"), port: 8080 },
      ascompiler: ASCompilerConf {
        image: String::from("as-compiler"),
        src_dir: format!("{}/as_compiler", current_dir().unwrap().to_str().unwrap()),
      },
    };
    let serialized = toml::ser::to_string(&default_conf).unwrap();
    let _ = fs::write(Args::parse().config_file, serialized);
  }
}

lazy_static! {
  pub static ref config: TomlConfig = TomlConfig::read_from_file(Args::parse().config_file.as_str()).expect(
    "Failed to load config. Use --dump-config to generate config file."
  );
}
