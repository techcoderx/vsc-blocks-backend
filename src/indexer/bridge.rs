use log::info;
use mongodb::{ bson::doc, Collection };
use tokio::time::sleep;
use std::time::Duration;
use crate::{ constants::BRIDGE_TXS_TALLY_INTERVAL, types::vsc::{ BridgeStats, Ledger, LedgerActions } };

#[derive(Clone)]
pub struct BridgeStatsIndexer {
  ledger: Collection<Ledger>,
  ledger_actions: Collection<LedgerActions>,
  bridge_stats: Collection<BridgeStats>,
}

impl BridgeStatsIndexer {
  pub fn init(
    ledger: Collection<Ledger>,
    ledger_actions: Collection<LedgerActions>,
    bridge_stats: Collection<BridgeStats>
  ) -> BridgeStatsIndexer {
    return BridgeStatsIndexer { ledger, ledger_actions, bridge_stats };
  }

  pub fn start(&self) {
    let ledger = self.ledger.clone();
    let ledger_actions = self.ledger_actions.clone();
    let bridge_stats = self.bridge_stats.clone();

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
