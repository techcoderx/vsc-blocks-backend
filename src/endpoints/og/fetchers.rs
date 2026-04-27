use mongodb::bson::doc;
use serde::Deserialize;
use serde_json::{ json, Value };

use crate::{ config::config, mongo::MongoDB };

pub struct OgState {
  pub http_client: reqwest::Client,
  pub gql_api_url: String,
  pub hasura_url: String,
  pub haf_api_url: String,
}

async fn json_get<T: for<'de> Deserialize<'de>>(client: &reqwest::Client, url: &str) -> Option<T> {
  let res = client.get(url).header("accept", "application/json").send().await.ok()?;
  if !res.status().is_success() {
    return None;
  }
  res.json::<T>().await.ok()
}

async fn gql_query<T: for<'de> Deserialize<'de>>(
  client: &reqwest::Client,
  url: &str,
  query: &str,
  variables: Value
) -> Option<T> {
  #[derive(Deserialize)]
  struct GqlResp<D> {
    data: Option<D>,
  }
  let body = json!({ "query": query, "variables": variables });
  let res = client
    .post(url)
    .header("accept", "application/json")
    .header("content-type", "application/json")
    .json(&body)
    .send().await
    .ok()?;
  if !res.status().is_success() {
    return None;
  }
  let parsed: GqlResp<T> = res.json().await.ok()?;
  parsed.data
}

// ---- L1 Tx via hafah-api (uses the same hive_rpc the rest of the backend uses) ----

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct L1TxOperation(pub String, pub Value);

#[derive(Deserialize, Debug)]
pub struct L1TxJson {
  pub operations: Option<Vec<L1TxOperation>>,
}

#[derive(Deserialize, Debug)]
pub struct L1TxResult {
  pub block_num: Option<u64>,
  pub transaction_json: Option<L1TxJson>,
}

pub async fn fetch_l1_tx(state: &OgState, txid: &str) -> Option<L1TxResult> {
  let url = format!("{}/hafah-api/transactions/{}?include-virtual=true", config.hive_rpc, txid);
  json_get(&state.http_client, &url).await
}

// ---- L2 Tx via GQL ----

#[derive(Deserialize, Debug)]
pub struct L2Op {
  #[serde(rename = "type")]
  pub op_type: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct L2Tx {
  pub anchr_height: Option<u64>,
  pub status: Option<String>,
  pub ops: Option<Vec<L2Op>>,
}

#[derive(Deserialize)]
struct L2TxResp {
  txns: Vec<L2Tx>,
}

pub async fn fetch_l2_tx(state: &OgState, txid: &str) -> Option<L2Tx> {
  let query =
    r#"query Tx($opts: TransactionFilter) { txns: findTransaction(filterOptions: $opts) { id anchr_ts anchr_height status ops { type data } } }"#;
  let vars = json!({ "opts": { "byId": txid, "offset": 0, "limit": 1 } });
  let res: L2TxResp = gql_query(&state.http_client, &state.gql_api_url, query, vars).await?;
  res.txns.into_iter().next()
}

// ---- Block via direct DB (same backend) ----

#[derive(Debug, Clone)]
pub struct BlockInfo {
  pub block_id: Option<u32>,
  pub proposer: Option<String>,
  pub ts: Option<String>,
}

pub enum BlockLookup {
  Id(i32),
  Id1(String),
  Cid(String),
}

pub async fn fetch_block(db: &MongoDB, lookup: BlockLookup) -> Option<BlockInfo> {
  let filter = match lookup {
    BlockLookup::Id(n) => doc! { "be_info.block_id": n },
    BlockLookup::Id1(s) => doc! { "id": s },
    BlockLookup::Cid(s) => doc! { "block": s },
  };
  let block = db.blocks.find_one(filter).await.ok().flatten()?;
  Some(BlockInfo {
    block_id: block.be_info.as_ref().map(|b| b.block_id),
    proposer: Some(block.proposer),
    ts: Some(block.ts),
  })
}

// ---- Epoch via direct DB ----

#[derive(Debug, Clone)]
pub struct EpochInfo {
  pub block_height: Option<u64>,
  pub proposer: Option<String>,
}

pub async fn fetch_epoch(db: &MongoDB, num: i64) -> Option<EpochInfo> {
  let ep = db.elections.find_one(doc! { "epoch": num }).await.ok().flatten()?;
  Some(EpochInfo {
    block_height: Some(ep.block_height),
    proposer: Some(ep.proposer),
  })
}

// ---- L1 Account via HAF PostgREST ----

#[derive(Deserialize, Debug)]
pub struct L1Account {
  pub username: Option<String>,
}

pub async fn fetch_l1_account(state: &OgState, username: &str) -> Option<L1Account> {
  let url = format!("{}/rpc/get_l1_user", state.haf_api_url);
  let res = state.http_client
    .get(&url)
    .query(&[("username", username)])
    .header("accept", "application/json")
    .send().await
    .ok()?;
  if !res.status().is_success() {
    return None;
  }
  res.json::<L1Account>().await.ok()
}

// ---- Contract via direct DB ----

#[derive(Debug, Clone)]
pub struct ContractInfo {
  pub creator: Option<String>,
  pub creation_height: Option<i64>,
}

pub async fn fetch_contract(db: &MongoDB, contract_id: &str) -> Option<ContractInfo> {
  let c = db.contracts.find_one(doc! { "id": contract_id }).await.ok().flatten()?;
  Some(ContractInfo {
    creator: Some(c.creator),
    creation_height: Some(c.creation_height),
  })
}

// ---- Contract verification info via direct DB ----

#[derive(Debug, Clone)]
pub struct CvInfo {
  pub status: Option<String>,
}

pub async fn fetch_cv_info(db: &MongoDB, contract_id: &str) -> Option<CvInfo> {
  let c = db.cv_contracts.find_one(doc! { "contract_id": contract_id }).await.ok().flatten()?;
  Some(CvInfo { status: Some(c.status) })
}

// ---- Token via Hasura ----

#[derive(Deserialize, Debug)]
pub struct TokenInfo {
  pub name: Option<String>,
  pub symbol: Option<String>,
}

#[derive(Deserialize)]
struct TokenResp {
  magi_token_overview: Vec<TokenInfo>,
}

pub async fn fetch_token(state: &OgState, contract_id: &str) -> Option<TokenInfo> {
  let query =
    r#"query Token($id: String!) { magi_token_overview(where: {contract_id: {_eq: $id}}, limit: 1) { contract_id name symbol decimals max_supply current_supply owner } }"#;
  let vars = json!({ "id": contract_id });
  let res: TokenResp = gql_query(&state.http_client, &state.hasura_url, query, vars).await?;
  res.magi_token_overview.into_iter().next()
}

// ---- NFT via Hasura ----

#[derive(Deserialize, Debug)]
pub struct NftInfo {
  pub name: Option<String>,
  pub symbol: Option<String>,
}

#[derive(Deserialize)]
struct NftResp {
  magi_nft_registry: Vec<NftInfo>,
}

pub async fn fetch_nft(state: &OgState, contract_id: &str) -> Option<NftInfo> {
  let query =
    r#"query Nft($id: String!) { magi_nft_registry(where: {contract_id: {_eq: $id}}, limit: 1) { contract_id name symbol owner base_uri } }"#;
  let vars = json!({ "id": contract_id });
  let res: NftResp = gql_query(&state.http_client, &state.hasura_url, query, vars).await?;
  res.magi_nft_registry.into_iter().next()
}

// ---- Staking Claim via GQL ----

#[derive(Deserialize, Debug)]
pub struct StakingClaim {
  pub amount: Option<Value>,
  pub received_n: Option<u64>,
}

#[derive(Deserialize)]
struct ClaimResp {
  claims: Vec<StakingClaim>,
}

pub async fn fetch_staking_claim(state: &OgState, block_height: i64) -> Option<StakingClaim> {
  let query =
    r#"query Claim($opts: LedgerClaimFilter) { claims: findLedgerClaims(filterOptions: $opts) { block_height amount received_n observed_apr tx_id timestamp } }"#;
  let vars = json!({ "opts": { "byBlockHeight": block_height, "offset": 0, "limit": 1 } });
  let res: ClaimResp = gql_query(&state.http_client, &state.gql_api_url, query, vars).await?;
  res.claims.into_iter().next()
}
