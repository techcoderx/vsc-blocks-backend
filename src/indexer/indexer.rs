use crate::{
  haf::HAFDB,
  indexer::{ blocks::BlockIndexer, bridge::BridgeStatsIndexer, epoch::ElectionIndexer, stats::NetworkStatsIndexer },
  mongo::MongoDB,
};

#[derive(Clone)]
pub struct Indexer {
  block_idxer: BlockIndexer,
  election_idxer: ElectionIndexer,
  bridge_stats_idxer: BridgeStatsIndexer,
  network_stats_idxer: NetworkStatsIndexer,
}

impl Indexer {
  pub fn init(db: &MongoDB, haf: &HAFDB) -> Indexer {
    return Indexer {
      block_idxer: BlockIndexer::init(db, haf),
      election_idxer: ElectionIndexer::init(db, haf),
      bridge_stats_idxer: BridgeStatsIndexer::init(db),
      network_stats_idxer: NetworkStatsIndexer::init(db, haf),
    };
  }

  pub fn start(&self) {
    self.block_idxer.start();
    self.election_idxer.start();
    self.bridge_stats_idxer.start();
    self.network_stats_idxer.start();
  }
}
