use futures_util::StreamExt;
use serde_json::Value;
use tokio::{ time::{ sleep, Duration }, sync::RwLock };
use mongodb::{ bson::{ doc, Bson }, options::FindOneOptions, Collection };
use reqwest;
use log::{ error, info };
use std::sync::Arc;
use bv_decoder::BvWeights;
use crate::{
  config::config,
  types::{ hive::{ CustomJson, TxByHash }, vsc::{ json_to_bson, BlockHeaderRecord, IndexerState, ElectionResultRecord } },
};

#[derive(Clone)]
pub struct BlockIndexer {
  http_client: reqwest::Client,
  blocks_db: Collection<BlockHeaderRecord>,
  elections_db: Collection<ElectionResultRecord>,
  indexer2: Collection<IndexerState>,
  is_running: Arc<RwLock<bool>>,
}

impl BlockIndexer {
  pub fn init(
    http_client: reqwest::Client,
    blocks_db: Collection<BlockHeaderRecord>,
    elections_db: Collection<ElectionResultRecord>,
    indexer2: Collection<IndexerState>
  ) -> BlockIndexer {
    return BlockIndexer { http_client, blocks_db, elections_db, indexer2, is_running: Arc::new(RwLock::new(false)) };
  }

  pub fn start(&self) {
    let http_client = self.http_client.clone();
    let blocks_db = self.blocks_db.clone();
    let election_db = self.elections_db.clone();
    let indexer2 = self.indexer2.clone();
    let running = Arc::clone(&self.is_running);

    tokio::spawn(async move {
      info!("Begin indexing L2 blocks");
      {
        let mut r = running.write().await;
        *r = true;
      }
      let sync_state = indexer2.find_one(doc! { "_id": 0 }).await;
      if sync_state.is_err() {
        error!("{}", sync_state.unwrap_err());
        return;
      }
      let mut nums = match sync_state.unwrap() {
        Some(state) => (state.l1_height, state.l2_height),
        None => (0, 0),
      };
      'mainloop: loop {
        let r = running.read().await;
        if !*r {
          break;
        }
        let next_blocks = blocks_db
          .find(doc! { "slot_height": doc! {"$gt": nums.0} })
          .sort(doc! { "slot_height": 1 })
          .limit(100).await;
        if next_blocks.is_err() {
          error!("{}", next_blocks.unwrap_err());
          sleep(Duration::from_secs(60)).await;
          continue;
        }
        let mut next_blocks = next_blocks.unwrap();
        let mut next_nums = (nums.0, nums.1);
        while let Some(b) = next_blocks.next().await {
          if b.is_err() {
            error!("Failed to deserialize block header");
            break 'mainloop;
          }
          let block = b.unwrap();
          next_nums.1 += 1;
          let tx = http_client
            .get(format!("{}/hafah-api/transactions/{}?include-virtual=false", config.hive_rpc.clone(), block.id.clone()))
            .send().await;
          if tx.is_err() {
            error!("{}", tx.unwrap_err());
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          let tx = tx.unwrap().json::<TxByHash<CustomJson>>().await.unwrap();
          // there should be one operation, otherwise this is a bug with go-vsc-node
          let j = serde_json::from_str::<Value>(&tx.transaction_json.operations[0].value.json);
          if j.is_err() {
            error!("Failed to parse json, this is a fatal error likely caused by a bug in go-vsc-node.");
            break 'mainloop;
          }
          let j = j.unwrap();
          let sb = j.get("signed_block");
          if sb.is_none() {
            error!("signed_block is missing, this is also a fatal error likely caused by a bug in go-vsc-node.");
            break 'mainloop;
          }
          let sb = sb.unwrap();
          let signature = sb.get("signature");
          if signature.is_none() {
            error!("No signature for block in tx {}?!", block.id);
            break 'mainloop;
          }
          let epoch = election_db.find_one(doc! { "block_height": doc! {"$lt": block.slot_height} }).with_options(
            FindOneOptions::builder()
              .sort(doc! { "block_height": -1 })
              .build()
          ).await;
          if epoch.is_err() {
            error!("Failed to qeury epoch in block: {}", epoch.unwrap_err());
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          let epoch = epoch.unwrap().unwrap();
          let bv = BvWeights::from_b64url(signature.unwrap().get("bv").unwrap().as_str().unwrap(), &epoch.weights);
          if bv.is_err() {
            error!("Failed to decode bv: {}", bv.unwrap_err());
            break 'mainloop;
          }
          let bv = bv.unwrap();
          let up = blocks_db
            .update_one(
              doc! { "block": block.block.clone() },
              doc! { "$set": 
                doc!{
                  "be_info": doc! {
                    "block_id": next_nums.1,
                    "epoch": epoch.epoch as i32,
                    "signature": json_to_bson(signature),
                    "voted_weight": Bson::from(bv.voted_weight() as i64),
                    "eligible_weight": Bson::from(bv.eligible_weight() as i64)
                  }
                }
              }
            )
            .upsert(true).await;
          if up.is_err() {
            error!("Failed to update {}", up.unwrap_err());
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          next_nums.0 = block.slot_height;
        }
        let upd_state = indexer2
          .update_one(doc! { "_id": 0 }, doc! { "$set": doc! { "l1_height": next_nums.0, "l2_height": next_nums.1 } })
          .upsert(true).await;
        if upd_state.is_err() {
          error!("Failed to update state {}", upd_state.unwrap_err());
          sleep(Duration::from_secs(120)).await;
          continue 'mainloop;
        }
        let processed = next_nums.1 - nums.1;
        if processed > 0 {
          info!("Indexed {} L2 blocks for BE API: ({},{}]", processed, nums.1, next_nums.1);
        }
        nums = (next_nums.0, next_nums.1);
        let r = running.read().await;
        if processed < 100 && *r {
          sleep(Duration::from_secs(30)).await;
        }
      }
      let mut r = running.write().await;
      *r = false;
    });
  }
}
