use crate::{ mongo::MongoDB, types::vsc::{ LedgerBalance, WitnessStat, Witnesses } };
use serde::Serialize;
use futures_util::StreamExt;
use mongodb::{ bson::{ doc, Bson }, error::Error, options::FindOneOptions };

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
  let contracts = db.contracts.estimated_document_count().await?;
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
