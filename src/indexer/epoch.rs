use futures_util::StreamExt;
use serde_json::{ Value, Number, from_value };
use tokio::{ time::{ sleep, Duration }, sync::RwLock };
use mongodb::{ bson::doc, Collection };
use reqwest;
use log::{ error, info };
use std::sync::Arc;
use bv_decoder::BvWeights;
use crate::{
  config::config,
  types::{ hive::{ CustomJson, TxByHash }, vsc::{ json_to_bson, ElectionResultRecord, IndexerState, Signature } },
};

#[derive(Clone)]
pub struct ElectionIndexer {
  http_client: reqwest::Client,
  elections_db: Collection<ElectionResultRecord>,
  indexer2: Collection<IndexerState>,
  is_running: Arc<RwLock<bool>>,
}

impl ElectionIndexer {
  pub fn init(
    http_client: reqwest::Client,
    elections_db: Collection<ElectionResultRecord>,
    indexer2: Collection<IndexerState>
  ) -> ElectionIndexer {
    return ElectionIndexer { http_client, elections_db, indexer2, is_running: Arc::new(RwLock::new(false)) };
  }

  pub fn start(&self) {
    let http_client = self.http_client.clone();
    let election_db = self.elections_db.clone();
    let indexer2 = self.indexer2.clone();
    let running = Arc::clone(&self.is_running);

    tokio::spawn(async move {
      info!("Begin indexing elections");
      {
        let mut r = running.write().await;
        *r = true;
      }
      let sync_state = indexer2.find_one(doc! { "_id": 0 }).await;
      if sync_state.is_err() {
        error!("{}", sync_state.unwrap_err());
        return;
      }
      let mut num = match sync_state.unwrap() {
        Some(state) => state.epoch,
        None => -1,
      };
      'mainloop: loop {
        let r = running.read().await;
        if !*r {
          break;
        }
        let next_epochs = election_db
          .find(doc! { "epoch": doc! {"$gt": num as i64} })
          .sort(doc! { "epoch": 1 })
          .limit(100).await;
        if next_epochs.is_err() {
          error!("{}", next_epochs.unwrap_err());
          sleep(Duration::from_secs(60)).await;
          continue;
        }
        let mut next_epochs = next_epochs.unwrap();
        let mut next_num = num;
        while let Some(ep) = next_epochs.next().await {
          if ep.is_err() {
            error!("Failed to deserialize election");
            break 'mainloop;
          }
          let epoch = ep.unwrap();
          next_num += 1;
          let tx = http_client
            .get(format!("{}/hafah-api/transactions/{}?include-virtual=false", config.hive_rpc.clone(), epoch.tx_id.clone()))
            .send().await;
          if tx.is_err() {
            error!("{}", tx.unwrap_err());
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          let tx = tx.unwrap().json::<TxByHash<CustomJson>>().await.unwrap();
          // there should be only one operation here
          let j = match serde_json::from_str::<Value>(&tx.transaction_json.operations[0].value.json) {
            Ok(json) => json,
            Err(e) => {
              error!("Failed to parse json, this is a fatal error likely caused by a bug in go-vsc-node. {}", e);
              break 'mainloop;
            }
          };
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
            let sig_obj = match signature {
              Some(sig) => from_value::<Option<Signature>>(sig.clone()).unwrap_or(None),
              None => None,
            };
            let weights = match sig_obj {
              Some(sign) => {
                let weights = match election_db.find_one(doc! { "epoch": (next_num as i64)-1 }).await {
                  Ok(pe) =>
                    match pe {
                      Some(pe) => pe.weights,
                      None => vec![],
                    }
                  Err(e) => {
                    error!("Failed to query previous epoch {}", e);
                    sleep(Duration::from_secs(60)).await;
                    continue 'mainloop;
                  }
                };
                match BvWeights::from_b64url(&sign.bv, &weights) {
                  Ok(bv) => (bv.voted_weight(), bv.eligible_weight()),
                  Err(_) => (0, 0),
                }
              }
              None => (0, 0),
            };
            let up = election_db
              .update_one(
                doc! { "epoch": epoch.epoch as i64 },
                doc! { "$set": doc! {
                  "be_info": doc! {
                    "ts": &tx.timestamp,
                    "signature": json_to_bson(signature),
                    "voted_weight": weights.0 as i64,
                    "eligible_weight": weights.1 as i64
                  }
                }}
              )
              .upsert(true).await;
            if up.is_err() {
              error!("Failed to update {}", up.unwrap_err());
              sleep(Duration::from_secs(120)).await;
              continue 'mainloop;
            }
          }
        }
        let upd_state = indexer2.update_one(doc! { "_id": 0 }, doc! { "$set": doc! { "epoch": next_num } }).upsert(true).await;
        if upd_state.is_err() {
          error!("Failed to update state {}", upd_state.unwrap_err());
          sleep(Duration::from_secs(120)).await;
          continue 'mainloop;
        }
        let processed = next_num - num;
        if processed > 0 {
          info!("Indexed {} epochs for BE API: ({},{}]", processed, num, next_num);
        }
        num = next_num;
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
