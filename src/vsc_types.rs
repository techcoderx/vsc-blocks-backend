use serde::{ Serialize, Deserialize };
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractById {
  pub error: Option<String>,
  pub contract_id: String,
  pub created_in_op: String,
  pub created_in_l1_block: usize,
  pub created_at: String,
  pub creator: String,
  pub name: String,
  pub description: String,
  pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResp {
  pub id: isize,
  pub jsonrpc: String,
  pub result: Option<Value>,
  pub error: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DgpAtBlock {
  pub block_num: u64,
  pub hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DIDKey {
  ct: String,
  t: String,
  key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Witnesses {
  account: String,
  height: i64,
  did_keys: Vec<DIDKey>,
  enabled: bool,
  gateway_key: String,
  git_commit: String,
  net_id: String,
  peer_addrs: Vec<String>,
  peer_id: String,
  protocol_version: i64,
  ts: String,
  version_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contract {
  pub id: String,
  pub code: String,
  pub tx_id: String,
  pub name: Option<String>,
  pub description: Option<String>,
  pub creator: String,
  pub owner: String,
  pub creation_height: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionMember {
  #[serde(rename = "key")]
  pub key: String,
  #[serde(rename = "account")]
  pub account: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionCommonInfo {
  #[serde(rename = "epoch")]
  pub epoch: u64,
  #[serde(rename = "net_id")]
  pub net_id: String,
  #[serde(rename = "type")]
  pub r#type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionHeaderInfo {
  #[serde(rename = "data")]
  pub data: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionHeader {
  #[serde(flatten)]
  pub common: ElectionCommonInfo,
  #[serde(flatten)]
  pub header_info: ElectionHeaderInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionDataInfo {
  #[serde(rename = "members")]
  pub members: Vec<ElectionMember>,
  #[serde(rename = "weights")]
  pub weights: Vec<u64>,
  #[serde(rename = "protocol_version")]
  pub protocol_version: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionData {
  #[serde(flatten)]
  pub common: ElectionCommonInfo,
  #[serde(flatten)]
  pub data_info: ElectionDataInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionResult {
  #[serde(flatten)]
  pub common: ElectionCommonInfo,
  #[serde(flatten)]
  pub header_info: ElectionHeaderInfo,
  #[serde(flatten)]
  pub data_info: ElectionDataInfo,

  #[serde(rename = "total_weight")]
  pub total_weight: u64,
  #[serde(rename = "block_height")]
  pub block_height: u64,
  #[serde(rename = "proposer")]
  pub proposer: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionResultRecord {
  #[serde(rename = "epoch")]
  pub epoch: u64,
  #[serde(rename = "net_id")]
  pub net_id: String,
  #[serde(rename = "data")]
  pub data: String,
  #[serde(rename = "members")]
  pub members: Vec<ElectionMember>,
  #[serde(rename = "weights")]
  pub weights: Vec<u64>,
  #[serde(rename = "protocol_version")]
  pub protocol_version: u64,
  #[serde(rename = "total_weight")]
  pub total_weight: u64,
  #[serde(rename = "block_height")]
  pub block_height: u64,
  #[serde(rename = "proposer")]
  pub proposer: String,
  #[serde(rename = "type")]
  pub r#type: String,
}
