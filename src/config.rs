use serde_derive::{ Serialize, Deserialize };
use std::{ fs, error, env::{ current_dir, set_var }, path::Path, process };
use env_logger;
use log::{ info, warn };
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

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerConf {
  pub enabled: Option<bool>,
  pub github_api_key: Option<String>,
  pub wasm_strip: String,
  pub wasm_tools: String,
  pub whitelist: Vec<String>,
  pub fix_permissions: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GoCompilerConf {
  pub src_dir: String,
  pub src_host_dir: Option<String>,
  pub output_dir: String,
  pub output_host_dir: Option<String>,
  pub timeout: usize,
}

#[derive(Serialize, Deserialize)]
pub struct AuthConf {
  pub enabled: bool,
  pub id: Option<String>,
  pub timeout_blocks: Option<u64>,
  pub key: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiscordConf {
  pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct TomlConfig {
  pub log_level: Option<String>,
  pub mongo_url: String,
  pub hive_rpc: String,
  pub be_indexer: Option<bool>,
  pub auth: AuthConf,
  pub server: ServerConfig,
  pub compiler: Option<CompilerConf>,
  pub gocompiler: GoCompilerConf,
  pub discord: Option<DiscordConf>,
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
    set_var("RUST_LOG", String::from("info"));
    env_logger::init();
    let filepath = Args::parse().config_file;
    if !Path::new(&filepath).exists() {
      let default_conf = TomlConfig {
        log_level: Some(String::from("info")),
        mongo_url: String::from("mongodb://localhost:27017"),
        hive_rpc: String::from("https://techcoderx.com"),
        be_indexer: None,
        auth: AuthConf {
          enabled: true,
          id: Some(String::from("vsc_cv_login")),
          timeout_blocks: Some(20),
          key: Some(hex::encode(rand::rng().random::<[u8; 32]>())),
        },
        server: ServerConfig { address: String::from("127.0.0.1"), port: 8080 },
        compiler: Some(CompilerConf {
          enabled: Some(false),
          github_api_key: None,
          wasm_strip: format!("wasm-strip"),
          wasm_tools: format!("wasm-tools"),
          whitelist: Vec::new(),
          fix_permissions: Some(false),
        }),
        gocompiler: GoCompilerConf {
          src_dir: format!("{}/go_compiler", current_dir().unwrap().to_str().unwrap()),
          src_host_dir: None,
          output_dir: format!("{}/artifacts", current_dir().unwrap().to_str().unwrap()),
          output_host_dir: None,
          timeout: 10,
        },
        discord: None,
      };
      let serialized = toml::ser::to_string(&default_conf).unwrap();
      let _ = fs::write(&filepath, serialized);
      info!("Dumped sample config file to {}", &filepath);
    } else {
      warn!("Config file already exists, doing nothing.");
    }
    process::exit(0);
  }
}

lazy_static! {
  pub static ref config: TomlConfig = TomlConfig::read_from_file(Args::parse().config_file.as_str()).expect(
    "Failed to load config. Use --dump-config to generate config file."
  );
}
