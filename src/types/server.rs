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
  #[display("Contract verifier is disabled")] CvDisabled,
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
      RespErr::CvDisabled => StatusCode::IM_A_TEAPOT,
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
