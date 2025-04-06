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
  // pub id: String,
  pub json: String,
  pub required_auths: Vec<String>,
  // pub required_posting_auths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpHeader<T> {
  // #[serde(rename = "type")]
  // pub r#type: String,
  pub value: T,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpInBlock<T> {
  pub op: OpHeader<T>,
  // pub block: u32,
  pub trx_id: String,
  // pub op_pos: u8,
  // pub op_type_id: u16,
  pub timestamp: String,
  // pub virtual_op: bool,
  // pub operation_id: String,
  // pub trx_in_block: u16,
}

/**
For type safety, only one operation type is allowed (i.e. exactly ONE `operation-types` must be specified in API call)

Example API call: https://techcoderx.com/hafah-api/blocks/94713886/operations?operation-types=18&page=1&page-size=2000&page-order=asc&data-size-limit=2000000&path-filter=value.id%3Dvsc.election_result
*/
#[derive(Clone, Debug, Deserialize)]
pub struct OpsInBlock<T> {
  // pub total_operations: u32,
  // pub total_pages: u32,
  pub operations_result: Vec<OpInBlock<T>>,
}
