use actix_web::{ http::{ header::ContentType, StatusCode }, HttpResponse };
use derive_more::derive::{ Display, Error };
use serde::Serialize;
use reqwest;
use utoipa::{ ToResponse, ToSchema };
use log::error;
use std::fmt;
use crate::{ compiler::Compiler, mongo::MongoDB };

#[derive(Display, Error)]
pub enum RespErr {
  #[display("Unknown error occured when querying database")] DbErr {
    msg: String,
  },
  #[display("Missing access token in authentication header")] TokenMissing,
  #[display("Access token expired")] TokenExpired,
  #[display("Access token is invalid")] TokenInvalid,
  #[display("Failed to make signature verification request")] SigVerifyReqFail,
  #[display("Failed to verify signature")] SigVerifyFail,
  #[display("Failed to check for recent block")] SigRecentBlkReqFail,
  #[display("Signature is too old")] SigTooOld,
  #[display("Block hash does not match the corresponding block number")] SigBhNotMatch,
  #[display("Failed to generate access token")] TokenGenFail,
  #[display("Contract not found")] ContractNotFound,
  #[display("Contract verifier is disabled")] CvDisabled,
  #[display("Only contract deployer or owner can request verification")] CvNotAuthorized,
  #[display("Only whitelisted users can request verification")] CvNotWhitelisted,
  #[display("Invalid GitHub repository URL")] CvInvalidGitHubURL,
  #[display("Invalid git branch name")] CvInvalidGitBranch,
  #[display("Invalid Wasm strip tool name")] CvInvalidWasmStripTool,
  #[display("Invalid TinyGo version")] CvInvalidTinyGoVersion,
  #[display("Verification retry is only allowed 12 hours after the previous request time")] CvRetryLater,
  #[display("Another contract with exact bytecode was already verified")] CvSimilarMatch,
  #[display("{msg}")] InternalErr {
    msg: String,
  },
  #[display("{msg}")] BadRequest {
    msg: String,
  },
}

impl fmt::Debug for RespErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RespErr::DbErr { msg } => write!(f, "{}", msg),
      _ => Ok(()),
    }
  }
}

impl actix_web::error::ResponseError for RespErr {
  fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
    let e = format!("{:?}", self);
    if e.len() > 0 {
      error!("{}", e);
    }
    HttpResponse::build(self.status_code()).insert_header(ContentType::json()).json(ErrorRes { error: self.to_string() })
  }

  fn status_code(&self) -> StatusCode {
    match *self {
      RespErr::DbErr { .. } => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::TokenMissing => StatusCode::UNAUTHORIZED,
      RespErr::TokenExpired => StatusCode::UNAUTHORIZED,
      RespErr::TokenInvalid => StatusCode::UNAUTHORIZED,
      RespErr::SigVerifyReqFail => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::SigVerifyFail => StatusCode::UNAUTHORIZED,
      RespErr::SigRecentBlkReqFail => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::SigTooOld => StatusCode::UNAUTHORIZED,
      RespErr::SigBhNotMatch => StatusCode::UNAUTHORIZED,
      RespErr::TokenGenFail => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::InternalErr { .. } => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::BadRequest { .. } => StatusCode::BAD_REQUEST,
      RespErr::ContractNotFound => StatusCode::NOT_FOUND,
      RespErr::CvDisabled => StatusCode::IM_A_TEAPOT,
      RespErr::CvNotAuthorized => StatusCode::FORBIDDEN,
      RespErr::CvNotWhitelisted => StatusCode::FORBIDDEN,
      RespErr::CvInvalidGitHubURL => StatusCode::BAD_REQUEST,
      RespErr::CvInvalidGitBranch => StatusCode::BAD_REQUEST,
      RespErr::CvInvalidWasmStripTool => StatusCode::BAD_REQUEST,
      RespErr::CvInvalidTinyGoVersion => StatusCode::BAD_REQUEST,
      RespErr::CvRetryLater => StatusCode::TOO_MANY_REQUESTS,
      RespErr::CvSimilarMatch => StatusCode::FOUND,
    }
  }
}

#[derive(Clone)]
pub struct Context {
  pub db: MongoDB,
  pub compiler: Option<Compiler>,
  pub http_client: reqwest::Client,
}

#[derive(Serialize, ToSchema, ToResponse)]
pub struct SuccessRes {
  pub success: bool,
}

#[derive(Serialize, ToSchema, ToResponse)]
pub struct ErrorRes {
  error: String,
}
