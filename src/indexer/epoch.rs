use futures_util::StreamExt;
use serde_json::{ Value, from_value };
use tokio::{ time::{ sleep, Duration }, sync::RwLock };
use mongodb::bson::doc;
use log::{ error, info };
use std::sync::Arc;
use bv_decoder::BvWeights;
use crate::{ haf::HAFDB, mongo::MongoDB, types::vsc::{ json_to_bson, Signature } };

#[derive(Clone)]
pub struct ElectionIndexer {
  db: MongoDB,
  haf: HAFDB,
  is_running: Arc<RwLock<bool>>,
}

impl ElectionIndexer {
  pub fn init(db: &MongoDB, haf: &HAFDB) -> ElectionIndexer {
    return ElectionIndexer { db: db.clone(), haf: haf.clone(), is_running: Arc::new(RwLock::new(false)) };
  }

  pub fn start(&self) {
    let haf = self.haf.clone();
    let election_db = self.db.elections.clone();
    let indexer2 = self.db.indexer2.clone();
    let witness_stats = self.db.witness_stats.clone();
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
        Some(state) => state.epoch.unwrap_or(-1),
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
        let mut epochs = Vec::new();
        while let Some(ep) = next_epochs.next().await {
          if ep.is_err() {
            error!("Failed to deserialize election: {}", ep.unwrap_err().to_string());
            break 'mainloop;
          }
          epochs.push(ep.unwrap());
        }
        let mut hashes = Vec::with_capacity(epochs.len());
        for ep in &epochs {
          match hex::decode(ep.tx_id.strip_prefix("0x").unwrap_or(&ep.tx_id)) {
            Ok(v) => hashes.push(v),
            Err(e) => {
              error!("Failed to decode transaction id {}: {}", ep.tx_id, e);
              sleep(Duration::from_secs(60)).await;
              continue 'mainloop;
            }
          }
        }
        let txs = haf.get_custom_json_txs(&hashes).await;
        if txs.is_err() {
          error!("Failed to fetch transaction details from HAF: {}", txs.unwrap_err());
          sleep(Duration::from_secs(120)).await;
          continue 'mainloop;
        }
        let mut txs = txs.unwrap();
        let mut next_num = num;
        for (i, epoch) in epochs.into_iter().enumerate() {
          next_num += 1;
          let tx = match txs.remove(&hashes[i]) {
            Some(t) => t,
            None => {
              error!("No transaction details found in HAF for tx {}", epoch.tx_id);
              sleep(Duration::from_secs(60)).await;
              continue 'mainloop;
            }
          };
          // there should be only one operation here
          let j = match serde_json::from_str::<Value>(&tx.json) {
            Ok(json) => json,
            Err(e) => {
              error!("Failed to parse json, this is a fatal error likely caused by a bug in go-vsc-node. {}", e);
              break 'mainloop;
            }
          };
          let signature = j.get("signature");
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
                    "ts": tx.timestamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
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
          match witness_stats.find_one(doc! { "_id": &epoch.proposer }).await {
            Ok(last_stat) => {
              if last_stat.is_none() || (last_stat.unwrap().last_epoch.unwrap_or(-1) as i64) < epoch.epoch {
                let _ = witness_stats
                  .update_one(
                    doc! { "_id": &epoch.proposer },
                    doc! {
                      "$set": doc! {"last_epoch": epoch.epoch as i32},
                      "$inc": doc! {"election_count": 1}
                    }
                  )
                  .upsert(true).await;
              }
            }
            Err(_) => (),
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
