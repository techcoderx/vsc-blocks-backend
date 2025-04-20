use mongodb::Collection;
use crate::{
  indexer::{ blocks::BlockIndexer, bridge::BridgeStatsIndexer, epoch::ElectionIndexer },
  types::vsc::{ BlockHeaderRecord, BridgeStats, ElectionResultRecord, IndexerState, Ledger, LedgerActions, WitnessStat },
};

#[derive(Clone)]
pub struct Indexer {
  block_idxer: BlockIndexer,
  election_idxer: ElectionIndexer,
  bridge_stats_idxer: BridgeStatsIndexer,
}

impl Indexer {
  pub fn init(
    http_client: reqwest::Client,
    blocks_db: Collection<BlockHeaderRecord>,
    elections_db: Collection<ElectionResultRecord>,
    ledger_db: Collection<Ledger>,
    ledger_actions_db: Collection<LedgerActions>,
    indexer2: Collection<IndexerState>,
    witness_stats: Collection<WitnessStat>,
    bridge_stats: Collection<BridgeStats>
  ) -> Indexer {
    return Indexer {
      block_idxer: BlockIndexer::init(
        http_client.clone(),
        blocks_db.clone(),
        elections_db.clone(),
        indexer2.clone(),
        witness_stats.clone()
      ),
      election_idxer: ElectionIndexer::init(http_client.clone(), elections_db.clone(), indexer2.clone(), witness_stats.clone()),
      bridge_stats_idxer: BridgeStatsIndexer::init(ledger_db.clone(), ledger_actions_db.clone(), bridge_stats.clone()),
    };
  }

  pub fn start(&self) {
    self.block_idxer.start();
    self.election_idxer.start();
    self.bridge_stats_idxer.start();
  }
}
