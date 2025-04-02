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
