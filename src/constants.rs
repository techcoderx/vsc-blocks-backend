use serde::{ Deserialize, Serialize };

use crate::config::config;

// bridge tx count tally interval (in seconds)
pub static BRIDGE_TXS_TALLY_INTERVAL: u64 = 600;

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkConsts {
  pub name: String,
  pub magi_explorer_url: String,
  pub l1_explorer_url: String,
  pub start_date: String,
}

pub fn mainnet_const() -> NetworkConsts {
  return NetworkConsts {
    name: format!("mainnet"),
    magi_explorer_url: format!("https://vsc.techcoderx.com"),
    l1_explorer_url: format!("https://hivehub.dev"),
    start_date: format!("2025-03-31"),
  };
}

pub fn testnet_const() -> NetworkConsts {
  return NetworkConsts {
    name: format!("testnet"),
    magi_explorer_url: format!("https://testnet.magi.techcoderx.com"),
    l1_explorer_url: format!("https://testnet.techcoderx.com/explorer"),
    start_date: format!("2026-01-24"),
  };
}

pub fn from_config() -> NetworkConsts {
  if config.network_conf.is_some() {
    return config.network_conf.clone().unwrap();
  }
  let net = config.network.clone().unwrap_or(format!("mainnet"));
  match net.as_str() {
    "mainnet" => mainnet_const(),
    "testnet" => testnet_const(),
    _ => panic!("invalid network"),
  }
}
