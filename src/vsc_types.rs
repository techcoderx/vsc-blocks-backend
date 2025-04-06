use serde::{ Serialize, Deserialize };

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature {
  pub sig: String,
  pub bv: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionMember {
  pub key: String,
  pub account: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElectionResultRecord {
  pub epoch: u64,
  pub net_id: String,
  pub data: String,
  pub members: Vec<ElectionMember>,
  pub weights: Vec<u64>,
  pub protocol_version: u64,
  pub total_weight: u64,
  pub block_height: u64,
  pub proposer: String,
  #[serde(rename = "type")]
  pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElectionExt {
  #[serde(rename = "_id")]
  pub epoch: u64,
  pub ts: String,
  pub trx_id: String,
  pub signature: Option<Signature>,
  pub voted_weight: u64,
  pub eligible_weight: u64,
}
