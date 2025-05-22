use crate::mongo::MongoDB;
use serde::Serialize;
use futures_util::StreamExt;
use mongodb::{ error::Error, bson::doc };

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
