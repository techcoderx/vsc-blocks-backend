use crate::{ mongo::MongoDB, types::{ hive::DgpAtBlock, vsc::{ ElectionMember, LedgerBalance, WitnessStat, Witnesses } } };
use chrono::{ DateTime, NaiveDateTime, Utc };
use serde::Serialize;
use futures_util::StreamExt;
use std::error::Error as Error2;
use mongodb::{ bson::{ doc, Bson, Document }, error::Error, options::FindOneOptions };

#[derive(Clone, Serialize)]
pub struct Props {
  pub last_processed_block: i64,
  pub l2_block_height: u64,
  pub witnesses: i32,
  pub epoch: u64,
  pub contracts: u64,
  pub transactions: u64,
}

pub async fn get_props(db: &MongoDB) -> Result<Props, Error> {
  let db = db.clone();
  let pipeline = vec![doc! {
      "$group": {
        "_id": "$account"
      }
    }, doc! { "$count": "total" }];

  let mut wit_cursor = db.witnesses.aggregate(pipeline).await?;
  let witness_count = wit_cursor
    .next().await
    .transpose()?
    .map(|d| d.get_i32("total").unwrap_or(0))
    .unwrap_or(0);
  let contracts = db.contracts.count_documents(doc! { "latest": true }).await?;
  let epoch = db.elections.estimated_document_count().await?;
  let block_count = db.blocks.estimated_document_count().await?;
  let last_l1_block = match db.l1_blocks.find_one(doc! { "type": "metadata" }).await? {
    Some(state) => state.last_processed_block,
    None => 0,
  };
  let tx_count = db.tx_pool.estimated_document_count().await?;
  return Ok(Props {
    last_processed_block: last_l1_block,
    l2_block_height: block_count,
    witnesses: witness_count,
    epoch: epoch.saturating_sub(1),
    contracts,
    transactions: tx_count,
  });
}

pub async fn get_witness(db: &MongoDB, user: String) -> Result<Option<Witnesses>, Error> {
  let db = db.clone();
  let opt = FindOneOptions::builder()
    .sort(doc! { "height": -1 })
    .build();
  let result = db.witnesses.find_one(doc! { "account": &user }).with_options(opt).await?;
  Ok(result)
}

pub async fn get_witness_stats(db: &MongoDB, user: String) -> Result<WitnessStat, Error> {
  let db = db.clone();
  let stats = db.witness_stats.find_one(doc! { "_id": &user }).await?.unwrap_or(WitnessStat {
    proposer: user.clone(),
    block_count: None,
    election_count: None,
    last_block: None,
    last_epoch: None,
  });
  Ok(stats)
}

pub async fn get_user_balance(db: &MongoDB, user: String) -> Result<LedgerBalance, Error> {
  let db = db.clone();
  let opt = FindOneOptions::builder()
    .sort(doc! { "block_height": -1 })
    .build();
  let bal = db.ledger_bal
    .find_one(doc! { "account": user.clone() })
    .with_options(opt.clone()).await?
    .unwrap_or(LedgerBalance {
      hbd: 0,
      hbd_savings: 0,
      hive: 0,
      hive_consensus: 0,
    });
  Ok(bal)
}

pub async fn get_user_cons_unstaking(db: &MongoDB, user: String) -> Result<i64, Error> {
  let db = db.clone();
  let unstaking_pipeline = vec![
    doc! {
      "$match": doc! {
        "to": user.clone(),
        "status": "pending",
        "type": "consensus_unstake"
      }
    },
    doc! {
      "$group": doc! {
        "_id": Bson::Null,
        "totalAmount": doc! {"$sum": "$amount"}
      }
    }
  ];
  let mut unstaking_cursor = db.ledger_actions.aggregate(unstaking_pipeline).await?;
  Ok(
    unstaking_cursor
      .next().await
      .transpose()?
      .map(|d| d.get_i64("totalAmount").unwrap_or(0))
      .unwrap_or(0)
  )
}

pub async fn get_members_at_l1_block(db: &MongoDB, height: i64) -> Result<(i64, Vec<ElectionMember>, i64), Error> {
  let db = db.clone();
  let opt = FindOneOptions::builder()
    .sort(doc! { "block_height": -1 })
    .build();
  let epoch = db.elections.find_one(doc! { "block_height": {"$lt": height} }).with_options(opt).await?;
  match epoch {
    Some(e) => Ok((e.epoch, e.members, e.total_weight)),
    None => Ok((-1, vec![], 0)),
  }
}

pub async fn get_total_deposits(db: &MongoDB, asset: &str, start_block: u32, end_block: u32) -> Result<i64, Error> {
  let mut cursor = db.ledger.aggregate(
    vec![
      doc! { "$match": {"t": "deposit", "tk": asset, "block_height": {"$gte": start_block, "$lt": end_block}} },
      doc! { "$group": {"_id": Bson::Null, "total": {"$sum": "$amount"}} }
    ]
  ).await?;
  Ok(
    cursor
      .next().await
      .transpose()?
      .map(|d| d.get_i64("total").unwrap_or(0))
      .unwrap_or(0)
  )
}

pub async fn get_total_withdrawals(db: &MongoDB, asset: &str, start_block: u32, end_block: u32) -> Result<i64, Error> {
  let mut cursor = db.ledger_actions.aggregate(
    vec![
      doc! { "$match": {"type": "withdraw", "asset": asset, "block_height": {"$gte": start_block, "$lt": end_block}} },
      doc! { "$group": {"_id": Bson::Null, "total": {"$sum": "$amount"}} }
    ]
  ).await?;
  Ok(
    cursor
      .next().await
      .transpose()?
      .map(|d| d.get_i64("total").unwrap_or(0))
      .unwrap_or(0)
  )
}

pub async fn get_last_processed_block_ts(
  db: &MongoDB,
  http_client: &reqwest::Client,
  rpc: String
) -> Result<(u32, DateTime<Utc>), Box<dyn Error2 + Send + Sync>> {
  let db = db.clone();
  let http_client = http_client.clone();
  let last_processed_block = db.l1_blocks
    .find_one(doc! {}).await?
    .map(|s| s.last_processed_block)
    .unwrap_or(1);
  let current_state = http_client
    .get(format!("{}/hafah-api/global-state?block-num={}", rpc, last_processed_block.to_string()))
    .send().await?;
  let current_state = current_state.json::<DgpAtBlock>().await?;
  Ok((current_state.block_num, NaiveDateTime::parse_from_str(&current_state.created_at, "%Y-%m-%dT%H:%M:%S")?.and_utc()))
}

pub fn apply_block_range(filter: Document, bh_field: &str, from_blk: Option<i64>, to_blk: Option<i64>) -> Document {
  let mut filter = filter;
  if let Some(from) = from_blk {
    filter.insert(bh_field, doc! { "$gte": from });
  }
  if let Some(to) = to_blk {
    filter.insert(bh_field, doc! { "$lte": to });
  }
  filter
}
