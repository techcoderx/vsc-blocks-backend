use serde::{ Serialize, Deserialize };
use serde_json::Value;

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

#[derive(Clone, Debug, Deserialize)]
pub struct CustomJson {
  pub id: String,
  pub json: String,
  // pub required_auths: Vec<String>,
  // pub required_posting_auths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpHeader<T> {
  #[serde(rename = "type")]
  pub r#type: String,
  pub value: T,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Transaction<T> {
  pub operations: Vec<OpHeader<T>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TxByHash<T> {
  pub transaction_json: Transaction<T>,
  pub timestamp: String,
}
