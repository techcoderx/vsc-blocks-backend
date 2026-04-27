use lazy_static::lazy_static;
use std::collections::HashMap;

#[derive(Clone)]
pub struct StaticMeta {
  pub title: &'static str,
  pub description: &'static str,
  pub og_type: Option<&'static str>,
  pub noindex: bool,
}

const GENERIC_DESC: &str = "Block explorer for the Magi network — a Hive L2 smart contract platform.";

lazy_static! {
  pub static ref STATIC_ROUTES: HashMap<&'static str, StaticMeta> = {
    let mut m = HashMap::new();
    m.insert("/", StaticMeta {
      title: "Magi Blocks Explorer",
      description: GENERIC_DESC,
      og_type: None,
      noindex: false,
    });
    m.insert("/blocks", StaticMeta {
      title: "Blocks",
      description: "Latest Magi L2 blocks — height, proposer, tx count, and timestamp.",
      og_type: None,
      noindex: false,
    });
    m.insert("/transactions", StaticMeta {
      title: "Transactions",
      description: "Latest transactions on the Magi network — Magi L2 and Hive L1 operations.",
      og_type: None,
      noindex: false,
    });
    m.insert("/transactions/magi", StaticMeta {
      title: "Magi Transactions",
      description: "Latest Magi L2 transactions.",
      og_type: None,
      noindex: false,
    });
    m.insert("/transactions/hive", StaticMeta {
      title: "Hive Transactions",
      description: "Latest Hive L1 custom_json operations routed to Magi.",
      og_type: None,
      noindex: false,
    });
    m.insert("/contracts", StaticMeta {
      title: "Contracts",
      description: "Smart contracts deployed on the Magi network.",
      og_type: None,
      noindex: false,
    });
    m.insert("/tokens", StaticMeta {
      title: "Tokens",
      description: "Fungible tokens on Magi — supply, holders, and transfers.",
      og_type: None,
      noindex: false,
    });
    m.insert("/nfts", StaticMeta {
      title: "NFT Collections",
      description: "NFT collections on Magi — tokens, owners, and transfers.",
      og_type: None,
      noindex: false,
    });
    m.insert("/witnesses", StaticMeta {
      title: "Witnesses",
      description: "Magi network witnesses — uptime, stake weight, and participation.",
      og_type: None,
      noindex: false,
    });
    m.insert("/schedule", StaticMeta {
      title: "Witness Schedule",
      description: "Current Magi witness block-production schedule.",
      og_type: None,
      noindex: false,
    });
    m.insert("/elections", StaticMeta {
      title: "Elections",
      description: "Magi witness election history.",
      og_type: None,
      noindex: false,
    });
    m.insert("/nam/btc", StaticMeta {
      title: "BTC Mapping",
      description: "Native bitcoin mappings on the Magi network.",
      og_type: None,
      noindex: false,
    });
    m.insert("/nam/hive", StaticMeta {
      title: "Hive Bridge",
      description: "Deposits and withdrawals between Hive L1 and Magi L2.",
      og_type: None,
      noindex: false,
    });
    m.insert("/nam/hive/maps", StaticMeta {
      title: "Hive Bridge Deposits",
      description: "Latest deposits from Hive L1 to Magi L2.",
      og_type: None,
      noindex: false,
    });
    m.insert("/nam/hive/unmaps", StaticMeta {
      title: "Hive Bridge Withdrawals",
      description: "Latest withdrawals from Magi L2 to Hive L1.",
      og_type: None,
      noindex: false,
    });
    m.insert("/staking/hbd", StaticMeta {
      title: "HBD Staking",
      description: "Stake HBD on Magi to earn interest rewards.",
      og_type: None,
      noindex: false,
    });
    m.insert("/staking/hbd/claims", StaticMeta {
      title: "HBD Staking Claims",
      description: "Latest HBD staking interest claims on Magi.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts", StaticMeta {
      title: "Charts",
      description: "Network charts for the Magi L2 explorer.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts/blocks", StaticMeta {
      title: "Block Charts",
      description: "Block-production statistics over time.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts/txs", StaticMeta {
      title: "Transaction Charts",
      description: "Transaction volume and breakdowns over time.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts/addresses", StaticMeta {
      title: "Address Charts",
      description: "Active-address statistics over time.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts/contracts", StaticMeta {
      title: "Contract Charts",
      description: "Contract-deployment and call statistics.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts/bridge", StaticMeta {
      title: "Bridge Charts",
      description: "Hive ↔ Magi bridge volume over time.",
      og_type: None,
      noindex: false,
    });
    m.insert("/charts/witnesses", StaticMeta {
      title: "Witness Charts",
      description: "Witness-participation statistics over time.",
      og_type: None,
      noindex: false,
    });
    m.insert("/tools/verify/contract", StaticMeta {
      title: "Verify Contract",
      description: "Submit a Magi contract for source-code verification.",
      og_type: None,
      noindex: true,
    });
    m.insert("/tools/dag", StaticMeta {
      title: "DAG Inspector",
      description: "Inspect raw DAG entries stored by the Magi network.",
      og_type: None,
      noindex: true,
    });
    m.insert("/tools/broadcast", StaticMeta {
      title: "Broadcast Operation",
      description: "Broadcast a signed Hive operation targeting the Magi network.",
      og_type: None,
      noindex: true,
    });
    m.insert("/settings", StaticMeta {
      title: "Settings",
      description: "User preferences for Magi Blocks Explorer.",
      og_type: None,
      noindex: true,
    });
    m
  };
}

pub const PAGINATED_PREFIXES: &[(&str, &str)] = &[
  ("/blocks/", "/blocks"),
  ("/transactions/magi/", "/transactions/magi"),
  ("/elections/", "/elections"),
  ("/nam/hive/maps/", "/nam/hive/maps"),
  ("/nam/hive/unmaps/", "/nam/hive/unmaps"),
  ("/staking/hbd/claims/", "/staking/hbd/claims"),
];
