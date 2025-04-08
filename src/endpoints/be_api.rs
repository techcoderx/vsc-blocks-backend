use actix_web::{ get, web, HttpResponse, Responder };
use futures_util::StreamExt;
use mongodb::{ bson::{ doc, Bson }, options::{ FindOneOptions, FindOptions } };
use serde::Deserialize;
use serde_json::{ json, Value };
use std::cmp::{ min, max };
use crate::{
  config::config,
  indexer::epoch::{ combine_inferred_epoch, infer_epoch },
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
}

#[get("/epochs")]
async fn list_epochs(params: web::Query<ListEpochOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let last_epoch = params.last_epoch;
  let count = max(min(1, params.count.unwrap_or(100)), 100);
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

#[derive(Debug, Deserialize)]
struct ListBlockOpts {
  last_block_id: Option<i64>,
  count: Option<i64>,
}

#[get("/blocks")]
async fn list_blocks(params: web::Query<ListBlockOpts>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let last_block_id = params.last_block_id;
  let count = max(min(1, params.count.unwrap_or(100)), 100);
  let opt = FindOptions::builder()
    .sort(doc! { "be_info.block_id": -1 })
    .build();
  let filter = match last_block_id {
    Some(lb) => doc! { "be_info": doc! {"$exists": true}, "be_info.block_id": doc! {"$lte": lb} },
    None => doc! { "be_info": doc! {"$exists": true} },
  };
  let mut blocks_cursor = ctx.vsc_db.blocks
    .find(filter)
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = blocks_cursor.next().await {
    results.push(
      serde_json
        ::to_value(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?)
        .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    );
  }
  Ok(HttpResponse::Ok().json(results))
}

#[get("/block/by-id/{id}")]
async fn get_block(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let block_num = path
    .into_inner()
    .parse::<i32>()
    .map_err(|_| RespErr::BadRequest { msg: String::from("Invalid block number") })?;
  let epoch = ctx.vsc_db.blocks
    .find_one(doc! { "be_info.block_id": block_num }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match epoch {
    Some(block) => { Ok(HttpResponse::Ok().json(block)) }
    None => Ok(HttpResponse::NotFound().json(json!({"error": "Block does not exist"}))),
  }
}

#[get("/block/by-cid/{cid}")]
async fn get_block_by_cid(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let block_cid = path.into_inner();
  let block = ctx.vsc_db.blocks.find_one(doc! { "block": block_cid }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  match block {
    Some(block) => { Ok(HttpResponse::Ok().json(block)) }
    None => Ok(HttpResponse::NotFound().json(json!({"error": "Block does not exist"}))),
  }
}

#[get("/block/in-epoch/{epoch}")]
async fn get_blocks_in_epoch(
  path: web::Path<String>,
  params: web::Query<ListBlockOpts>,
  ctx: web::Data<Context>
) -> Result<HttpResponse, RespErr> {
  let last_block_id = params.last_block_id;
  let count = max(min(1, params.count.unwrap_or(100)), 100);
  let epoch = path
    .into_inner()
    .parse::<i32>()
    .map_err(|_| RespErr::BadRequest { msg: String::from("Invalid epoch number") })?;
  let opt = FindOptions::builder()
    .sort(doc! { "be_info.block_id": -1 })
    .build();
  let filter = match last_block_id {
    Some(lb) => doc! { "be_info.epoch": epoch, "be_info.block_id": doc! {"$lte": lb} },
    None => doc! { "be_info.epoch": epoch },
  };
  let mut blocks_cursor = ctx.vsc_db.blocks
    .find(filter)
    .with_options(opt)
    .limit(count).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(doc) = blocks_cursor.next().await {
    results.push(
      serde_json
        ::to_value(doc.map_err(|e| RespErr::DbErr { msg: e.to_string() })?)
        .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    );
  }
  Ok(HttpResponse::Ok().json(results))
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
