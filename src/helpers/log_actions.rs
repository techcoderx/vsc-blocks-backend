use std::collections::{ HashMap, HashSet };

use serde::Deserialize;
use serde_json::{ json, Value };

use formatter::thousand_separator;

const STATIC_CONTRACT_TYPES: &[(&str, &str)] = &[
  ("vsc1Brvi4YZHLkocYNAFd7Gf1JpsPjzNnv4i45", "dex_router"),
  ("vsc1BdrQ6EtbQ64rq2PkPd21x4MaLnVRcJj85d", "btc_mapping"),
  ("vsc1BVLuXCWC1UShtDBenWJ2B6NWpnyV2T637n", "oki_inarow"),
  ("vsc1BgfucQVHwYBHuK2yMEv4AhYua9rtQ45Uoe", "oki_escrow"),
  ("vsc1BiM4NC1yeGPCjmq8FC3utX8dByizjcCBk7", "oki_lottery"),
  ("vsc1Ba9AyyUcMnYVoDVsjoJztnPFHNxQwWBPsb", "oki_dao"),
];

const COLON_DELIM_TYPES: &[&str] = &["oki_lottery", "oki_dao"];

pub fn static_contract_type(contract_id: &str) -> Option<&'static str> {
  STATIC_CONTRACT_TYPES.iter().find(|(id, _)| *id == contract_id).map(|(_, t)| *t)
}

#[derive(Debug, Default, Clone)]
pub struct LogActionMetadata {
  pub contract_types: HashMap<String, String>,
  pub token_info: HashMap<String, TokenMeta>,
  pub nft_info: HashMap<String, NftMeta>,
  pub pool_info: HashMap<String, PoolMeta>,
}

#[derive(Debug, Clone)]
pub struct TokenMeta {
  pub symbol: String,
  pub decimals: u32,
}

#[derive(Debug, Clone)]
pub struct NftMeta {
  pub name: String,
  #[allow(dead_code)]
  pub symbol: String,
}

#[derive(Debug, Clone)]
pub struct PoolMeta {
  pub asset0: String,
  pub asset1: String,
}

pub fn resolve_contract_type(contract_id: &str, meta: &LogActionMetadata) -> Option<String> {
  if let Some(t) = static_contract_type(contract_id) {
    return Some(t.to_string());
  }
  let raw = meta.contract_types.get(contract_id)?.as_str();
  let mapped = match raw {
    "init_magi_token" => "magi_token",
    "init_magi_nft" => "magi_nft",
    "pool_init" => "dex_pool",
    other => other,
  };
  Some(mapped.to_string())
}

#[derive(Debug, Default, Clone)]
pub struct ParsedLog {
  pub event_type: String,
  pub fields: HashMap<String, String>,
}

pub fn parse_log(contract_type: Option<&str>, log_str: &str) -> ParsedLog {
  let trimmed = log_str.trim();

  if trimmed.starts_with('{') {
    if let Ok(obj) = serde_json::from_str::<Value>(trimmed) {
      let event_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      let mut fields = HashMap::new();
      if let Some(attrs) = obj.get("attributes").and_then(|v| v.as_object()) {
        for (k, v) in attrs.iter() {
          let s = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
          };
          fields.insert(k.clone(), s);
        }
      }
      return ParsedLog { event_type, fields };
    }
    let mut fields = HashMap::new();
    fields.insert("raw".to_string(), log_str.to_string());
    return ParsedLog { event_type: String::new(), fields };
  }

  let parts: Vec<&str> = trimmed.split('|').collect();
  let event_type = parts.first().copied().unwrap_or("").to_string();

  let key_sep = if let Some(ct) = contract_type {
    if COLON_DELIM_TYPES.contains(&ct) { ':' } else { '=' }
  } else {
    let sample = parts.get(1).copied().unwrap_or("");
    if sample.contains(':') && !sample.contains('=') { ':' } else { '=' }
  };

  let mut fields = HashMap::new();
  for part in parts.iter().skip(1) {
    if let Some(idx) = part.find(key_sep) {
      if idx > 0 {
        fields.insert(part[..idx].to_string(), part[idx + 1..].to_string());
      }
    }
  }

  ParsedLog { event_type, fields }
}

fn fetch_field<'a>(f: &'a HashMap<String, String>, key: &str) -> &'a str {
  f.get(key).map(|s| s.as_str()).unwrap_or("")
}

fn format_token_amount(raw: &str, decimals: u32) -> String {
  let n: f64 = raw.parse().unwrap_or(0.0);
  let scaled = n / (10f64).powi(decimals as i32);
  thousand_separator(format!("{:.*}", decimals as usize, scaled))
}

fn fmt_sats(raw: &str) -> String {
  let n: f64 = raw.parse().unwrap_or(0.0);
  thousand_separator(format!("{:.8}", n / 100_000_000.0)) + " BTC"
}

fn fmt_dex_amount(amount: &str, asset: &str) -> String {
  let upper = asset.to_uppercase();
  if upper == "HIVE" || upper == "HBD" {
    if let Ok(n) = amount.parse::<u64>() {
      let val = (n as f64) / 1000.0;
      return format!("{} {}", thousand_separator(format!("{:.3}", val)), upper);
    }
  }
  if upper == "BTC" || upper == "SATS" {
    return fmt_sats(amount);
  }
  format!("{} {}", thousand_separator(amount), upper)
}

fn arrow() -> &'static str {
  "→"
}

fn describe_token(
  contract_id: &str,
  event_type: &str,
  f: &HashMap<String, String>,
  meta: &LogActionMetadata
) -> Option<String> {
  let info = meta.token_info.get(contract_id);
  let sym = info.map(|i| i.symbol.as_str()).unwrap_or("???");
  let dec = info.map(|i| i.decimals).unwrap_or(0);
  let fmt_amt = |raw: &str| if info.is_some() { format_token_amount(raw, dec) } else { raw.to_string() };

  match event_type {
    "transfer" => {
      let from = fetch_field(f, "from");
      let to = fetch_field(f, "to");
      let amt = fmt_amt(fetch_field(f, "amount").trim_start_matches('-'));
      let amt = if fetch_field(f, "amount").is_empty() { fmt_amt("0") } else { amt };
      if from.is_empty() || from == "null" {
        Some(format!("Mint {} {} to {}", amt, sym, to))
      } else if to.is_empty() || to == "null" {
        Some(format!("Burn {} {} from {}", amt, sym, from))
      } else {
        Some(format!("Transfer {} {} {} {} {}", amt, sym, from, arrow(), to))
      }
    }
    "approval" => {
      let owner = fetch_field(f, "owner");
      let spender = fetch_field(f, "spender");
      let amt = fmt_amt(if fetch_field(f, "amount").is_empty() { "0" } else { fetch_field(f, "amount") });
      Some(format!("{} approved {} to spend {} {}", owner, spender, amt, sym))
    }
    "ownerChange" => {
      let prev = fetch_field(f, "previousOwner");
      let new = fetch_field(f, "newOwner");
      Some(format!("Ownership transferred {} {} {}", prev, arrow(), new))
    }
    "paused" => Some(format!("Token paused by {}", fetch_field(f, "by"))),
    "unpaused" => Some(format!("Token unpaused by {}", fetch_field(f, "by"))),
    "init_magi_token" =>
      Some(format!("Token initialized: {} ({})", fetch_field(f, "name"), fetch_field(f, "symbol"))),
    _ => None,
  }
}

fn describe_nft(
  contract_id: &str,
  event_type: &str,
  f: &HashMap<String, String>,
  meta: &LogActionMetadata
) -> Option<String> {
  let info = meta.nft_info.get(contract_id);
  let label = info.map(|i| format!("{} ", i.name)).unwrap_or_default();

  match event_type {
    "TransferSingle" => {
      let from = fetch_field(f, "from");
      let to = fetch_field(f, "to");
      let id = fetch_field(f, "id");
      let value = fetch_field(f, "value");
      let qty = if !value.is_empty() && value != "1" { format!(" (x{})", value) } else { String::new() };
      if from.is_empty() || from == "null" {
        Some(format!("Mint {}NFT #{}{} to {}", label, id, qty, to))
      } else if to.is_empty() || to == "null" {
        Some(format!("Burn {}NFT #{}{} from {}", label, id, qty, from))
      } else {
        Some(format!("Transfer {}NFT #{}{} {} {} {}", label, id, qty, from, arrow(), to))
      }
    }
    "TransferBatch" => {
      let from = fetch_field(f, "from");
      let to = fetch_field(f, "to");
      if from.is_empty() || from == "null" {
        Some(format!("Batch mint {}NFTs to {}", label, to))
      } else {
        Some(format!("Batch transfer {}NFTs {} {} {}", label, from, arrow(), to))
      }
    }
    "ApprovalForAll" =>
      Some(format!("{} set approval for all to {}", fetch_field(f, "account"), fetch_field(f, "operator"))),
    "tokenCreated" => {
      let token_id = if fetch_field(f, "tokenId").is_empty() { fetch_field(f, "id") } else { fetch_field(f, "tokenId") };
      let soulbound = if fetch_field(f, "soulbound") == "true" { " (soulbound)" } else { "" };
      Some(format!("Created {}NFT #{}{}", label, token_id, soulbound))
    }
    "templateMint" => Some(format!("Template mint: template #{}", fetch_field(f, "templateId"))),
    "propertiesSet" => Some(format!("Properties set for {}NFT #{}", label, fetch_field(f, "tokenId"))),
    "ownerChange" =>
      Some(
        format!(
          "NFT ownership transferred {} {} {}",
          fetch_field(f, "previousOwner"),
          arrow(),
          fetch_field(f, "newOwner")
        )
      ),
    "paused" => Some(format!("NFT paused by {}", fetch_field(f, "by"))),
    "unpaused" => Some(format!("NFT unpaused by {}", fetch_field(f, "by"))),
    "Approval" =>
      Some(
        format!(
          "{} approved {} to spend {} of {}NFT #{}",
          fetch_field(f, "owner"),
          fetch_field(f, "spender"),
          fetch_field(f, "amount"),
          label,
          fetch_field(f, "id")
        )
      ),
    "URI" => Some(format!("{}NFT #{} URI updated", label, fetch_field(f, "id"))),
    "baseUriChange" => Some(format!("{}Base URI changed", label)),
    "init_magi_nft" =>
      Some(format!("NFT collection initialized: {} ({})", fetch_field(f, "name"), fetch_field(f, "symbol"))),
    _ => None,
  }
}

fn describe_dex_pool(
  contract_id: &str,
  event_type: &str,
  f: &HashMap<String, String>,
  meta: &LogActionMetadata
) -> Option<String> {
  let pool = meta.pool_info.get(contract_id);
  let fmt_a0 = |v: &str| {
    pool.map(|p| fmt_dex_amount(v, &p.asset0)).unwrap_or_else(|| thousand_separator(v))
  };
  let fmt_a1 = |v: &str| {
    pool.map(|p| fmt_dex_amount(v, &p.asset1)).unwrap_or_else(|| thousand_separator(v))
  };
  match event_type {
    "swap" => {
      let in_amt = fetch_field(f, "ai");
      let in_asset = fetch_field(f, "in");
      let out_amt = fetch_field(f, "ao");
      let out_asset = fetch_field(f, "out");
      let to = fetch_field(f, "to");
      let mut s = format!("Swap {} for {}", fmt_dex_amount(in_amt, in_asset), fmt_dex_amount(out_amt, out_asset));
      if !to.is_empty() {
        s.push_str(&format!(" to {}", to));
      }
      Some(s)
    }
    "add_liq" =>
      Some(
        format!(
          "{} added liquidity: {} + {}, minted {} LP",
          fetch_field(f, "p"),
          fmt_a0(fetch_field(f, "a0")),
          fmt_a1(fetch_field(f, "a1")),
          thousand_separator(fetch_field(f, "lp"))
        )
      ),
    "rem_liq" =>
      Some(
        format!(
          "{} removed liquidity: {} + {}, burned {} LP",
          fetch_field(f, "p"),
          fmt_a0(fetch_field(f, "a0")),
          fmt_a1(fetch_field(f, "a1")),
          thousand_separator(fetch_field(f, "lp"))
        )
      ),
    "fee" => {
      let asset = if fetch_field(f, "a").is_empty() { fetch_field(f, "__asset") } else { fetch_field(f, "a") };
      let fmt = |v: &str| if asset.is_empty() { thousand_separator(v) } else { fmt_dex_amount(v, asset) };
      Some(
        format!(
          "Fee: total {}, LP {}, Magi {}",
          fmt(fetch_field(f, "t")),
          fmt(fetch_field(f, "lp")),
          fmt(fetch_field(f, "m"))
        )
      )
    }
    "pool_init" =>
      Some(
        format!(
          "Pool initialized: {}/{}, fee: {} bps",
          fetch_field(f, "a0").to_uppercase(),
          fetch_field(f, "a1").to_uppercase(),
          fetch_field(f, "fee")
        )
      ),
    "migrate" => Some(format!("Pool migrated to v{}", fetch_field(f, "v"))),
    _ => None,
  }
}

fn describe_dex_router(event_type: &str, f: &HashMap<String, String>) -> Option<String> {
  match event_type {
    "reg_pool" =>
      Some(
        format!(
          "Registered pool {} {}/{}",
          fetch_field(f, "pool"),
          fetch_field(f, "a0").to_uppercase(),
          fetch_field(f, "a1").to_uppercase()
        )
      ),
    _ => None,
  }
}

fn describe_btc_mapping(event_type: &str, f: &HashMap<String, String>) -> Option<String> {
  match event_type {
    "dep" | "map" => {
      let from = if fetch_field(f, "f").is_empty() { "Unknown" } else { fetch_field(f, "f") };
      Some(format!("{} mapped {} to {}", from, fmt_sats(fetch_field(f, "a")), fetch_field(f, "t")))
    }
    "xfer" =>
      Some(
        format!(
          "Transfer {} {} {} {}",
          fmt_sats(fetch_field(f, "a")),
          fetch_field(f, "f"),
          arrow(),
          fetch_field(f, "t")
        )
      ),
    "unm" | "unmap" => {
      let target = if fetch_field(f, "t").is_empty() { "Unknown" } else { fetch_field(f, "t") };
      Some(
        format!(
          "{} unmapped {} to {} (sent: {})",
          fetch_field(f, "f"),
          fmt_sats(fetch_field(f, "d")),
          target,
          fmt_sats(fetch_field(f, "s"))
        )
      )
    }
    "fee" => Some(format!("Fee: Magi {}, BTC {}", fmt_sats(fetch_field(f, "m")), fmt_sats(fetch_field(f, "b")))),
    "migrate" => Some(format!("Contract migrated to v{}", fetch_field(f, "v"))),
    _ => None,
  }
}

fn describe_inarow(event_type: &str, f: &HashMap<String, String>) -> Option<String> {
  match event_type {
    "c" =>
      Some(
        format!(
          "Game #{} created by {} bet: {} {}",
          fetch_field(f, "id"),
          fetch_field(f, "by"),
          fetch_field(f, "betamount"),
          fetch_field(f, "betasset").to_uppercase()
        )
      ),
    "j" => Some(format!("{} joined game #{}", fetch_field(f, "by"), fetch_field(f, "id"))),
    "m" =>
      Some(
        format!(
          "{} placed move in game #{}, cell {}",
          fetch_field(f, "by"),
          fetch_field(f, "id"),
          fetch_field(f, "cell")
        )
      ),
    "w" => Some(format!("{} won game #{}", fetch_field(f, "winner"), fetch_field(f, "id"))),
    "r" => Some(format!("{} resigned from game #{}", fetch_field(f, "resigner"), fetch_field(f, "id"))),
    "t" => Some(format!("{} timed out in game #{}", fetch_field(f, "timedout"), fetch_field(f, "id"))),
    "d" => Some(format!("Game #{} ended in draw", fetch_field(f, "id"))),
    "s" => Some(format!("{} swapped in game #{}", fetch_field(f, "by"), fetch_field(f, "id"))),
    _ => None,
  }
}

fn escrow_role(r: &str) -> &str {
  match r {
    "f" => "Sender",
    "t" => "Receiver",
    "arb" => "Arbitrator",
    other => other,
  }
}

fn escrow_decision(d: &str) -> &str {
  match d {
    "r" => "release",
    "f" => "refund",
    other => other,
  }
}

fn escrow_outcome(o: &str) -> &str {
  match o {
    "r" => "released to receiver",
    "f" => "refunded to sender",
    other => other,
  }
}

fn describe_escrow(event_type: &str, f: &HashMap<String, String>) -> Option<String> {
  match event_type {
    "cr" =>
      Some(
        format!(
          "Escrow #{} created: {} {} {} {} {}, arbitrator: {}",
          fetch_field(f, "id"),
          fetch_field(f, "f"),
          arrow(),
          fetch_field(f, "t"),
          fetch_field(f, "am"),
          fetch_field(f, "as").to_uppercase(),
          fetch_field(f, "arb")
        )
      ),
    "de" =>
      Some(
        format!(
          "Escrow #{}: {} {} decided: {}",
          fetch_field(f, "id"),
          escrow_role(fetch_field(f, "r")),
          fetch_field(f, "a"),
          escrow_decision(fetch_field(f, "d"))
        )
      ),
    "cl" => Some(format!("Escrow #{} closed, {}", fetch_field(f, "id"), escrow_outcome(fetch_field(f, "o")))),
    _ => None,
  }
}

fn describe_lottery(event_type: &str, f: &HashMap<String, String>) -> Option<String> {
  match event_type {
    "lc" =>
      Some(
        format!(
          "Lottery #{} created by {} \"{}\", ticket: {} {}",
          fetch_field(f, "id"),
          fetch_field(f, "creator"),
          fetch_field(f, "name"),
          fetch_field(f, "ticket"),
          fetch_field(f, "asset").to_uppercase()
        )
      ),
    "lj" =>
      Some(
        format!(
          "{} bought {} ticket(s) for lottery #{}",
          fetch_field(f, "participant"),
          fetch_field(f, "tickets"),
          fetch_field(f, "id")
        )
      ),
    "le" =>
      Some(
        format!(
          "Lottery #{} executed: {} participants, {} tickets, pool: {} {}",
          fetch_field(f, "id"),
          fetch_field(f, "participants"),
          fetch_field(f, "tickets"),
          fetch_field(f, "pool"),
          fetch_field(f, "asset").to_uppercase()
        )
      ),
    "lp" =>
      Some(
        format!(
          "Lottery #{} payout: {} won {} {} (position #{})",
          fetch_field(f, "id"),
          fetch_field(f, "winner"),
          fetch_field(f, "amount"),
          fetch_field(f, "asset").to_uppercase(),
          fetch_field(f, "position")
        )
      ),
    "ld" =>
      Some(
        format!(
          "Lottery #{} donation: {} {} to {}",
          fetch_field(f, "id"),
          fetch_field(f, "amount"),
          fetch_field(f, "asset").to_uppercase(),
          fetch_field(f, "recipient")
        )
      ),
    "lu" =>
      Some(
        format!(
          "Lottery #{}: {} {} undistributed",
          fetch_field(f, "id"),
          fetch_field(f, "amount"),
          fetch_field(f, "asset").to_uppercase()
        )
      ),
    "lm" => Some(format!("Lottery #{} metadata updated", fetch_field(f, "id"))),
    _ => None,
  }
}

fn describe_dao(event_type: &str, f: &HashMap<String, String>) -> Option<String> {
  match event_type {
    "dc" =>
      Some(
        format!(
          "DAO project #{} created by {} \"{}\"",
          fetch_field(f, "id"),
          fetch_field(f, "by"),
          fetch_field(f, "name")
        )
      ),
    "mj" => Some(format!("{} joined project #{}", fetch_field(f, "by"), fetch_field(f, "id"))),
    "ml" => Some(format!("{} left project #{}", fetch_field(f, "by"), fetch_field(f, "id"))),
    "af" => {
      let kind = if fetch_field(f, "s") == "true" { "(stake)" } else { "(treasury)" };
      Some(
        format!(
          "{} added {} {} to project #{} {}",
          fetch_field(f, "by"),
          fetch_field(f, "am"),
          fetch_field(f, "as").to_uppercase(),
          fetch_field(f, "id"),
          kind
        )
      )
    }
    "rf" => {
      let kind = if fetch_field(f, "s") == "true" { "(stake)" } else { "(treasury)" };
      Some(
        format!(
          "{} removed {} {} from project #{} {}",
          fetch_field(f, "by"),
          fetch_field(f, "am"),
          fetch_field(f, "as").to_uppercase(),
          fetch_field(f, "id"),
          kind
        )
      )
    }
    "pc" =>
      Some(
        format!(
          "Proposal #{} created by {} \"{}\"",
          fetch_field(f, "id"),
          fetch_field(f, "by"),
          fetch_field(f, "name")
        )
      ),
    "ps" => Some(format!("Proposal #{} state changed to: {}", fetch_field(f, "id"), fetch_field(f, "s"))),
    "px" =>
      Some(format!("Proposal #{} in project #{} ready for execution", fetch_field(f, "prId"), fetch_field(f, "pId"))),
    "pr" =>
      Some(
        format!(
          "Proposal #{} in project #{} result: {}",
          fetch_field(f, "prId"),
          fetch_field(f, "pId"),
          fetch_field(f, "r")
        )
      ),
    "pm" => {
      let old = fetch_field(f, "old");
      let suffix = if !old.is_empty() { format!(" {} {} {}", old, arrow(), fetch_field(f, "new")) } else { String::new() };
      Some(
        format!(
          "Proposal #{} in project #{}: {} changed{}",
          fetch_field(f, "prId"),
          fetch_field(f, "pId"),
          fetch_field(f, "f"),
          suffix
        )
      )
    }
    "v" =>
      Some(
        format!(
          "{} voted on proposal #{} with weight {}",
          fetch_field(f, "by"),
          fetch_field(f, "id"),
          fetch_field(f, "w")
        )
      ),
    "wl" => {
      let action = if fetch_field(f, "a") == "add" { "added" } else { "removed" };
      Some(
        format!("Whitelist {} in project #{}: {}", action, fetch_field(f, "pId"), fetch_field(f, "ad"))
      )
    }
    _ => None,
  }
}

pub fn describe_action(
  contract_id: &str,
  contract_type: Option<&str>,
  event_type: &str,
  fields: &HashMap<String, String>,
  metadata: &LogActionMetadata
) -> Option<String> {
  match contract_type? {
    "magi_token" => describe_token(contract_id, event_type, fields, metadata),
    "magi_nft" => describe_nft(contract_id, event_type, fields, metadata),
    "dex_pool" => describe_dex_pool(contract_id, event_type, fields, metadata),
    "dex_router" => describe_dex_router(event_type, fields),
    "btc_mapping" => describe_btc_mapping(event_type, fields),
    "oki_inarow" => describe_inarow(event_type, fields),
    "oki_escrow" => describe_escrow(event_type, fields),
    "oki_lottery" => describe_lottery(event_type, fields),
    "oki_dao" => describe_dao(event_type, fields),
    _ => None,
  }
}

#[derive(Debug, Deserialize)]
pub struct ContractOutputResult {
  #[allow(dead_code)]
  pub ok: Option<bool>,
  pub logs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ContractOutputDag {
  pub contract_id: String,
  pub results: Vec<ContractOutputResult>,
}

pub async fn fetch_dag_batch(
  client: &reqwest::Client,
  gql_url: &str,
  cids: &[String]
) -> Result<Vec<ContractOutputDag>, Box<dyn std::error::Error + Send + Sync>> {
  if cids.is_empty() {
    return Ok(Vec::new());
  }
  let mut frags: Vec<String> = Vec::with_capacity(cids.len());
  let mut params: Vec<String> = Vec::with_capacity(cids.len());
  let mut vars = serde_json::Map::new();
  for (i, cid) in cids.iter().enumerate() {
    frags.push(format!("d{}: getDagByCID(cidString: $cid{})", i, i));
    params.push(format!("$cid{}: String!", i));
    vars.insert(format!("cid{}", i), Value::String(cid.clone()));
  }
  let query = format!("query DagByCID({}) {{ {} }}", params.join(", "), frags.join(" "));
  let body = json!({ "query": query, "variables": Value::Object(vars) });
  let resp = client
    .post(gql_url)
    .header("accept", "application/json")
    .header("content-type", "application/json")
    .json(&body)
    .send().await?;
  if !resp.status().is_success() {
    return Err(format!("DAG batch query failed: HTTP {}", resp.status()).into());
  }
  let parsed: Value = resp.json().await?;
  let data = match parsed.get("data") {
    Some(d) => d,
    None => {
      return Ok(Vec::new());
    }
  };
  let mut out = Vec::with_capacity(cids.len());
  for i in 0..cids.len() {
    let raw = data
      .get(format!("d{}", i))
      .and_then(|v| v.as_str())
      .unwrap_or("");
    if raw.is_empty() {
      continue;
    }
    if let Ok(parsed) = serde_json::from_str::<ContractOutputDag>(raw) {
      out.push(parsed);
    }
  }
  Ok(out)
}

pub async fn fetch_log_metadata(
  client: &reqwest::Client,
  hasura_url: &str,
  contract_ids: &[String]
) -> Result<LogActionMetadata, Box<dyn std::error::Error + Send + Sync>> {
  if contract_ids.is_empty() {
    return Ok(LogActionMetadata::default());
  }
  let query =
    r#"query LogActionMeta($ids: [String!]!) {
      contract_type_lookup(where: { contract_id: { _in: $ids } }) { contract_id contract_type }
      magi_token_registry(where: { contract_id: { _in: $ids } }) { contract_id symbol decimals }
      magi_nft_registry(where: { contract_id: { _in: $ids } }) { contract_id name symbol }
      dex_pool_registry(where: { pool_contract: { _in: $ids } }) { pool_contract asset0 asset1 }
    }"#;
  let body = json!({ "query": query, "variables": { "ids": contract_ids } });
  let resp = client
    .post(hasura_url)
    .header("accept", "application/json")
    .header("content-type", "application/json")
    .json(&body)
    .send().await?;
  if !resp.status().is_success() {
    return Err(format!("Hasura metadata query failed: HTTP {}", resp.status()).into());
  }
  let parsed: Value = resp.json().await?;
  let data = parsed.get("data").cloned().unwrap_or(Value::Null);
  let mut meta = LogActionMetadata::default();
  if let Some(arr) = data.get("contract_type_lookup").and_then(|v| v.as_array()) {
    for item in arr {
      let cid = item.get("contract_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let ct = item.get("contract_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
      if !cid.is_empty() {
        meta.contract_types.insert(cid, ct);
      }
    }
  }
  if let Some(arr) = data.get("magi_token_registry").and_then(|v| v.as_array()) {
    for item in arr {
      let cid = item.get("contract_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let symbol = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let decimals = item.get("decimals").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
      if !cid.is_empty() {
        meta.token_info.insert(cid, TokenMeta { symbol, decimals });
      }
    }
  }
  if let Some(arr) = data.get("magi_nft_registry").and_then(|v| v.as_array()) {
    for item in arr {
      let cid = item.get("contract_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let symbol = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("").to_string();
      if !cid.is_empty() {
        meta.nft_info.insert(cid, NftMeta { name, symbol });
      }
    }
  }
  if let Some(arr) = data.get("dex_pool_registry").and_then(|v| v.as_array()) {
    for item in arr {
      let cid = item.get("pool_contract").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let asset0 = item.get("asset0").and_then(|v| v.as_str()).unwrap_or("").to_string();
      let asset1 = item.get("asset1").and_then(|v| v.as_str()).unwrap_or("").to_string();
      if !cid.is_empty() {
        meta.pool_info.insert(cid, PoolMeta { asset0, asset1 });
      }
    }
  }
  Ok(meta)
}

pub fn unique_dynamic_contract_ids(outputs: &[ContractOutputDag]) -> Vec<String> {
  let mut set: HashSet<String> = HashSet::new();
  for o in outputs {
    if static_contract_type(&o.contract_id).is_none() {
      set.insert(o.contract_id.clone());
    }
  }
  set.into_iter().collect()
}
