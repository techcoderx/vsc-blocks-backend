use actix_web::{ get, web, HttpResponse, Responder };
use futures_util::StreamExt;
use mongodb::bson::doc;
use serde_json::json;
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
  Ok(HttpResponse::Ok().json(json!({
    "witnesses": witness_count
  })))
}

#[get("/witnesses")]
async fn list_witnesses(ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let pipeline = vec![
    // Sort by account (ascending) and height (descending)
    doc! {
      "$sort": doc! {
        "account": 1,
        "height": -1
      }
    },
    // Group by account and keep first document (highest height)
    doc! {
      "$group": {
        "_id": "$account",
        "doc": { "$first": "$$ROOT" }
      }
    },
    // Restore original document structure
    doc! {
      "$replaceRoot": {
        "newRoot": "$doc"
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
