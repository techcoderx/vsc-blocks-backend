use actix_web::{ http::{ header::ContentType, StatusCode }, get, post, web, HttpResponse, Responder };
use derive_more::derive::{ Display, Error };
use tokio_postgres::types::Type;
use serde::{ Serialize, Deserialize };
use serde_json;
use reqwest;
use crate::db::DbPool;
use crate::vsc_types;
use crate::config::config;

#[derive(Debug, Display, Error)]
enum RespErr {
  #[display("Unknown error occured when querying database")]
  DbErr,
  #[display("Failed to query VSC-HAF backend")] VscHafErr,
}

impl actix_web::error::ResponseError for RespErr {
  fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
    HttpResponse::build(self.status_code())
      .insert_header(ContentType::json())
      .json(serde_json::json!({ "error": self.to_string() }))
  }

  fn status_code(&self) -> StatusCode {
    match *self {
      RespErr::DbErr => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::VscHafErr => StatusCode::INTERNAL_SERVER_ERROR,
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
    return Ok(HttpResponse::NotFound().json(serde_json::json!(ct_det)));
  }
  db
    .query(
      "SELECT vsc_cv.verify_new($1,$2,$3,0::SMALLINT,$4,$5,NULL)",
      &[
        (&ct_det.contract_id, Type::VARCHAR),
        (&ct_det.code, Type::VARCHAR),
        (&req_data.username, Type::VARCHAR),
        (&req_data.license, Type::VARCHAR),
        (&req_data.lang, Type::VARCHAR),
      ]
    ).await
    .map_err(|_| RespErr::DbErr)?;
  Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[get("/languages")]
async fn list_langs(ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let rows = match db.query("SELECT jsonb_agg(name) FROM vsc_cv.languages;", &[]).await {
    Ok(r) => r,
    Err(e) => {
      log::error!("GET /languages failed: {}", e.to_string());
      return Err(RespErr::DbErr);
    }
  };
  let result: serde_json::Value = rows[0].get(0);
  Ok(HttpResponse::Ok().json(result))
}

#[get("/licenses")]
async fn list_licenses(ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let rows = match db.query("SELECT jsonb_agg(name) FROM vsc_cv.licenses;", &[]).await {
    Ok(r) => r,
    Err(e) => {
      log::error!("GET /licenses failed: {}", e.to_string());
      return Err(RespErr::DbErr);
    }
  };
  let result: serde_json::Value = rows[0].get(0);
  Ok(HttpResponse::Ok().json(result))
}
