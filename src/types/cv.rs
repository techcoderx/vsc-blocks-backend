use serde::{ Deserialize, Serialize };
use utoipa::{ ToResponse, ToSchema };
use mongodb::bson::DateTime;
use lazy_static::lazy_static;
use std::{ collections::HashMap, fmt };

pub enum CVStatus {
  // Pending,
  Queued,
  // InProgress,
  Success,
  Failed,
  NotMatch,
}

impl fmt::Display for CVStatus {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      // CVStatus::Pending => write!(f, "pending"),
      CVStatus::Queued => write!(f, "queued"),
      // CVStatus::InProgress => write!(f, "in progress"),
      CVStatus::Success => write!(f, "success"),
      CVStatus::Failed => write!(f, "failed"),
      CVStatus::NotMatch => write!(f, "not match"),
    }
  }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CVTinyGoLibVersions {
  pub go: String,
  pub llvm: String,
  pub img_digest: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CVContract {
  #[serde(rename = "_id")]
  pub id: String,
  pub code: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub verifier: Option<String>,
  pub repo_name: String,
  pub repo_branch: String,
  pub git_commit: Option<String>,
  pub tinygo_version: String,
  pub go_version: String,
  pub llvm_version: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub strip_tool: Option<String>,
  pub request_ts: DateTime,
  pub verified_ts: Option<DateTime>,
  pub status: String,
  pub exports: Option<Vec<String>>,
  pub license: Option<String>,
  pub lang: String,
}

#[derive(Clone, Serialize, ToResponse, ToSchema)]
pub struct CVContractResult {
  /// Contract address
  pub address: String,
  /// Contract bytecode CID
  pub code: String,
  /// Whether this contract was verified through identical bytecode of another contract
  pub similar_match: Option<String>,
  /// Username whom verified the contract
  pub verifier: Option<String>,
  /// Contract verification request timestamp
  pub request_ts: String,
  /// Contract verification completion timestamp
  pub verified_ts: Option<String>,
  /// Contract verification status (pending, queued, in progress, success, failed, not match)
  pub status: String,
  /// Repository name
  pub repo_name: String,
  /// Git branch
  pub repo_branch: String,
  /// Git commit hash
  pub git_commit: Option<String>,
  /// TinyGo compiler version
  pub tinygo_version: String,
  /// Go version
  pub go_version: String,
  /// LLVM version
  pub llvm_version: String,
  /// WASM strip tool that was used on the compiled output
  pub strip_tool: Option<String>,
  /// Contract public exports
  pub exports: Option<Vec<String>>,
  /// SPDX identifier of contract source code license as listed in https://spdx.org/licenses
  pub license: Option<String>,
  /// Language of contract source code
  pub lang: String,
}

#[derive(Clone, Deserialize)]
pub struct GithubRepoInfo {
  pub default_branch: String,
  pub size: usize,
  pub license: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct GithubBranchCommit {
  pub sha: String,
}

#[derive(Clone, Deserialize)]
pub struct GithubBranchInfo {
  pub commit: GithubBranchCommit,
}

lazy_static! {
  /// https://hub.docker.com/r/tinygo/tinygo/tags
  pub static ref tinygo_versions: HashMap<String, CVTinyGoLibVersions> = HashMap::from([
    (
      format!("0.38.0"),
      CVTinyGoLibVersions {
        go: format!("1.24.4"),
        llvm: format!("19.1.2"),
        img_digest: format!("sha256:98447dff0e56426b98f96a1d47ac7c1d82d27e3cd630cba81732cfc13c9a410f")
      },
    ),
    (
      format!("0.39.0"),
      CVTinyGoLibVersions {
        go: format!("1.25.0"),
        llvm: format!("19.1.2"),
        img_digest: format!("sha256:0e51d243c1b84ec650f2dcd1cce3a09bb09730e1134771aeace2240ade4b32f5")
      },
    ),
  ]);
}
