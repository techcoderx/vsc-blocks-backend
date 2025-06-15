use bson::doc;
use mongodb::{ options::{ ClientOptions, IndexOptions }, Client, Collection, IndexModel };
use std::error::Error;
use log::info;
use crate::types::{
  cv::{ CVContract, CVContractCode, CVIdName },
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
  pub cv_source_codes: Collection<CVContractCode>,
  pub cv_licenses: Collection<CVIdName>,
}

impl MongoDB {
  pub async fn init(url: String) -> Result<MongoDB, Box<dyn Error>> {
    let client_options = ClientOptions::parse(url).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("go-vsc");
    let db2 = client.database("vsc2");
    let db3 = client.database("vsc-cv");
    info!("Connected to VSC MongoDB database successfully");
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
      cv_contracts: db3.collection("contracts"),
      cv_source_codes: db3.collection("source_code"),
      cv_licenses: db3.collection("licenses"),
    })
  }

  pub async fn setup(&self) -> Result<(), Box<dyn Error>> {
    let already_setup = self.cv_licenses.find_one(doc! {}).await?;
    if already_setup.is_some() {
      return Ok(());
    }
    let licenses = vec![
      "MIT",
      "Apache-2.0",
      "GPL-3.0-only",
      "GPL-3.0-or-later",
      "LGPL-3.0-only",
      "LGPL-3.0-or-later",
      "AGPL-3.0-only",
      "AGPL-3.0-or-later",
      "MPL 2.0",
      "BSL-1.0",
      "WTFPL",
      "Unlicense"
    ];
    let licenses: Vec<CVIdName> = licenses
      .into_iter()
      .enumerate()
      .map(|(i, name)| CVIdName {
        id: i as i32,
        name: name.to_string(),
      })
      .collect();
    self.cv_licenses.insert_many(licenses).await?;

    // Create compound unique index for source_code collection
    let source_codes_index = IndexModel::builder()
      .keys(bson::doc! { "addr": 1, "fname": 1 })
      .options(IndexOptions::builder().unique(true).build())
      .build();
    self.cv_source_codes.create_index(source_codes_index).await?;

    // Create indexes for contracts collection
    let status_index = IndexModel::builder()
      .keys(bson::doc! { "status": 1 })
      .build();
    self.cv_contracts.create_index(status_index).await?;

    let status_ts_index = IndexModel::builder()
      .keys(bson::doc! { "status": 1, "request_ts": 1 })
      .build();
    self.cv_contracts.create_index(status_ts_index).await?;

    Ok(())
  }
}
