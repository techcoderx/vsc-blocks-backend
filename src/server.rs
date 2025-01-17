use actix_web::{ get, web, HttpResponse, Responder };
use deadpool_postgres::Manager;
use deadpool::managed::Pool;
use serde_json;
use crate::db;

#[get("/")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[get("/languages")]
async fn list_langs(db_pool: web::Data<Pool<Manager>>) -> impl Responder {
  let rows = db
    ::query(
      db_pool.get_ref().clone(),
      "SELECT jsonb_agg(jsonb_build_object('id',id,'name',name)) FROM vsc_cv.languages",
      &[]
    ).await
    .unwrap();
  let result: serde_json::Value = rows[0].get(0);
  HttpResponse::Ok().json(result)
}

#[get("/licenses")]
async fn list_licenses(db_pool: web::Data<Pool<Manager>>) -> impl Responder {
  let rows = db
    ::query(
      db_pool.get_ref().clone(),
      "SELECT jsonb_agg(jsonb_build_object('id',id,'name',name)) FROM vsc_cv.licenses",
      &[]
    ).await
    .unwrap();
  let result: serde_json::Value = rows[0].get(0);
  HttpResponse::Ok().json(result)
}
