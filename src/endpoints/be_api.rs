use actix_web::{ get, web, HttpResponse, Responder };
use futures_util::StreamExt;
use mongodb::{ bson::doc, options::{ FindOneOptions, FindOptions } };
use serde::Deserialize;
use serde_json::json;
use std::cmp::max;
use crate::server_types::{ Context, RespErr };

#[get("")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[get("/props")]
async fn props(ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let pipeline = vec![doc! {
      "$group": {
        "_id": "$account"
      }
    }, doc! { "$count": "total" }];

  let mut wit_cursor = ctx.vsc_db.witnesses.aggregate(pipeline).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let witness_count = wit_cursor
    .next().await
    .transpose()
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    .map(|d| d.get_i32("total").unwrap_or(0))
    .unwrap_or(0);
  let epoch = ctx.vsc_db.elections
    .find_one(doc! {})
    .with_options(
      FindOneOptions::builder()
        .sort(doc! { "epoch": -1 })
        .build()
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    .map(|epoch| epoch.epoch);
  Ok(HttpResponse::Ok().json(json!({
    "witnesses": witness_count,
    "epoch": epoch
  })))
}

#[get("/witnesses")]
async fn list_witnesses(ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let pipeline = vec![
    doc! { "$sort": { "account": 1, "height": -1 } },
    doc! { 
      "$group": {
        "_id": "$account",
        "doc": { "$first": "$$ROOT" }
      }
    },
    doc! { "$replaceRoot": { "newRoot": "$doc" } },
    // New projection stage to exclude _id
    doc! { 
      "$project": {
        "_id": 0
      }
    }
  ];

  let mut cursor = ctx.vsc_db.witnesses.aggregate(pipeline).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = cursor.next().await {
    results.push(doc.unwrap());
  }

  // Convert each MongoDB document to serde_json::Value
  let json_results = results
    .into_iter()
    .map(|doc| serde_json::to_value(doc).map_err(|e| RespErr::DbErr { msg: e.to_string() }))
    .collect::<Result<Vec<_>, _>>()?;

  // Return the JSON array
  Ok(HttpResponse::Ok().json(json_results))
}

#[get("/witness/{username}")]
async fn get_witness(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let user = path.into_inner();
  let opt = FindOneOptions::builder()
    .sort(doc! { "height": -1 })
    .build();
  let wit = ctx.vsc_db.witnesses
    .find_one(doc! { "account": &user })
    .with_options(opt).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if wit.is_none() {
    return Ok(HttpResponse::NotFound().json(json!({"error": "witness does not exist"})));
  }
  Ok(HttpResponse::Ok().json(wit.unwrap()))
}

#[derive(Debug, Deserialize)]
struct ListEpochOpts {
  last_epoch: Option<i64>,
  count: Option<i64>,
}

#[get("/epochs")]
async fn list_epochs(params: web::Query<ListEpochOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let last_epoch = params.last_epoch;
  let count = max(params.count.unwrap_or(100), 100);
  let opt = FindOptions::builder()
    .sort(doc! { "epoch": -1 })
    .build();
  let filter = match last_epoch.is_some() {
    true => doc! { "epoch": doc! {"$lte": last_epoch.unwrap()} },
    false => doc! {},
  };
  let mut epochs_cursor = ctx.vsc_db.elections
    .find(filter)
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = epochs_cursor.next().await {
    results.push(doc.unwrap());
  }

  let json_results = results
    .into_iter()
    .map(|doc| serde_json::to_value(doc).map_err(|e| RespErr::DbErr { msg: e.to_string() }))
    .collect::<Result<Vec<_>, _>>()?;

  // Return the JSON array
  Ok(HttpResponse::Ok().json(json_results))
}

#[get("/epoch/{epoch}")]
async fn get_epoch(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let epoch_num = path.into_inner().parse::<i32>();
  if epoch_num.is_err() {
    return Err(RespErr::BadRequest { msg: String::from("Invalid epoch number") });
  }
  let epoch = ctx.vsc_db.elections
    .find_one(doc! { "epoch": epoch_num.unwrap() }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if epoch.is_none() {
    return Ok(HttpResponse::NotFound().json(json!({"error": "epoch does not exist"})));
  }
  Ok(HttpResponse::Ok().json(epoch.unwrap()))
}
