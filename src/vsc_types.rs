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
