use actix_web::{ get, web, HttpResponse, Responder };
use serde_json;
use log;
use crate::db::DbPool;

const GENERIC_DB_ERR: &str = "Unknown error occured when querying database";

#[get("/")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[get("/languages")]
async fn list_langs(ctx: web::Data<DbPool>) -> impl Responder {
  let db = ctx.get_ref().clone();
  let rows = match db.query("SELECT jsonb_agg(name) FROM vsc_cv.languages;", &[]).await {
    Ok(r) => r,
    Err(e) => {
      log::error!("GET /languages failed: {}", e.to_string());
      return HttpResponse::BadGateway().json(serde_json::json!({ "error": GENERIC_DB_ERR }));
    }
  };
  let result: serde_json::Value = rows[0].get(0);
  HttpResponse::Ok().json(result)
}

#[get("/licenses")]
async fn list_licenses(ctx: web::Data<DbPool>) -> impl Responder {
  let db = ctx.get_ref().clone();
  let rows = match db.query("SELECT jsonb_agg(name) FROM vsc_cv.licenses;", &[]).await {
    Ok(r) => r,
    Err(e) => {
      log::error!("GET /licenses failed: {}", e.to_string());
      return HttpResponse::BadGateway().json(serde_json::json!({ "error": GENERIC_DB_ERR }));
    }
  };
  let result: serde_json::Value = rows[0].get(0);
  HttpResponse::Ok().json(result)
}
