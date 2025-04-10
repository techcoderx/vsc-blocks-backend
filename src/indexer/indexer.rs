use mongodb::Collection;
use crate::{
  types::vsc::{ BlockHeaderRecord, ElectionResultRecord, IndexerState },
  indexer::{ blocks::BlockIndexer, epoch::ElectionIndexer },
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
    indexer2: Collection<IndexerState>
  ) -> Indexer {
    return Indexer {
      block_idxer: BlockIndexer::init(http_client.clone(), blocks_db.clone(), elections_db.clone(), indexer2.clone()),
      election_idxer: ElectionIndexer::init(http_client.clone(), elections_db.clone(), indexer2.clone()),
    };
  }

  pub fn start(&self) {
    self.block_idxer.start();
    self.election_idxer.start();
  }
}
