use formatter::thousand_separator;

use crate::mongo::MongoDB;

use super::{
  fetchers::{ self, BlockLookup, OgState },
  helpers::{ abbreviate_hash, is_cid, is_hex_string, is_positive_int, validate_hive_username },
  static_routes::{ StaticMeta, PAGINATED_PREFIXES, STATIC_ROUTES },
  template::MetaTags,
};

const SITE_SUFFIX: &str = " | Magi Blocks";
const DEFAULT_TITLE: &str = "Magi Blocks Explorer";
const DEFAULT_DESC: &str = "Block explorer for the Magi network — a Hive L2 smart contract platform.";
const DEFAULT_OG_TYPE: &str = "website";

struct PartialMeta {
  title: Option<String>,
  description: Option<String>,
  og_type: Option<String>,
  noindex: bool,
}

impl PartialMeta {
  fn new() -> Self {
    Self { title: None, description: None, og_type: None, noindex: false }
  }
}

fn with_suffix(title: &str) -> String {
  if title.ends_with(SITE_SUFFIX) { title.to_string() } else { format!("{}{}", title, SITE_SUFFIX) }
}

fn finalize(partial: PartialMeta, canonical: String, og_image: &str) -> MetaTags {
  MetaTags {
    title: with_suffix(&partial.title.unwrap_or_else(|| DEFAULT_TITLE.to_string())),
    description: partial.description.unwrap_or_else(|| DEFAULT_DESC.to_string()),
    canonical,
    og_type: partial.og_type.unwrap_or_else(|| DEFAULT_OG_TYPE.to_string()),
    image: og_image.to_string(),
    noindex: partial.noindex,
  }
}

fn static_to_partial(s: &StaticMeta) -> PartialMeta {
  PartialMeta {
    title: Some(s.title.to_string()),
    description: Some(s.description.to_string()),
    og_type: s.og_type.map(|v| v.to_string()),
    noindex: s.noindex,
  }
}

fn address_tab_description(addr: &str, sub: Option<&str>) -> String {
  let target = if let Some(name) = addr.strip_prefix("hive:") {
    format!("@{}", name)
  } else {
    addr.to_string()
  };
  match sub {
    Some("hiveops") => format!("Hive L1 operations history for {} on Magi.", target),
    Some("ledger") => format!(
      "Ledger history for {} on Magi — deposits, withdrawals, transfers, interest, and fees.",
      target
    ),
    Some("actions") => format!("Ledger actions initiated by {} on Magi.", target),
    Some("deposits") => format!("Bridge deposits for {} on Magi.", target),
    Some("withdrawals") => format!("Bridge withdrawals for {} on Magi.", target),
    Some("witness") =>
      format!("Witness profile for {} on Magi — stake, uptime, and signing keys.", target),
    Some("balances") => format!("Token and native balances for {} on Magi.", target),
    Some("nfts") => format!("NFT holdings for {} on Magi.", target),
    _ => {
      if addr.starts_with("hive:") {
        format!(
          "Hive account {} on Magi — balances, transactions, ledger ops, bridge deposits/withdrawals, and witness info.",
          target
        )
      } else {
        format!(
          "Address {} on Magi — balances, transactions, ledger ops, and NFT holdings.",
          addr
        )
      }
    }
  }
}

async fn resolve_tx(state: &OgState, txid: &str) -> PartialMeta {
  let mut p = PartialMeta::new();
  p.og_type = Some("article".to_string());
  if is_hex_string(txid, 40) {
    let data = fetchers::fetch_l1_tx(state, txid).await;
    let op_type = data
      .as_ref()
      .and_then(|d| d.transaction_json.as_ref())
      .and_then(|j| j.operations.as_ref())
      .and_then(|ops| ops.first())
      .map(|o| o.0.clone());
    let block = data.as_ref().and_then(|d| d.block_num);
    p.title = Some(format!("L1 Tx {}", abbreviate_hash(txid, 8, 6)));
    p.description = Some(match (block, op_type) {
      (Some(b), Some(t)) =>
        format!(
          "Hive L1 transaction {} — {} operation in block #{} on Magi.",
          txid,
          t,
          thousand_separator(b)
        ),
      (Some(b), None) =>
        format!("Hive L1 transaction {} in block #{} on Magi.", txid, thousand_separator(b)),
      _ => format!("Hive L1 transaction {} on Magi.", txid),
    });
    return p;
  }
  let data = fetchers::fetch_l2_tx(state, txid).await;
  p.title = Some(format!("Tx {}", abbreviate_hash(txid, 12, 6)));
  p.description = Some(match data {
    Some(t) => {
      let mut parts: Vec<String> = vec![format!("Magi L2 transaction {}", txid)];
      if let Some(status) = t.status {
        parts.push(format!("status {}", status));
      }
      if let Some(h) = t.anchr_height {
        parts.push(format!("anchored at block #{}", thousand_separator(h)));
      }
      if let Some(ops) = t.ops {
        let mut names: Vec<String> = ops
          .into_iter()
          .filter_map(|o| o.op_type)
          .collect();
        names.sort();
        names.dedup();
        if !names.is_empty() {
          parts.push(format!("ops: {}", names.join(", ")));
        }
      }
      format!("{}.", parts.join(" — "))
    }
    None => format!("Magi L2 transaction {}.", txid),
  });
  p
}

async fn resolve_address(state: &OgState, addr: &str, sub: Option<&str>) -> PartialMeta {
  let mut p = PartialMeta::new();
  let is_l1 = addr
    .strip_prefix("hive:")
    .map(validate_hive_username)
    .unwrap_or(false);
  let is_l2 = addr.starts_with("did:") || addr.starts_with("system:");
  if !is_l1 && !is_l2 {
    p.title = Some("Not Found".to_string());
    p.description = Some(format!("No address matches {} on Magi.", addr));
    p.noindex = true;
    return p;
  }
  if is_l1 {
    let username = &addr[5..];
    let acc = fetchers::fetch_l1_account(state, username).await;
    let has_data = acc.as_ref().and_then(|a| a.username.as_ref()).is_some();
    p.title = Some(format!("@{}", username));
    p.description = Some(if has_data {
      address_tab_description(addr, sub)
    } else {
      format!("Hive account @{} on Magi.", username)
    });
    p.og_type = Some("profile".to_string());
    return p;
  }
  p.title = Some(abbreviate_hash(addr, 10, 6));
  p.description = Some(address_tab_description(addr, sub));
  p.og_type = Some("profile".to_string());
  p
}

async fn resolve_block(db: &MongoDB, block_id: &str) -> PartialMeta {
  let mut p = PartialMeta::new();
  let lookup = if is_positive_int(block_id) {
    match block_id.parse::<i32>() {
      Ok(n) => Some(BlockLookup::Id(n)),
      Err(_) => None,
    }
  } else if is_hex_string(block_id, 40) {
    Some(BlockLookup::Id1(block_id.to_string()))
  } else if is_cid(block_id) {
    Some(BlockLookup::Cid(block_id.to_string()))
  } else {
    None
  };
  let lookup = match lookup {
    Some(l) => l,
    None => {
      p.title = Some("Block Not Found".to_string());
      p.description = Some(format!("No block matches {} on Magi.", block_id));
      p.noindex = true;
      return p;
    }
  };
  let is_id_lookup = matches!(lookup, BlockLookup::Id(_));
  let numeric_from_path = if is_id_lookup { block_id.parse::<u32>().ok() } else { None };
  let data = fetchers::fetch_block(db, lookup).await;
  let height_num = data.as_ref().and_then(|d| d.block_id).or(numeric_from_path);
  let height_str = match height_num {
    Some(n) => thousand_separator(n),
    None => abbreviate_hash(block_id, 10, 6),
  };
  p.og_type = Some("article".to_string());
  p.title = Some(format!("Block #{}", height_str));
  p.description = Some(match data {
    Some(d) => {
      let proposer_part = d.proposer.map(|v| format!(" proposed by {}", v)).unwrap_or_default();
      let ts_part = d.ts.map(|v| format!(" at {}", v)).unwrap_or_default();
      format!(
        "Magi L2 block #{}{}{}. View transactions, op logs, contract outputs, and participation.",
        height_str,
        proposer_part,
        ts_part
      )
    }
    None => format!("Magi L2 block #{} details on Magi Blocks Explorer.", height_str),
  });
  p
}

async fn resolve_contract(db: &MongoDB, contract_id: &str) -> PartialMeta {
  let mut p = PartialMeta::new();
  let (info, cv) = tokio::join!(
    fetchers::fetch_contract(db, contract_id),
    fetchers::fetch_cv_info(db, contract_id)
  );
  p.title = Some(format!("Contract {}", abbreviate_hash(contract_id, 10, 6)));
  let mut parts: Vec<String> = vec![format!("Magi smart contract {}", contract_id)];
  if let Some(c) = info.as_ref() {
    if let Some(creator) = &c.creator {
      parts.push(format!("deployed by {}", creator));
    }
    if let Some(h) = c.creation_height {
      parts.push(format!("at block #{}", thousand_separator(h)));
    }
  }
  if let Some(c) = cv {
    if matches!(c.status.as_deref(), Some("match") | Some("full")) {
      parts.push("source code verified".to_string());
    }
  }
  p.og_type = Some("article".to_string());
  p.description = Some(format!("{}.", parts.join(" — ")));
  p
}

async fn resolve_token(state: &OgState, contract_id: &str, sub: Option<&str>) -> PartialMeta {
  let mut p = PartialMeta::new();
  let data = fetchers::fetch_token(state, contract_id).await;
  match data {
    None => {
      p.title = Some(format!("Token {}", abbreviate_hash(contract_id, 10, 6)));
      p.description = Some(format!("Fungible token {} on Magi.", contract_id));
    }
    Some(d) => {
      let label = match (&d.symbol, &d.name) {
        (Some(sym), Some(name)) => format!("{} ({})", name, sym),
        (Some(sym), None) => sym.clone(),
        (None, Some(name)) => name.clone(),
        (None, None) => contract_id.to_string(),
      };
      let sub_desc = match sub {
        Some("holders") => format!(" Top holders of {}.", label),
        Some("info") => format!(" Contract metadata for {}.", label),
        _ => format!(" Transfers, holders, and supply for {}.", label),
      };
      p.title = Some(format!("{} Token", label));
      p.description = Some(format!("{} — a fungible token on Magi.{}", label, sub_desc));
    }
  }
  p
}

async fn resolve_nft(state: &OgState, contract_id: &str, sub: Option<&str>) -> PartialMeta {
  let mut p = PartialMeta::new();
  let data = fetchers::fetch_nft(state, contract_id).await;
  match data {
    None => {
      p.title = Some(format!("NFT {}", abbreviate_hash(contract_id, 10, 6)));
      p.description = Some(format!("NFT collection {} on Magi.", contract_id));
    }
    Some(d) => {
      let label = match (&d.symbol, &d.name) {
        (Some(sym), Some(name)) => format!("{} ({})", name, sym),
        (Some(sym), None) => sym.clone(),
        (None, Some(name)) => name.clone(),
        (None, None) => contract_id.to_string(),
      };
      let sub_desc = match sub {
        Some("transfers") => format!(" Recent transfers in {}.", label),
        Some("info") => format!(" Collection metadata for {}.", label),
        _ => format!(" Tokens, owners, and transfers for {}.", label),
      };
      p.title = Some(format!("{} NFT Collection", label));
      p.description = Some(format!("{} — an NFT collection on Magi.{}", label, sub_desc));
    }
  }
  p
}

async fn resolve_epoch(db: &MongoDB, num: i64) -> PartialMeta {
  let mut p = PartialMeta::new();
  p.og_type = Some("article".to_string());
  p.title = Some(format!("Epoch #{}", num));
  let data = fetchers::fetch_epoch(db, num).await;
  p.description = Some(match data {
    Some(d) => {
      let proposer_part = d.proposer.map(|v| format!(" elected by {}", v)).unwrap_or_default();
      let h_part = d.block_height
        .map(|h| format!(" at block #{}", thousand_separator(h)))
        .unwrap_or_default();
      format!("Magi witness epoch #{}{}{}.", num, proposer_part, h_part)
    }
    None => format!("Magi witness epoch #{} details.", num),
  });
  p
}

async fn resolve_staking_claim(state: &OgState, block_height: i64) -> PartialMeta {
  let mut p = PartialMeta::new();
  p.og_type = Some("article".to_string());
  p.title = Some(format!("HBD Staking Claim #{}", thousand_separator(block_height)));
  let claim = fetchers::fetch_staking_claim(state, block_height).await;
  p.description = Some(match claim {
    Some(c) => {
      let amount_part = c.amount.map(|v| format!(" — amount {}", v)).unwrap_or_default();
      let recv_part = c.received_n.map(|n| format!(", {} recipients", n)).unwrap_or_default();
      format!(
        "HBD staking interest claim at block #{}{}{}.",
        thousand_separator(block_height),
        amount_part,
        recv_part
      )
    }
    None =>
      format!(
        "HBD staking interest claim at block #{} on Magi.",
        thousand_separator(block_height)
      ),
  });
  p
}

fn normalize_pathname(input: &str) -> String {
  let path = input.split('?').next().unwrap_or("/");
  let path = if path.is_empty() { "/" } else { path };
  if path.len() > 1 && path.ends_with('/') { path[..path.len() - 1].to_string() } else { path.to_string() }
}

fn match_paginated_static(path: &str) -> Option<PartialMeta> {
  for (prefix, meta_key) in PAGINATED_PREFIXES {
    if let Some(page) = path.strip_prefix(prefix) {
      if is_positive_int(page) {
        let base = STATIC_ROUTES.get(*meta_key)?;
        let mut p = static_to_partial(base);
        let page_num: u64 = page.parse().unwrap_or(1);
        if page_num > 1 {
          p.title = Some(format!("{} — Page {}", base.title, page));
        }
        return Some(p);
      }
    }
  }
  None
}

pub async fn resolve_meta(
  pathname: &str,
  origin: &str,
  og_image: &str,
  db: &MongoDB,
  state: &OgState
) -> MetaTags {
  let path = normalize_pathname(pathname);
  let canonical = format!("{}{}", origin, if path == "/" { "/" } else { path.as_str() });

  if let Some(s) = STATIC_ROUTES.get(path.as_str()) {
    return finalize(static_to_partial(s), canonical, og_image);
  }

  if let Some(p) = match_paginated_static(&path) {
    return finalize(p, canonical, og_image);
  }

  let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

  if segments.len() >= 2 {
    match segments[0] {
      "tx" => {
        return finalize(resolve_tx(state, segments[1]).await, canonical, og_image);
      }
      "address" => {
        return finalize(
          resolve_address(state, segments[1], segments.get(2).copied()).await,
          canonical,
          og_image
        );
      }
      "block" => {
        return finalize(resolve_block(db, segments[1]).await, canonical, og_image);
      }
      "contract" => {
        return finalize(resolve_contract(db, segments[1]).await, canonical, og_image);
      }
      "token" => {
        return finalize(
          resolve_token(state, segments[1], segments.get(2).copied()).await,
          canonical,
          og_image
        );
      }
      "nft" => {
        return finalize(
          resolve_nft(state, segments[1], segments.get(2).copied()).await,
          canonical,
          og_image
        );
      }
      "epoch" if is_positive_int(segments[1]) => {
        let num: i64 = segments[1].parse().unwrap_or(0);
        return finalize(resolve_epoch(db, num).await, canonical, og_image);
      }
      _ => {}
    }
  }

  if
    segments.len() >= 4 &&
    segments[0] == "staking" &&
    segments[1] == "hbd" &&
    segments[2] == "claim" &&
    is_positive_int(segments[3])
  {
    let h: i64 = segments[3].parse().unwrap_or(0);
    return finalize(resolve_staking_claim(state, h).await, canonical, og_image);
  }

  if !segments.is_empty() && segments[0].starts_with('@') {
    let username = &segments[0][1..];
    if validate_hive_username(username) {
      let addr = format!("hive:{}", username);
      return finalize(
        resolve_address(state, &addr, segments.get(1).copied()).await,
        canonical,
        og_image
      );
    }
  }

  finalize(
    PartialMeta {
      title: Some("Not Found".to_string()),
      description: Some(format!("Path {} was not found on Magi Blocks Explorer.", path)),
      og_type: None,
      noindex: true,
    },
    canonical,
    og_image
  )
}
