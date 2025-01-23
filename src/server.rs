use actix_web::{ http::{ header::ContentType, StatusCode }, get, post, web, HttpResponse, Responder };
use derive_more::derive::{ Display, Error };
use tokio_postgres::types::Type;
use serde::{ Serialize, Deserialize };
use serde_json;
use reqwest;
use log::error;
use std::fmt;
use crate::db::DbPool;
use crate::vsc_types;
use crate::config::config;

#[derive(Display, Error)]
enum RespErr {
  #[display("Unknown error occured when querying database")] DbErr {
    msg: String,
  },
  #[display("Failed to query VSC-HAF backend")] VscHafErr,
  #[display("{msg}")] BadRequest {
    msg: String,
  },
}

impl fmt::Debug for RespErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RespErr::DbErr { msg } => write!(f, "{}", msg),
      RespErr::VscHafErr => Ok(()),
      RespErr::BadRequest { .. } => Ok(()),
    }
  }
}

impl actix_web::error::ResponseError for RespErr {
  fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
    let e = format!("{:?}", self);
    if e.len() > 0 {
      error!("{}", e);
    }
    HttpResponse::build(self.status_code())
      .insert_header(ContentType::json())
      .json(serde_json::json!({ "error": self.to_string() }))
  }

  fn status_code(&self) -> StatusCode {
    match *self {
      RespErr::DbErr { .. } => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::VscHafErr => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::BadRequest { .. } => StatusCode::BAD_REQUEST,
    }
  }
}

#[get("/")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[derive(Serialize, Deserialize)]
struct ReqVerifyNew {
  address: String,
  username: String,
  license: String,
  lang: String,
  dependencies: serde_json::Value,
}

#[post("/verify/new")]
async fn verify_new(req_data: web::Json<ReqVerifyNew>, ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let ct_req_method = config.vsc_haf_url.clone() + "/get_contract_by_id?id=" + &req_data.address;
  let ct_det = reqwest
    ::get(ct_req_method.as_str()).await
    .map_err(|_| RespErr::VscHafErr)?
    .json::<vsc_types::ContractById>().await
    .map_err(|_| RespErr::VscHafErr)?;
  if ct_det.error.is_some() {
    // as of now only error this api could return is contract not found with status code 200
    return Ok(HttpResponse::NotFound().json(serde_json::json!(ct_det)));
  }
  let can_verify: String = db
    .query(
      "SELECT vsc_cv.can_verify_new($1,$2,$3);",
      &[
        (&req_data.address, Type::VARCHAR),
        (&req_data.license, Type::VARCHAR),
        (&req_data.lang, Type::VARCHAR),
      ]
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    [0].get(0);
  if can_verify.len() > 0 {
    return Err(RespErr::BadRequest { msg: can_verify });
  }
  db
    .query(
      "SELECT vsc_cv.verify_new($1,$2,$3,0::SMALLINT,$4,$5,$6);",
      &[
        (&ct_det.contract_id, Type::VARCHAR),
        (&ct_det.code, Type::VARCHAR),
        (&req_data.username, Type::VARCHAR),
        (&req_data.license, Type::VARCHAR),
        (&req_data.lang, Type::VARCHAR),
        (&req_data.dependencies, Type::JSONB),
      ]
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[get("/languages")]
async fn list_langs(ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let rows = db
    .query("SELECT jsonb_agg(name) FROM vsc_cv.languages;", &[]).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let result: serde_json::Value = rows[0].get(0);
  Ok(HttpResponse::Ok().json(result))
}

#[get("/licenses")]
async fn list_licenses(ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let rows = db
    .query("SELECT jsonb_agg(name) FROM vsc_cv.licenses;", &[]).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let result: serde_json::Value = rows[0].get(0);
  Ok(HttpResponse::Ok().json(result))
}
