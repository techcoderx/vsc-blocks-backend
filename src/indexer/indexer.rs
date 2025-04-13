use mongodb::Collection;
use crate::{
  indexer::{ blocks::BlockIndexer, epoch::ElectionIndexer },
  types::vsc::{ BlockHeaderRecord, ElectionResultRecord, IndexerState, WitnessStat },
};

#[derive(Clone)]
pub struct Indexer {
  block_idxer: BlockIndexer,
  election_idxer: ElectionIndexer,
}

impl Indexer {
  pub fn init(
    http_client: reqwest::Client,
    blocks_db: Collection<BlockHeaderRecord>,
    elections_db: Collection<ElectionResultRecord>,
    indexer2: Collection<IndexerState>,
    witness_stats: Collection<WitnessStat>
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
    };
  }

  pub fn start(&self) {
    self.block_idxer.start();
    self.election_idxer.start();
  }
}
