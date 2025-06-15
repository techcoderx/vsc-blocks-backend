use crate::{
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
  pub fn init(http_client: &reqwest::Client, db: &MongoDB) -> Indexer {
    return Indexer {
      block_idxer: BlockIndexer::init(http_client, db),
      election_idxer: ElectionIndexer::init(http_client, db),
      bridge_stats_idxer: BridgeStatsIndexer::init(db),
      network_stats_idxer: NetworkStatsIndexer::init(http_client, db),
    };
  }

  pub fn start(&self) {
    self.block_idxer.start();
    self.election_idxer.start();
    self.bridge_stats_idxer.start();
    self.network_stats_idxer.start();
  }
}
