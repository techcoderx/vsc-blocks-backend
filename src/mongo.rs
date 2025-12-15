use mongodb::{ options::ClientOptions, Client, Collection, IndexModel };
use std::error::Error;
use log::info;
use crate::types::{
  cv::CVContract,
  vsc::{
    BlockHeaderRecord,
    BridgeStats,
    Contract,
    DailyStats,
    ElectionResultRecord,
    HiveBlocksSyncState,
    IndexerState,
    Ledger,
    LedgerActions,
    LedgerBalance,
    TransactionRecord,
    WitnessStat,
    Witnesses,
  },
};

#[derive(Clone)]
pub struct MongoDB {
  // go-vsc
  pub contracts: Collection<Contract>,
  pub elections: Collection<ElectionResultRecord>,
  pub witnesses: Collection<Witnesses>,
  pub blocks: Collection<BlockHeaderRecord>,
  pub l1_blocks: Collection<HiveBlocksSyncState>,
  pub tx_pool: Collection<TransactionRecord>,
  pub ledger_actions: Collection<LedgerActions>,
  pub ledger: Collection<Ledger>,
  pub ledger_bal: Collection<LedgerBalance>,
  pub network_stats: Collection<DailyStats>,

  // be-api additional data
  pub indexer2: Collection<IndexerState>,
  pub witness_stats: Collection<WitnessStat>,
  pub bridge_stats: Collection<BridgeStats>,

  // contract verifier
  pub cv_contracts: Collection<CVContract>,
}

impl MongoDB {
  pub async fn init(url: String) -> Result<MongoDB, Box<dyn Error>> {
    let client_options = ClientOptions::parse(url).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("go-vsc");
    let db2 = client.database("vsc2");
    let db3 = client.database("vsc-cv");
    let cv_contracts: Collection<CVContract> = db3.collection("contracts");
    let is_setup = db3.list_collection_names().await?.contains(&String::from("contracts"));
    if !is_setup {
      MongoDB::setup_cv_db(&cv_contracts).await?;
    }
    info!("Connected to Magi MongoDB database successfully");
    Ok(MongoDB {
      contracts: db.collection("contracts"),
      elections: db.collection("elections"),
      witnesses: db.collection("witnesses"),
      blocks: db.collection("block_headers"),
      l1_blocks: db.collection("hive_blocks"),
      tx_pool: db.collection("transaction_pool"),
      ledger_actions: db.collection("ledger_actions"),
      ledger: db.collection("ledger"),
      ledger_bal: db.collection("ledger_balances"),
      indexer2: db2.collection("indexer_state"),
      witness_stats: db2.collection("witness_stats"),
      bridge_stats: db2.collection("bridge_stats"),
      network_stats: db2.collection("network_stats"),
      cv_contracts: cv_contracts,
    })
  }

  pub async fn setup_cv_db(contracts_db: &Collection<CVContract>) -> Result<(), Box<dyn Error>> {
    // Create indexes for contracts collection
    let status_index = IndexModel::builder()
      .keys(bson::doc! { "status": 1 })
      .build();
    contracts_db.create_index(status_index).await?;

    let status_ts_index = IndexModel::builder()
      .keys(bson::doc! { "status": 1, "request_ts": 1 })
      .build();
    contracts_db.create_index(status_ts_index).await?;

    let contract_id_idx = IndexModel::builder()
      .keys(bson::doc! { "contract_id": 1 })
      .build();
    contracts_db.create_index(contract_id_idx).await?;

    Ok(())
  }
}
