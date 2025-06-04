use actix_web::{ get, web, HttpResponse, Responder };
use futures_util::StreamExt;
use mongodb::{ bson::doc, options::{ FindOptions } };
use serde::Deserialize;
use serde_json::{ json, Value };
use std::cmp::{ min, max };
use crate::{
  config::config,
  helpers::db::{ get_props, get_witness_stats },
  types::{ hive::{ CustomJson, TxByHash }, server::{ Context, RespErr }, vsc::{ BridgeStats, UserStats, WitnessStat } },
};

#[get("")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[get("/props")]
async fn props(ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let props = get_props(&ctx.db).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(props))
}

#[get("/witness/{username}/stats")]
async fn witness_stats(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let user = path.into_inner();
  let stats = get_witness_stats(&ctx.db, user).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(stats))
}

#[get("/witness/{username}/stats/many")]
async fn get_witness_stats_many(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let p = path.into_inner();
  if p.len() == 0 {
    return Err(RespErr::BadRequest { msg: String::from("Invalid username") });
  }
  let users = p.split(",").collect::<Vec<&str>>();
  if p.len() > 1000 {
    return Err(RespErr::BadRequest { msg: String::from("Max 1000 usernames allowed") });
  }
  let mut stats: Vec<WitnessStat> = Vec::new();
  for user in users {
    stats.push(get_witness_stats(&ctx.db, String::from(user)).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?);
  }
  Ok(HttpResponse::Ok().json(stats))
}

#[derive(Debug, Deserialize)]
struct ListEpochOpts {
  last_epoch: Option<i64>,
  count: Option<i64>,
  proposer: Option<String>,
}

#[get("/epochs")]
async fn list_epochs(params: web::Query<ListEpochOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let last_epoch = params.last_epoch;
  let proposer = params.proposer.clone();
  let count = min(max(1, params.count.unwrap_or(100)), 100);
  let opt = FindOptions::builder()
    .sort(doc! { "epoch": -1 })
    .build();
  let mut filter = match last_epoch {
    Some(le) => doc! { "epoch": doc! {"$lte": le} },
    None => doc! {},
  };
  if proposer.is_some() {
    filter.insert("proposer", &proposer.unwrap());
  }
  let mut epochs_cursor = ctx.db.elections
    .find(filter)
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = epochs_cursor.next().await {
    results.push(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?);
  }
  Ok(HttpResponse::Ok().json(results))
}

#[get("/epoch/{epoch}")]
async fn get_epoch(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let epoch_num = path
    .into_inner()
    .parse::<i32>()
    .map_err(|_| RespErr::BadRequest { msg: String::from("Invalid epoch number") })?;
  let epoch = ctx.db.elections.find_one(doc! { "epoch": epoch_num }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match epoch {
    Some(ep) => Ok(HttpResponse::Ok().json(ep)),
    None => Ok(HttpResponse::NotFound().json(json!({"error": "Epoch does not exist"}))),
  }
}

#[derive(Debug, Deserialize)]
struct ListBlockOpts {
  last_block_id: Option<i64>,
  offset: Option<u64>,
  count: Option<i64>,
  proposer: Option<String>,
  epoch: Option<i64>,
}

#[get("/blocks")]
async fn list_blocks(params: web::Query<ListBlockOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let offset = params.offset.unwrap_or(0);
  if offset > 100000 {
    return Err(RespErr::BadRequest { msg: String::from("Invalid offset") });
  }
  let last_block_id = params.last_block_id;
  let proposer = params.proposer.clone();
  let epoch = params.epoch;
  let count = min(max(1, params.count.unwrap_or(100)), 100);
  let opt = FindOptions::builder()
    .sort(doc! { "be_info.block_id": -1 })
    .skip(offset)
    .build();
  let mut filter = doc! { "be_info": doc! {"$exists": true} };
  if last_block_id.is_some() {
    filter.insert("be_info.block_id", doc! { "$lte": last_block_id.unwrap() });
  }
  if proposer.is_some() {
    filter.insert("proposer", &proposer.unwrap());
  }
  if epoch.is_some() {
    filter.insert("be_info.epoch", epoch.unwrap());
  }
  let mut blocks_cursor = ctx.db.blocks
    .find(filter)
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = blocks_cursor.next().await {
    results.push(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?);
  }
  Ok(HttpResponse::Ok().json(results))
}

#[get("/block/by-{by}/{id}")]
async fn get_block(path: web::Path<(String, String)>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let (by, id) = path.into_inner();
  let filter = match by.as_str() {
    "id" =>
      doc! { "be_info.block_id": id.parse::<i32>().map_err(|_| RespErr::BadRequest { msg: String::from("Invalid block number") })? },
    "cid" => doc! { "block": id },
    "slot" =>
      doc! { "slot_height": id.parse::<i32>().map_err(|_| RespErr::BadRequest { msg: String::from("Invalid slot height") })? },
    _ => {
      return Err(RespErr::BadRequest { msg: String::from("Invalid by clause") });
    }
  };
  let epoch = ctx.db.blocks.find_one(filter).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match epoch {
    Some(block) => { Ok(HttpResponse::Ok().json(block)) }
    None => Ok(HttpResponse::NotFound().json(json!({"error": "Block not found"}))),
  }
}

#[get("/tx/{trx_id}/output")]
async fn get_tx_output(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let trx_id = path.into_inner();
  if trx_id.len() == 40 {
    let tx = ctx.http_client
      .get(format!("{}/hafah-api/transactions/{}", config.hive_rpc.clone(), &trx_id))
      .send().await
      .map_err(|e| RespErr::InternalErr { msg: e.to_string() })?;
    if tx.status() == reqwest::StatusCode::BAD_REQUEST {
      return Err(RespErr::BadRequest { msg: String::from("transaction does not exist") });
    }
    let tx = tx.json::<TxByHash<Value>>().await.unwrap();
    let mut result: Vec<Option<Value>> = Vec::new();
    for i in 0..tx.transaction_json.operations.len() {
      let o = tx.transaction_json.operations[i].clone();
      if o.r#type == "custom_json_operation" {
        let op = serde_json::from_value::<CustomJson>(o.value).unwrap();
        if &op.id == "vsc.produce_block" {
          let block = ctx.db.blocks.find_one(doc! { "id": &trx_id }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
          result.push(Some(serde_json::to_value(block).unwrap()));
        } else if &op.id == "vsc.create_contract" {
          let contract = ctx.db.contracts
            .find_one(doc! { "tx_id": &trx_id }).await
            .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
          result.push(Some(serde_json::to_value(contract).unwrap()));
        } else if &op.id == "vsc.election_result" {
          let election = ctx.db.elections
            .find_one(doc! { "tx_id": &trx_id }).await
            .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
          result.push(Some(serde_json::to_value(election).unwrap()));
        } else {
          result.push(None);
        }
      } else {
        result.push(None);
      }
    }
    Ok(HttpResponse::Ok().json(result))
  } else {
    Err(RespErr::InternalErr { msg: String::from("L2 transaction outputs are currently WIP") })
  }
}

#[get("/bridge/stats")]
async fn bridge_stats(ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let stats = ctx.db.bridge_stats.find_one(doc! { "_id": 0 }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match stats {
    Some(s) => Ok(HttpResponse::Ok().json(s)),
    None => Ok(HttpResponse::Ok().json(BridgeStats { deposits: 0, withdrawals: 0 })),
  }
}

#[get("/address/{addr}/stats")]
async fn addr_stats(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let user = path.into_inner();
  let txs = ctx.db.tx_pool
    .count_documents(doc! { "$or": [{"required_auths": &user }, {"required_posting_auths": &user}, {"data.to": &user}] }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let ledger_txs = ctx.db.ledger
    .count_documents(doc! { "$or": [{"from": &user }, {"owner": &user}] }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let ledger_actions = ctx.db.ledger_actions
    .count_documents(doc! { "to": &user }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let deposits = ctx.db.ledger
    .count_documents(doc! { "$or": [{"from": &user }, {"owner": &user}], "t": "deposit" }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let withdrawals = ctx.db.ledger_actions
    .count_documents(doc! { "to": &user, "type": "withdraw" }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(UserStats { txs, ledger_txs, ledger_actions, deposits, withdrawals }))
}

#[get("/search/{query}")]
async fn search(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let query = path.into_inner();
  let block = ctx.db.blocks.find_one(doc! { "block": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if block.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "block", "result": &query})));
  }
  let election = ctx.db.elections.find_one(doc! { "data": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if election.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "election", "result": election.unwrap().epoch})));
  }
  let contract = ctx.db.contracts.find_one(doc! { "id": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if contract.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "contract", "result": &query})));
  }
  let tx = ctx.db.tx_pool.find_one(doc! { "id": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if tx.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "tx", "result": &query})));
  }
  Ok(HttpResponse::Ok().json(json!({"type": "", "result": ""})))
}
