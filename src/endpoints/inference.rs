use mongodb::bson;
use reqwest;
use mongodb::{ bson::doc, Collection };
use serde_json::{ Number, Value, from_value, json };
use std::{ error, fmt };
use crate::vsc_types::{ ElectionExt, ElectionResultRecord, Signature };
use crate::hive_types::{ OpsInBlock, CustomJson };
use crate::config::config;

const REQ_ERR: &str = "Failed to make request for inference";
const PARSE_ERR: &str = "Failed to parse request response for inference";
const NOT_FOUND_ERR: &str = "Failed to infer the transaction";
const DB_ERR: &str = "Failed to query db2";

#[derive(Debug)]
pub struct InferError {
  message: String,
}

impl error::Error for InferError {}
impl fmt::Display for InferError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

fn json_to_bson(option_json: Option<&Value>) -> bson::Bson {
  match option_json {
    Some(json_val) => bson::to_bson(json_val).expect("Failed to convert JSON to BSON"),
    None => bson::Bson::Null,
  }
}

pub async fn infer_epoch(
  http_client: &reqwest::Client,
  elections2: &Collection<ElectionExt>,
  epoch: &ElectionResultRecord
) -> Result<ElectionExt, InferError> {
  let epoch = epoch.clone();
  let elections2 = elections2.clone();
  let stored = elections2
    .find_one(doc! { "_id": epoch.epoch as i64 }).await
    .map_err(|_| InferError { message: String::from(DB_ERR) })?;
  if stored.is_some() {
    return Ok(stored.unwrap());
  }
  let block = http_client
    .clone()
    .get(
      format!(
        "{}/hafah-api/blocks/{}/operations?operation-types=18&page=1&page-size=2000&page-order=asc&data-size-limit=2000000&path-filter=value.id%3Dvsc.election_result",
        config.hive_rpc.clone(),
        epoch.block_height.to_string()
      )
    )
    .send().await
    .map_err(|_| InferError { message: String::from(REQ_ERR) })?
    .json::<OpsInBlock<CustomJson>>().await
    .map_err(|_| InferError { message: String::from(PARSE_ERR) })?;
  for op in block.operations_result.iter() {
    let op = op.clone();
    let operation = op.op.value;
    if operation.required_auths.len() > 0 && operation.required_auths[0] == epoch.proposer {
      let j = serde_json::from_str::<Value>(&operation.json);
      if j.is_ok() {
        let j = j.unwrap();
        let net_id_valid = match j.get("net_id") {
          Some(n) => n.as_str().unwrap_or("") == "vsc-mainnet",
          None => false,
        };
        let data_match = match j.get("data") {
          Some(n) => n.as_str().unwrap_or("") == epoch.data,
          None => false,
        };
        let epoch_num_match = match j.get("epoch") {
          Some(n) => n.as_number().unwrap_or(&Number::from(0)).as_u64().unwrap_or(0) == epoch.epoch,
          None => false,
        };
        let signature = j.get("signature");
        if net_id_valid && data_match && epoch_num_match {
          let _ = elections2
            .update_one(
              doc! { "_id": epoch.epoch as i32 },
              doc! {
                "$set": doc! {
                  "ts": op.timestamp.clone(),
                  "trx_id": op.trx_id.clone(),
                  "signature": json_to_bson(signature),
                  "voted_weight": 0,
                  "eligible_weight": 0
                }
              }
            )
            .upsert(true).await;
          let sign = match signature {
            Some(s) => from_value::<Option<Signature>>(s.clone()).unwrap_or(None),
            None => None,
          };
          return Ok(ElectionExt {
            epoch: epoch.epoch,
            ts: op.timestamp,
            trx_id: op.trx_id,
            signature: sign,
            voted_weight: 0,
            eligible_weight: 0,
          });
        }
      }
    }
  }
  Err(InferError { message: String::from(NOT_FOUND_ERR) })
}

pub fn combine_inferred_epoch(original: &ElectionResultRecord, inferred: &ElectionExt) -> Value {
  let original = original.clone();
  let inferred = inferred.clone();
  json!({
    "epoch": original.epoch,
    "data": original.data,
    "members": original.members,
    "weights": original.weights,
    "total_weight": original.total_weight,
    "protocol_version": original.protocol_version,
    "proposer": original.proposer,
    "block_height": original.block_height,
    "trx_id": inferred.trx_id,
    "ts": inferred.ts,
    "type": original.r#type,
    "signature": inferred.signature,
    "eligible_weight": inferred.eligible_weight,
    "voted_weight": inferred.voted_weight
  })
}
