use log::info;
use mongodb::bson::doc;
use tokio::time::sleep;
use std::time::Duration;
use crate::{ constants::BRIDGE_TXS_TALLY_INTERVAL, mongo::MongoDB };

#[derive(Clone)]
pub struct BridgeStatsIndexer {
  db: MongoDB,
}

impl BridgeStatsIndexer {
  pub fn init(db: &MongoDB) -> BridgeStatsIndexer {
    return BridgeStatsIndexer { db: db.clone() };
  }

  pub fn start(&self) {
    let ledger = self.db.ledger.clone();
    let ledger_actions = self.db.ledger_actions.clone();
    let bridge_stats = self.db.bridge_stats.clone();

    tokio::spawn(async move {
      info!("Begin indexing bridge stats every {} seconds", BRIDGE_TXS_TALLY_INTERVAL);
      loop {
        let deposits = ledger.count_documents(doc! { "t": "deposit" }).await;
        let withdrawals = ledger_actions.count_documents(doc! { "type": "withdraw" }).await;
        if deposits.is_ok() && withdrawals.is_ok() {
          let _ = bridge_stats
            .update_one(
              doc! { "_id": 0 },
              doc! { "$set": doc! { "deposits": deposits.unwrap() as i64, "withdrawals": withdrawals.unwrap() as i64 } }
            )
            .upsert(true).await;
        }
        sleep(Duration::from_secs(BRIDGE_TXS_TALLY_INTERVAL)).await;
      }
    });
  }
}
