use serde::{ Deserialize, Serialize };
use serde_json::Value;
use mongodb::bson::DateTime;
use std::fmt;

pub enum CVStatus {
  Pending,
  Queued,
  InProgress,
  Success,
  Failed,
  NotMatch,
}

impl fmt::Display for CVStatus {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CVStatus::Pending => write!(f, "pending"),
      CVStatus::Queued => write!(f, "queued"),
      CVStatus::InProgress => write!(f, "in progress"),
      CVStatus::Success => write!(f, "success"),
      CVStatus::Failed => write!(f, "failed"),
      CVStatus::NotMatch => write!(f, "not match"),
    }
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CVIdName {
  #[serde(rename = "_id")]
  pub id: i32,
  pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CVContract {
  #[serde(rename = "_id")]
  pub id: String,
  pub bytecode_cid: String,
  pub username: Option<String>,
  pub request_ts: DateTime,
  pub verified_ts: Option<DateTime>,
  pub status: String,
  pub exports: Option<Vec<String>>,
  pub license: String,
  pub lang: String,
  pub dependencies: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CVContractCode {
  pub addr: String,
  pub fname: String,
  pub is_lockfile: bool,
  pub content: String,
}
