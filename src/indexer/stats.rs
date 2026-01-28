use mongodb::bson::doc;
use tokio::{ join, sync::RwLock, time::{ sleep, Duration } };
use std::sync::Arc;
use chrono::{ Datelike, Days };
use log::{ error, info };
use crate::{
  config::config,
  constants::from_config,
  helpers::{
    datetime::*,
    db::{ get_last_processed_block_ts, get_members_at_l1_block, get_total_deposits, get_total_withdrawals },
  },
  mongo::MongoDB,
  types::hive::DgpAtBlock,
};

#[derive(Clone)]
pub struct NetworkStatsIndexer {
  http_client: reqwest::Client,
  db: MongoDB,
  is_running: Arc<RwLock<bool>>,
}

impl NetworkStatsIndexer {
  pub fn init(http_client: &reqwest::Client, db: &MongoDB) -> NetworkStatsIndexer {
    return NetworkStatsIndexer {
      http_client: http_client.clone(),
      db: db.clone(),
      is_running: Arc::new(RwLock::new(false)),
    };
  }
  pub fn start(&self) {
    let http_client = self.http_client.clone();
    let db = self.db.clone();
    let running = Arc::clone(&self.is_running);
    let start_date = from_config().start_date.clone();

    tokio::spawn(async move {
      info!("Begin indexing daily network stats");
      {
        let mut r = running.write().await;
        *r = true;
      }
      let sync_state = db.indexer2.find_one(doc! { "_id": 0 }).await;
      if sync_state.is_err() {
        error!("{}", sync_state.unwrap_err());
        return;
      }
      let mut last_date = match sync_state.unwrap() {
        Some(state) => state.network_stats_date.to_chrono(),
        None => parse_date_str(&start_date).expect("Failed to parse stats start date"),
      };
      'mainloop: loop {
        let r = running.read().await;
        if !*r {
          break;
        }
        let head = get_last_processed_block_ts(&db, &http_client, config.hive_rpc.clone()).await;
        let (_, head_time) = match head {
          Err(_) => {
            error!("Failed to query last processed state");
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          Ok(s) => s,
        };
        let date = last_date.checked_add_days(Days::new(1)).expect("Failed to increment day");
        if date.date_naive() < head_time.date_naive() {
          let date_str = format_date(date.day(), date.month(), date.year());
          let next_date = date.checked_add_days(Days::new(1)).expect("Failed to get following date");
          let next_date_str = format_date(next_date.day(), next_date.month(), next_date.year());
          let (start_block, end_block) = join!(
            http_client.get(format!("{}/hafah-api/global-state?block-num={}", config.hive_rpc, &date_str)).send(),
            http_client.get(format!("{}/hafah-api/global-state?block-num={}", config.hive_rpc, &next_date_str)).send()
          );
          if start_block.is_err() || end_block.is_err() {
            error!("Failed to query start or end block for {}", &date_str);
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          let (start_block, end_block) = join!(
            start_block.unwrap().json::<DgpAtBlock>(),
            end_block.unwrap().json::<DgpAtBlock>()
          );
          if start_block.is_err() || end_block.is_err() {
            error!("Failed to parse start or end block for date {}", date_str);
            sleep(Duration::from_secs(60)).await;
            continue 'mainloop;
          }
          let start_block = start_block.unwrap().block_num;
          let end_block = end_block.unwrap().block_num;
          info!("Processing stats for {} range [{},{})", &date_str, start_block, end_block);
          let (
            txs,
            lg_txs,
            lg_actions,
            deposits,
            dep_hive,
            dep_hbd,
            withdrawals,
            wd_hive,
            wd_hbd,
            blocks,
            contracts,
            members,
            active_l1_addr,
            active_l2_addr,
          ) = join!(
            db.tx_pool.count_documents(doc! { "anchr_height": { "$gte": start_block, "$lt": end_block } }),
            db.ledger.count_documents(doc! { "block_height": { "$gte": start_block, "$lt": end_block } }),
            db.ledger_actions.count_documents(doc! { "block_height": { "$gte": start_block, "$lt": end_block } }),
            db.ledger.count_documents(doc! { "t": "deposit", "block_height": {"$gte": start_block, "$lt": end_block} }),
            get_total_deposits(&db, "hive", start_block, end_block),
            get_total_deposits(&db, "hbd", start_block, end_block),
            db.ledger_actions.count_documents(
              doc! { "block_height": {"$gte": start_block, "$lt": end_block}, "type": "withdraw" }
            ),
            get_total_withdrawals(&db, "hive", start_block, end_block),
            get_total_withdrawals(&db, "hbd", start_block, end_block),
            db.blocks.count_documents(doc! { "slot_height": {"$gte": start_block, "$lt": end_block} }),
            db.contracts.count_documents(doc! { "creation_height": {"$gte": start_block, "$lt": end_block} }),
            get_members_at_l1_block(&db, end_block as i64),
            db.tx_pool.distinct(
              "required_auths",
              doc! { "type": "hive", "anchr_height": {"$gte": start_block, "$lt": end_block} }
            ),
            db.tx_pool.distinct("required_auths", doc! { "type": "vsc", "anchr_height": {"$gte": start_block, "$lt": end_block} })
          );
          if
            txs.is_err() ||
            lg_txs.is_err() ||
            lg_actions.is_err() ||
            deposits.is_err() ||
            dep_hive.is_err() ||
            dep_hbd.is_err() ||
            withdrawals.is_err() ||
            wd_hive.is_err() ||
            wd_hbd.is_err() ||
            blocks.is_err() ||
            contracts.is_err() ||
            members.is_err() ||
            active_l1_addr.is_err() ||
            active_l2_addr.is_err()
          {
            error!("Failed to collect stats for date {}", date_str);
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          let (_epoch, members, total_weights) = members.unwrap();
          let stat_day = db.network_stats
            .update_one(
              doc! { "_id": bson::DateTime::from_chrono(date) },
              doc! { "$set": doc! {
                "txs": txs.unwrap() as i64,
                "ledger_txs": lg_txs.unwrap() as i64,
                "ledger_actions": lg_actions.unwrap() as i64,
                "deposits": deposits.unwrap() as i32,
                "deposits_hive": dep_hive.unwrap(),
                "deposits_hbd": dep_hbd.unwrap(),
                "withdrawals": withdrawals.unwrap() as i32,
                "withdrawals_hive": wd_hive.unwrap(),
                "withdrawals_hbd": wd_hbd.unwrap(),
                "blocks": blocks.unwrap() as i32,
                "witnesses": members.len() as i32,
                "contracts": contracts.unwrap() as i32,
                "active_stake": total_weights,
                "active_l1_addresses": active_l1_addr.unwrap().len() as i32,
                "active_l2_addresses": active_l2_addr.unwrap().len() as i32,
              }}
            )
            .upsert(true).await;
          if stat_day.is_err() {
            error!("Failed to insert stats for date {}: {}", date_str, stat_day.unwrap_err());
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          let idx_state_upd = db.indexer2.update_one(
            doc! { "_id": 0 },
            doc! { "$set": {"network_stats_date": bson::DateTime::from_chrono(date) } }
          ).await;
          if idx_state_upd.is_err() {
            error!("Failed to update indexer state for date {}: {}", date_str, idx_state_upd.unwrap_err());
            sleep(Duration::from_secs(120)).await;
            continue 'mainloop;
          }
          last_date = date.clone();
        } else {
          sleep(Duration::from_secs(300)).await;
        }
      }
      let mut r = running.write().await;
      *r = false;
    });
  }
}
