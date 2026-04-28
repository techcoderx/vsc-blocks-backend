use formatter::thousand_separator;
use serde_json::Value;

use crate::types::vsc::OpInfo;

fn abbreviate(s: &str, first_chars: usize) -> String {
  if s.chars().count() <= first_chars + 2 {
    return s.to_string();
  }
  let prefix: String = s.chars().take(first_chars).collect();
  format!("{}...", prefix)
}

fn fmt_amount(amount: &Value) -> String {
  let n: f64 = match amount {
    Value::Number(n) => n.as_f64().unwrap_or(0.0),
    Value::String(s) => s.parse().unwrap_or(0.0),
    _ => 0.0,
  };
  thousand_separator(format!("{:.3}", n / 1000.0))
}

fn fmt_asset(asset: Option<&Value>) -> String {
  asset.and_then(|v| v.as_str()).unwrap_or("").to_uppercase()
}

fn to_suffix(data: &Value, signer: Option<&str>) -> String {
  let to = data.get("to").and_then(|v| v.as_str()).unwrap_or("");
  if to.is_empty() {
    return String::new();
  }
  let signer_addr = signer.map(|s| format!("hive:{}", s));
  if Some(to) == signer_addr.as_deref() {
    return String::new();
  }
  format!(" to {}", to)
}

pub fn op_method(op: &OpInfo) -> String {
  if op.r#type == "call" {
    let action = op.data
      .as_ref()
      .and_then(|d| d.get("action"))
      .and_then(|v| v.as_str())
      .unwrap_or("");
    if !action.is_empty() {
      return abbreviate(action, 20);
    }
  }
  op.r#type.clone()
}

pub fn describe_op(op: &OpInfo, signer: Option<&str>) -> String {
  let data = match &op.data {
    Some(d) => d,
    None => {
      return String::from("*N/A*");
    }
  };
  let amt = data.get("amount").map(fmt_amount).unwrap_or_default();
  let asset = fmt_asset(data.get("asset"));
  let suffix = to_suffix(data, signer);

  match op.r#type.as_str() {
    "call" => String::new(),
    "transfer" => format!("Transfer {} {}{}", amt, asset, suffix),
    "deposit" => format!("Deposit {} {}{}", amt, asset, suffix),
    "withdraw" => format!("Withdraw {} {}{}", amt, asset, suffix),
    "consensus_stake" => format!("Stake {} HIVE{}", amt, suffix),
    "consensus_unstake" => format!("Unstake {} HIVE{}", amt, suffix),
    "stake_hbd" => format!("Stake {} HBD{}", amt, suffix),
    "unstake_hbd" => format!("Unstake {} HBD{}", amt, suffix),
    _ => String::from("*N/A*"),
  }
}
