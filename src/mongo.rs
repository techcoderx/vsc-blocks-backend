use mongodb::{ options::ClientOptions, Client, Collection };
use std::error::Error;
use log::info;
use crate::types::vsc::{
  BlockHeaderRecord,
  IndexerState,
  Contract,
  ElectionResultRecord,
  HiveBlocksSyncState,
  LedgerActions,
  LedgerBalance,
  RcUsedAtHeight,
  TransactionRecord,
  Witnesses,
};

#[derive(Clone)]
pub struct MongoDB {
  pub contracts: Collection<Contract>,
  pub elections: Collection<ElectionResultRecord>,
  pub witnesses: Collection<Witnesses>,
  pub blocks: Collection<BlockHeaderRecord>,
  pub l1_blocks: Collection<HiveBlocksSyncState>,
  pub tx_pool: Collection<TransactionRecord>,
  pub balances: Collection<LedgerBalance>,
  pub ledger_actions: Collection<LedgerActions>,
  pub rc: Collection<RcUsedAtHeight>,
  pub indexer2: Collection<IndexerState>,
}

impl MongoDB {
  pub async fn init(url: String) -> Result<MongoDB, Box<dyn Error>> {
    let client_options = ClientOptions::parse(url).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("go-vsc");
    let db2 = client.database("vsc2");
    info!("Connected to VSC MongoDB database successfully");
    Ok(MongoDB {
      contracts: db.collection("contracts"),
      elections: db.collection("elections"),
      witnesses: db.collection("witnesses"),
      blocks: db.collection("block_headers"),
      l1_blocks: db.collection("hive_blocks"),
      tx_pool: db.collection("transaction_pool"),
      balances: db.collection("ledger_balances"),
      ledger_actions: db.collection("ledger_actions"),
      rc: db.collection("rcs"),
      indexer2: db2.collection("indexer_state"),
    })
  }
}
