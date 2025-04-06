use actix_web::{ get, web, HttpResponse, Responder };
use futures_util::StreamExt;
use mongodb::{ bson::doc, options::{ FindOneOptions, FindOptions } };
use serde::Deserialize;
use serde_json::json;
use std::cmp::{ min, max };
use crate::{
  config::config,
  endpoints::inference::{ combine_inferred_epoch, infer_epoch },
  server_types::{ Context, RespErr },
  vsc_types::{ HafProps, LedgerBalance },
};

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
  let contracts = ctx.vsc_db.contracts.estimated_document_count().await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let epoch = ctx.vsc_db.elections.estimated_document_count().await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let block_count = ctx.vsc_db.blocks.estimated_document_count().await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let last_l1_block = match
    ctx.vsc_db.l1_blocks.find_one(doc! { "type": "metadata" }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?
  {
    Some(state) => state.head_height,
    None => 0,
  };
  let l1_ops = match config.haf_url.clone() {
    Some(haf_url) => {
      match ctx.http_client.get(format!("{}/be-api/v1/haf", haf_url)).send().await {
        Ok(req) => req.json::<HafProps>().await.unwrap_or(HafProps { operations: 0 }).operations,
        Err(_) => 0,
      }
    }
    None => 0,
  };
  let tx_count = ctx.vsc_db.tx_pool.estimated_document_count().await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(
    HttpResponse::Ok().json(
      json!({
        "last_processed_block": last_l1_block,
        "l2_block_height": block_count,
        "witnesses": witness_count,
        "epoch": epoch.saturating_sub(1),
        "contracts": contracts,
        "operations": l1_ops,
        "transactions": tx_count
      })
    )
  )
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
    results.push(
      serde_json
        ::to_value(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?)
        .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    );
  }
  Ok(HttpResponse::Ok().json(results))
}

#[get("/witness/{username}")]
async fn get_witness(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let user = path.into_inner();
  let opt = FindOneOptions::builder()
    .sort(doc! { "height": -1 })
    .build();
  match
    ctx.vsc_db.witnesses
      .find_one(doc! { "account": &user })
      .with_options(opt).await
      .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
  {
    Some(wit) => Ok(HttpResponse::Ok().json(wit)),
    None => Ok(HttpResponse::NotFound().json(json!({"error": "witness does not exist"}))),
  }
}

#[get("/balance/{username}")]
async fn get_balance(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let user = path.into_inner(); // must be prefixed by hive: or did: (!)
  let opt = FindOneOptions::builder()
    .sort(doc! { "block_height": -1 })
    .build();
  match
    ctx.vsc_db.balances
      .find_one(doc! { "account": &user })
      .with_options(opt).await
      .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
  {
    Some(bal) => Ok(HttpResponse::Ok().json(bal)),
    None =>
      Ok(
        HttpResponse::NotFound().json(LedgerBalance {
          account: user,
          block_height: 0,
          hbd: 0,
          hbd_avg: 0,
          hbd_modify: 0,
          hbd_savings: 0,
          hive: 0,
          hive_consensus: 0,
        })
      ),
  }
}

#[derive(Debug, Deserialize)]
struct ListEpochOpts {
  last_epoch: Option<i64>,
  count: Option<i64>,
}

#[get("/epochs")]
async fn list_epochs(params: web::Query<ListEpochOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let last_epoch = params.last_epoch;
  let count = min(1, max(params.count.unwrap_or(100), 100));
  let opt = FindOptions::builder()
    .sort(doc! { "epoch": -1 })
    .build();
  let filter = match last_epoch {
    Some(le) => doc! { "epoch": doc! {"$lte": le} },
    None => doc! {},
  };
  let mut epochs_cursor = ctx.vsc_db.elections
    .find(filter)
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = epochs_cursor.next().await {
    let doc = doc.unwrap();
    let inferred = infer_epoch(&ctx.http_client, &ctx.vsc_db.elections2, &doc).await.map_err(|e| RespErr::InternalErr {
      msg: e.to_string(),
    })?;
    results.push(combine_inferred_epoch(&doc, &inferred));
  }
  Ok(HttpResponse::Ok().json(results))
}

#[get("/epoch/{epoch}")]
async fn get_epoch(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let epoch_num = path
    .into_inner()
    .parse::<i32>()
    .map_err(|_| RespErr::BadRequest { msg: String::from("Invalid epoch number") })?;
  let epoch = ctx.vsc_db.elections
    .find_one(doc! { "epoch": epoch_num }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match epoch {
    Some(ep) => {
      let inferred = infer_epoch(&ctx.http_client, &ctx.vsc_db.elections2, &ep).await.map_err(|e| RespErr::InternalErr {
        msg: e.to_string(),
      })?;
      Ok(HttpResponse::Ok().json(combine_inferred_epoch(&ep, &inferred)))
    }
    None => Ok(HttpResponse::NotFound().json(json!({"error": "epoch does not exist"}))),
  }
}
