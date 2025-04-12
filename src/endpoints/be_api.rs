use actix_web::{ get, web, HttpResponse, Responder };
use futures_util::StreamExt;
use mongodb::{ bson::{ doc, Bson }, options::{ FindOneOptions, FindOptions } };
use serde::Deserialize;
use serde_json::{ json, Value };
use std::cmp::{ min, max };
use crate::{
  config::config,
  types::{ hive::{ CustomJson, TxByHash }, server::{ Context, RespErr }, vsc::{ LedgerBalance, RcUsedAtHeight } },
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
  let tx_count = ctx.vsc_db.tx_pool.estimated_document_count().await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(
    HttpResponse::Ok().json(
      json!({
        "last_processed_block": last_l1_block,
        "l2_block_height": block_count,
        "witnesses": witness_count,
        "epoch": epoch.saturating_sub(1),
        "contracts": contracts,
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
    results.push(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?);
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
  let mut bal = ctx.vsc_db.balances
    .find_one(doc! { "account": user.clone() })
    .with_options(opt.clone()).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    .unwrap_or(LedgerBalance {
      account: user.clone(),
      block_height: 0,
      hbd: 0,
      hbd_avg: 0,
      hbd_modify: 0,
      hbd_savings: 0,
      hive: 0,
      hive_consensus: 0,
      hive_unstaking: None,
      rc_used: None,
    });
  bal.rc_used = Some(
    ctx.vsc_db.rc
      .find_one(doc! { "account": user.clone() })
      .with_options(opt.clone()).await
      .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
      .unwrap_or(RcUsedAtHeight {
        block_height: 0,
        amount: 0,
      })
  );
  let unstaking_pipeline = vec![
    doc! {
      "$match": doc! {
        "to": user.clone(),
        "status": "pending",
        "type": "consensus_unstake"
      }
    },
    doc! {
      "$group": doc! {
        "_id": Bson::Null,
        "totalAmount": doc! {"$sum": "$amount"}
      }
    }
  ];
  let mut unstaking_cursor = ctx.vsc_db.ledger_actions
    .aggregate(unstaking_pipeline).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  bal.hive_unstaking = Some(
    unstaking_cursor
      .next().await
      .transpose()
      .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
      .map(|d| d.get_i64("totalAmount").unwrap_or(0))
      .unwrap_or(0)
  );
  Ok(HttpResponse::Ok().json(bal))
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
  let mut epochs_cursor = ctx.vsc_db.elections
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
  let epoch = ctx.vsc_db.elections
    .find_one(doc! { "epoch": epoch_num }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match epoch {
    Some(ep) => Ok(HttpResponse::Ok().json(ep)),
    None => Ok(HttpResponse::NotFound().json(json!({"error": "Epoch does not exist"}))),
  }
}

#[derive(Debug, Deserialize)]
struct ListBlockOpts {
  last_block_id: Option<i64>,
  count: Option<i64>,
  proposer: Option<String>,
  epoch: Option<i64>,
}

#[get("/blocks")]
async fn list_blocks(params: web::Query<ListBlockOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let last_block_id = params.last_block_id;
  let proposer = params.proposer.clone();
  let epoch = params.epoch;
  let count = min(max(1, params.count.unwrap_or(100)), 100);
  let opt = FindOptions::builder()
    .sort(doc! { "be_info.block_id": -1 })
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
  let mut blocks_cursor = ctx.vsc_db.blocks
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
  let epoch = ctx.vsc_db.blocks.find_one(filter).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
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
    for o in tx.transaction_json.operations {
      if o.r#type == "custom_json_operation" {
        let op = serde_json::from_value::<CustomJson>(o.value).unwrap();
        if &op.id == "vsc.produce_block" {
          let block = ctx.vsc_db.blocks
            .find_one(doc! { "id": &trx_id }).await
            .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
          result.push(Some(serde_json::to_value(block).unwrap()));
        } else if
          &op.id == "vsc.call" ||
          &op.id == "vsc.transfer" ||
          &op.id == "vsc.withdraw" ||
          &op.id == "vsc.consensus_stake" ||
          &op.id == "vsc.consensus_unstake" ||
          &op.id == "vsc.stake_hbd" ||
          &op.id == "vsc.unstake_hbd"
        {
          let tx_out = ctx.vsc_db.tx_pool
            .find_one(doc! { "id": &trx_id }).await
            .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
          result.push(Some(serde_json::to_value(tx_out).unwrap()));
        } else if &op.id == "vsc.create_contract" {
          let contract = ctx.vsc_db.contracts
            .find_one(doc! { "tx_id": &trx_id }).await
            .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
          result.push(Some(serde_json::to_value(contract).unwrap()));
        } else if &op.id == "vsc.election_result" {
          let election = ctx.vsc_db.elections
            .find_one(doc! { "be_info.trx_id": &trx_id }).await
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

#[derive(Deserialize)]
struct ListContractsOpts {
  count: Option<i64>,
}

#[get("/contracts")]
async fn list_contracts(params: web::Query<ListContractsOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let count = min(max(1, params.count.unwrap_or(100)), 200);
  let opt = FindOptions::builder()
    .sort(doc! { "creation_height": -1 })
    .build();
  let mut contracts_cursor = ctx.vsc_db.contracts
    .find(doc! {})
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = contracts_cursor.next().await {
    results.push(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?);
  }
  Ok(HttpResponse::Ok().json(results))
}

#[get("/contract/{id}")]
async fn get_contract(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let id = path.into_inner();
  let contract = ctx.vsc_db.contracts.find_one(doc! { "id": &id }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match contract {
    Some(c) => { Ok(HttpResponse::Ok().json(c)) }
    None => Ok(HttpResponse::NotFound().json(json!({"error": "Contract does not exist"}))),
  }
}

#[get("/search/{query}")]
async fn search(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let query = path.into_inner();
  let block = ctx.vsc_db.blocks.find_one(doc! { "block": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if block.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "block", "result": &query})));
  }
  let election = ctx.vsc_db.elections.find_one(doc! { "data": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if election.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "election", "result": election.unwrap().epoch})));
  }
  let contract = ctx.vsc_db.contracts.find_one(doc! { "id": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if contract.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "contract", "result": &query})));
  }
  let tx = ctx.vsc_db.tx_pool.find_one(doc! { "id": &query }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if tx.is_some() {
    return Ok(HttpResponse::Ok().json(json!({"type": "tx", "result": &query})));
  }
  Ok(HttpResponse::Ok().json(json!({"type": "", "result": ""})))
}
