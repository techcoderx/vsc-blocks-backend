use serde::{ Serialize, Deserialize };
use serde_json::Value;
use mongodb::bson;

#[derive(Clone, Debug, Deserialize)]
pub struct HiveBlocksSyncState {
  pub last_processed_block: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IndexerState {
  pub l1_height: u32,
  pub l2_height: u32,
  pub epoch: i32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BridgeStats {
  pub deposits: i64,
  pub withdrawals: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LedgerActions {
  pub id: String,
  pub amount: u64,
  pub asset: String,
  pub block_height: u64,
  // pub  data: { epoch: 5 },
  pub memo: String,
  pub status: String,
  pub to: String,
  #[serde(rename = "type")]
  pub r#type: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Ledger {
  pub id: String,
  pub from: String,
  pub owner: String,
  pub amount: u64,
  #[serde(rename = "tk")]
  pub asset: String,
  pub block_height: u64,
  pub t: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LedgerBalance {
  pub hbd: u64,
  pub hbd_savings: u64,
  pub hive: u64,
  pub hive_consensus: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LedgerOpLog {
  pub to: String,
  pub from: String,
  pub amount: u64,
  pub asset: String,
  pub memo: String,
  #[serde(rename = "type")]
  pub r#type: String,
  pub id: String,
  pub bidx: u64,
  pub opidx: u64,
  pub blockheight: u64,
  pub params: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DIDKey {
  ct: String,
  t: String,
  key: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Witnesses {
  pub account: String,
  pub height: i64,
  pub did_keys: Vec<DIDKey>,
  pub enabled: bool,
  pub gateway_key: String,
  pub git_commit: String,
  pub net_id: String,
  pub peer_addrs: Vec<String>,
  pub peer_id: String,
  pub protocol_version: i64,
  pub ts: String,
  pub tx_id: String,
  pub version_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WitnessStat {
  #[serde(rename = "_id")]
  pub proposer: String,
  pub block_count: Option<i32>,
  pub election_count: Option<i32>,
  pub last_block: Option<i32>,
  pub last_epoch: Option<i32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Contract {
  pub id: String,
  pub code: String,
  pub tx_id: String,
  pub name: Option<String>,
  pub description: Option<String>,
  pub creator: String,
  pub owner: String,
  pub creation_height: i64,
  pub runtime: ContractRuntime,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ContractRuntime {
  pub value: String,
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
  pub epoch: i64,
  pub net_id: String,
  pub data: String,
  pub members: Vec<ElectionMember>,
  pub weights: Vec<u64>,
  pub protocol_version: u64,
  pub total_weight: u64,
  pub block_height: u64,
  pub proposer: String,
  pub tx_id: String,
  #[serde(rename = "type")]
  pub r#type: String,
  pub be_info: Option<ElectionExt>,
  pub blocks_info: Option<EpochBlocksInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElectionExt {
  pub ts: String,
  pub signature: Option<Signature>,
  pub voted_weight: u64,
  pub eligible_weight: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochBlocksInfo {
  pub count: i32,
  pub total_votes: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlockStat {
  pub size: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlockHeaderRecord {
  pub id: String,
  pub block: String,
  pub end_block: u32,
  pub merkle_root: String,
  pub proposer: String,
  pub sig_root: Option<String>,
  pub signers: Option<String>,
  pub slot_height: u32,
  pub start_block: u32,
  pub stats: BlockStat,
  pub ts: String,
  pub be_info: Option<BlockIndexed>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockIndexed {
  pub block_id: u32,
  pub epoch: u32,
  pub signature: Signature,
  pub voted_weight: u64,
  pub eligible_weight: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
  #[serde(rename = "id")]
  pub id: String,
  pub status: String,
  #[serde(rename = "required_auths")]
  pub required_auths: Option<Vec<String>>,
  pub nonce: Option<i64>,
  #[serde(rename = "rc_limit")]
  pub rc_limit: Option<u64>,
  // pub data: Document,
  // #[serde(rename = "anchr_block")]
  // pub anchored_block: String,
  // #[serde(rename = "anchr_id")]
  // pub anchored_id: String,
  // #[serde(rename = "anchr_index")]
  // pub anchored_index: i64,
  // #[serde(rename = "anchr_opidx")]
  // pub anchored_opidx: i64,
  // #[serde(rename = "anchr_height")]
  // pub anchored_height: u64,
  // #[serde(rename = "first_seen")]
  // pub first_seen: DateTime<Utc>,
  pub output: Option<Output>,
  pub ledger: Option<Vec<LedgerOpLog>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Output {
  #[serde(rename = "id")]
  pub id: String,
  pub index: i64,
}

#[derive(Serialize)]
pub struct UserStats {
  pub txs: u64,
  pub ledger_txs: u64,
  pub ledger_actions: u64,
  pub deposits: u64,
  pub withdrawals: u64,
}

pub fn json_to_bson(option_json: Option<&Value>) -> bson::Bson {
  match option_json {
    Some(json_val) => bson::to_bson(json_val).expect("Failed to convert JSON to BSON"),
    None => bson::Bson::Null,
  }
}
