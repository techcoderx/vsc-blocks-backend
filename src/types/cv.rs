use serde::{ Deserialize, Serialize };
use serde_json::Value;
use utoipa::{ ToResponse, ToSchema };
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

#[derive(Clone, Serialize, ToResponse, ToSchema)]
pub struct CVContractResult {
  /// Contract address
  pub address: String,
  /// Contract bytecode CID
  pub code: String,
  /// Username that submitted the contract source code
  pub username: Option<String>,
  /// Contract verification request timestamp
  pub request_ts: String,
  /// Contract verification completion timestamp
  pub verified_ts: Option<String>,
  /// Contract verification status (pending, queued, in progress, success, failed, not match)
  pub status: String,
  /// Contract public exports
  pub exports: Option<Vec<String>>,
  /// List of source code filenames for the contract
  pub files: Vec<String>,
  /// Filename of the lockfile (if any)
  pub lockfile: Option<String>,
  /// SPDX identifier of contract source code license as listed in https://spdx.org/licenses
  pub license: String,
  /// Language of contract source code
  pub lang: String,
  /// Contract dependencies
  pub dependencies: Option<Value>,
}

#[derive(Clone, Serialize, ToResponse, ToSchema)]
pub struct CVContractCatFile {
  /// Filename of source code
  pub name: String,
  /// Content of source code file
  pub content: String,
}
