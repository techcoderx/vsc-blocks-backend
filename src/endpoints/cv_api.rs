use actix_web::{ get, post, web, HttpRequest, HttpResponse, Responder };
use actix_multipart::form::{ tempfile::TempFile, MultipartForm, text::Text };
use futures_util::StreamExt;
use mongodb::bson::{ doc, DateTime };
use serde::{ Serialize, Deserialize };
use serde_json::{ json, Number, Value };
use semver::VersionReq;
use chrono::{ Utc, Duration };
use regex::Regex;
use hex;
use sha2::{ Sha256, Digest };
use jsonwebtoken::{ Header, EncodingKey, DecodingKey, Algorithm, Validation, errors::ErrorKind };
use log::{ error, debug };
use std::io::Read;
use crate::{
  config::config,
  constants::*,
  types::{ cv::{ CVContract, CVContractCode, CVStatus }, hive::{ DgpAtBlock, JsonRpcResp }, server::{ Context, RespErr } },
};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6f";

#[get("")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
  user: String,
  app: String,
  network: String,
  iat: i64, // Issued at (timestamp)
  exp: i64, // Expiration time (timestamp)
}

fn verify_auth_token(req: &HttpRequest) -> Result<String, RespErr> {
  if config.auth.enabled {
    if let Some(auth_header) = req.clone().headers().get("Authorization") {
      let auth_value = auth_header.to_str().unwrap_or("");
      let parts = auth_value.split(" ").collect::<Vec<&str>>();
      debug!("Authentication header: {}", auth_value);
      if parts.len() < 2 || parts[0] != "Bearer" {
        return Err(RespErr::TokenMissing);
      }
      let mut validation = Validation::new(Algorithm::HS256);
      validation.validate_exp = true;
      validation.leeway = 0;
      let claims = (match
        jsonwebtoken::decode::<Claims>(
          parts[1],
          &DecodingKey::from_secret(hex::decode(config.auth.key.clone().unwrap()).unwrap().as_slice()),
          &validation
        )
      {
        Ok(token_data) => {
          // Additional manual checks if needed
          let now = Utc::now().timestamp();

          // Verify iat is in the past
          if token_data.claims.iat > now {
            return Err(RespErr::TokenExpired);
          }

          Ok(token_data.claims)
        }
        Err(err) =>
          match err.kind() {
            ErrorKind::ExpiredSignature => Err(RespErr::TokenExpired),
            _ => Err(RespErr::TokenInvalid),
          }
      })?;
      return Ok(claims.user);
    } else {
      return Err(RespErr::TokenMissing);
    }
  }
  Ok(String::from(""))
}

#[post("/login")]
async fn login(payload: String, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  if !config.auth.enabled {
    return Ok(HttpResponse::NotFound().json(json!({"error": "Auth is disabled"})));
  }
  let parts: Vec<&str> = payload.split(":").collect();
  if parts.len() != 6 || parts[1] != &config.auth.id.clone().unwrap() || parts[2] != "hive" {
    return Err(RespErr::BadRequest { msg: String::from("Invalid auth message format") });
  }
  let block_num = parts[3].parse::<u64>();
  if block_num.is_err() {
    return Err(RespErr::BadRequest { msg: String::from("Could not parse block number") });
  }
  let block_num = block_num.unwrap();
  let original = (&parts[0..5]).join(":");
  let mut hasher = Sha256::new();
  hasher.update(&original);
  let hash = hex::encode(&hasher.finalize()[..]);
  let verify_req = ctx.http_client
    .post(config.hive_rpc.clone())
    .json::<Value>(
      &json!({
    "id": 1,
    "jsonrpc": "2.0",
    "method": "database_api.verify_signatures",
    "params": {
      "hash": &hash,
      "signatures": [parts[5]],
      "required_owner": [],
      "required_active": [],
      "required_posting": [parts[0]],
      "required_other": []
  }
  })
    )
    .send().await
    .map_err(|_| RespErr::SigVerifyReqFail)?
    .json::<JsonRpcResp>().await
    .map_err(|_| RespErr::SigVerifyReqFail)?;
  let is_valid =
    !verify_req.error.is_some() && verify_req.result.is_some() && verify_req.result.unwrap().clone()["valid"].as_bool().unwrap();
  if !is_valid {
    return Err(RespErr::SigVerifyFail);
  }
  let head_block_num = ctx.http_client
    .get(config.hive_rpc.clone() + "/hafah-api/headblock")
    .send().await
    .map_err(|_| RespErr::SigRecentBlkReqFail)?
    .json::<Number>().await
    .map_err(|_| RespErr::SigRecentBlkReqFail)?;
  if head_block_num.as_u64().unwrap() > block_num + config.auth.timeout_blocks.unwrap_or(20) {
    return Err(RespErr::SigTooOld);
  }
  let dgp_at_block = ctx.http_client
    .get(config.hive_rpc.clone() + "/hafah-api/global-state?block-num=" + &block_num.to_string())
    .send().await
    .map_err(|_| RespErr::SigRecentBlkReqFail)?
    .json::<DgpAtBlock>().await
    .map_err(|_| RespErr::SigRecentBlkReqFail)?;
  if &dgp_at_block.hash != parts[4] {
    return Err(RespErr::SigBhNotMatch);
  }

  // generate jwt
  let now = Utc::now();
  let iat = now.timestamp();
  let exp = (now + Duration::hours(1)).timestamp();
  let claims = Claims {
    user: String::from(parts[0]),
    app: config.auth.id.clone().unwrap(),
    network: String::from("hive"),
    iat,
    exp,
  };
  let decoded_secret = hex::decode(config.auth.key.clone().unwrap()).map_err(|_| RespErr::TokenGenFail)?;
  let token = jsonwebtoken
    ::encode(&Header::default(), &claims, &EncodingKey::from_secret(&decoded_secret))
    .map_err(|_| RespErr::TokenGenFail)?;
  Ok(HttpResponse::Ok().json(json!({ "access_token": token })))
}

#[derive(Serialize, Deserialize)]
struct ReqVerifyNew {
  license: String,
  lang: String,
  dependencies: Value,
}

#[post("/verify/{address}/new")]
async fn verify_new(
  req: HttpRequest,
  path: web::Path<String>,
  req_data: web::Json<ReqVerifyNew>,
  ctx: web::Data<Context>
) -> Result<HttpResponse, RespErr> {
  let username = verify_auth_token(&req)?;
  let address = path.into_inner();
  let contract = ctx.db.contracts.find_one(doc! { "id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if contract.is_none() {
    return Ok(HttpResponse::NotFound().json(json!({"error": "contract not found"})));
  }
  let contract = contract.unwrap();

  // license check
  let valid_license = ctx.db.cv_licenses
    .find_one(doc! { "name": &req_data.license }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if valid_license.is_none() {
    return Err(RespErr::BadRequest { msg: format!("License {} is not supported.", &req_data.license) });
  }

  // existing contract verification status check
  match ctx.db.cv_contracts.find_one(doc! { "_id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })? {
    Some(cv) => {
      if cv.status != "pending" && cv.status != "failed" && cv.status != "not match" {
        return Err(RespErr::BadRequest { msg: String::from("Contract is already verified or being verified.") });
      }
    }
    None => (),
  }

  // check required dependencies
  match req_data.lang.as_str() {
    "assemblyscript" => {
      if !req_data.dependencies.is_object() {
        return Err(RespErr::BadRequest { msg: String::from("Dependencies must be an object") });
      }
      let test_utils = req_data.dependencies.get(ASC_TEST_UTILS_NAME);
      let sdk = req_data.dependencies.get(ASC_SDK_NAME);
      let assemblyscript = req_data.dependencies.get(ASC_NAME);
      let assemblyscript_json = req_data.dependencies.get(ASC_JSON_NAME);
      if test_utils.is_none() || sdk.is_none() || assemblyscript.is_none() || assemblyscript_json.is_none() {
        return Err(RespErr::BadRequest {
          msg: format!(
            "The following dependencies are required: {}, {}, {}, {}",
            ASC_TEST_UTILS_NAME,
            ASC_SDK_NAME,
            ASC_NAME,
            ASC_JSON_NAME
          ),
        });
      }
      if let Value::Object(map) = &req_data.dependencies {
        // Iterate over the keys and values in the map
        for (key, val) in map.iter() {
          if !val.is_string() {
            return Err(RespErr::BadRequest { msg: String::from("Dependency versions must be strings") });
          }
          VersionReq::parse(val.as_str().unwrap()).map_err(|e| RespErr::BadRequest {
            msg: format!("Invalid semver for dependency {}: {}", key, e.to_string()),
          })?;
        }
      }
    }
    _ => {
      return Err(RespErr::BadRequest { msg: String::from("Language is currently unsupported") });
    }
  }
  // clear already uploaded source codes when the previous ones failed verification
  ctx.db.cv_source_codes.delete_many(doc! { "addr": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  ctx.db.cv_contracts.delete_one(doc! { "_id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let new_cv = CVContract {
    id: address.clone(),
    bytecode_cid: contract.code.clone(),
    username: Some(username.clone()),
    request_ts: DateTime::from_chrono(Utc::now()),
    verified_ts: None,
    status: CVStatus::Pending.to_string(),
    exports: None,
    license: req_data.license.clone(),
    lang: req_data.lang.clone(),
    dependencies: Some(req_data.dependencies.clone()),
  };
  ctx.db.cv_contracts.insert_one(new_cv).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(json!({ "success": true })))
}

#[derive(Debug, MultipartForm)]
struct VerifUploadForm {
  #[multipart(limit = "1MB")]
  file: TempFile,
  filename: Text<String>,
}

#[post("/verify/{address}/upload")]
async fn upload_file(
  path: web::Path<String>,
  req: HttpRequest,
  MultipartForm(mut form): MultipartForm<VerifUploadForm>,
  ctx: web::Data<Context>
) -> Result<HttpResponse, RespErr> {
  verify_auth_token(&req)?;
  let address = path.into_inner();
  debug!("Uploaded file {} with size: {}", form.file.file_name.unwrap(), form.file.size);
  debug!("Contract address {}, new filename: {}", &address, &form.filename.0);
  if form.file.size > 1024 * 1024 {
    return Err(RespErr::BadRequest { msg: String::from("Uploaded file size exceeds 1MB limit") });
  }
  let mut contents = String::new();
  match form.file.file.read_to_string(&mut contents) {
    Ok(_) => (),
    Err(e) => {
      error!("Failed to read uploaded file: {}", e.to_string());
      return Err(RespErr::BadRequest {
        msg: String::from("Failed to process uploaded file, most likely file is not in UTF-8 format."),
      });
    }
  }
  let fname_regex = Regex::new(r"^[A-Za-z0-9._-]+$").expect("Invalid regex pattern");
  if form.filename.0.len() > 50 {
    return Err(RespErr::BadRequest { msg: String::from("Filename length must be less than 50 characters") });
  } else if !fname_regex.is_match(&form.filename.0) {
    return Err(RespErr::BadRequest { msg: String::from("Invalid filename") });
  }
  match ctx.db.cv_contracts.find_one(doc! { "_id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })? {
    Some(cv) => {
      if cv.status != "pending" {
        return Err(RespErr::BadRequest { msg: format!("Status needs to be pending, it is currently {}", cv.status) });
      } else if cv.lang == "assemblyscript" && (&form.filename.0 == "pnpm-lock.yml" || &form.filename.0 == "pnpm-lock.yaml") {
        return Err(RespErr::BadRequest { msg: String::from("pnpm-lock.yaml is a reserved filename for pnpm lock files.") });
      }
    }
    None => {
      return Err(RespErr::BadRequest { msg: String::from("Begin contract verification with /verify/new first.") });
    }
  }
  let new_file = CVContractCode {
    addr: address.clone(),
    fname: form.filename.0.clone(),
    is_lockfile: false,
    content: contents.clone(),
  };
  ctx.db.cv_source_codes.insert_one(new_file).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(json!({ "success": true })))
}

#[post("/verify/{address}/complete")]
async fn upload_complete(path: web::Path<String>, req: HttpRequest, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  verify_auth_token(&req)?;
  let address = path.into_inner();
  match ctx.db.cv_contracts.find_one(doc! { "_id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })? {
    Some(cv) => {
      if cv.status != "pending" {
        return Err(RespErr::BadRequest { msg: String::from("Status is currently not pending upload") });
      }
    }
    None => {
      return Err(RespErr::BadRequest { msg: String::from("Contract does not exist") });
    }
  }
  let file_count = ctx.db.cv_source_codes
    .count_documents(doc! { "addr": &address }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if file_count < 1 {
    return Err(RespErr::BadRequest { msg: String::from("No source files were uploaded for this contract") });
  }
  ctx.db.cv_contracts
    .update_one(doc! { "_id": &address }, doc! { "$set": doc! {"status": CVStatus::Queued.to_string()} }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  ctx.compiler.notify();
  Ok(HttpResponse::Ok().json(json!({ "success": true })))
}

#[get("/languages")]
async fn list_langs() -> Result<HttpResponse, RespErr> {
  Ok(HttpResponse::Ok().json(vec!["assemblyscript", "golang"]))
}

#[get("/licenses")]
async fn list_licenses(ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let result = ctx.db.cv_licenses.distinct("name", doc! {}).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let result_arr: Vec<&str> = result
    .iter()
    .filter_map(|bson| bson.as_str())
    .collect();
  Ok(HttpResponse::Ok().json(result_arr))
}

#[get("/contract/{address}")]
async fn contract_info(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let addr = path.into_inner();
  let contract = ctx.db.cv_contracts.find_one(doc! { "_id": &addr }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if contract.is_none() {
    return Ok(HttpResponse::NotFound().json(json!({"error": "contract not found"})));
  }
  let contract = contract.unwrap();
  let files = ctx.db.cv_source_codes
    .distinct("fname", doc! { "addr": &addr, "is_lockfile": false }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let files_arr: Vec<&str> = files
    .iter()
    .filter_map(|bson| bson.as_str())
    .collect();
  let lockfilename = ctx.db.cv_source_codes
    .find_one(doc! { "addr": &addr, "is_lockfile": true }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    .map(|f| f.fname);
  let result =
    json!({
    "address": &addr,
    "code": contract.bytecode_cid.clone(),
    "username": contract.username.clone(),
    "request_ts": contract.request_ts.to_chrono().format(TIMESTAMP_FORMAT).to_string(),
    "verified_ts": contract.verified_ts.map(|t| t.to_chrono().format(TIMESTAMP_FORMAT).to_string()),
    "status": contract.status.clone(),
    "exports": contract.exports,
    "files": files_arr.clone(),
    "lockfile": lockfilename,
    "license": contract.license.clone(),
    "lang": contract.lang.clone(),
    "dependencies": contract.dependencies.clone()
  });
  Ok(HttpResponse::Ok().json(result))
}

#[get("/contract/{address}/files/ls")]
async fn contract_files_ls(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let addr = path.into_inner();
  let files = ctx.db.cv_source_codes
    .distinct("fname", doc! { "addr": &addr, "is_lockfile": false }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let files_arr: Vec<&str> = files
    .iter()
    .filter_map(|bson| bson.as_str())
    .collect();
  Ok(HttpResponse::Ok().json(files_arr))
}

#[get("/contract/{address}/files/cat/{filename}")]
async fn contract_files_cat(path: web::Path<(String, String)>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let (addr, filename) = path.into_inner();
  match
    ctx.db.cv_source_codes
      .find_one(doc! { "addr": &addr, "fname": &filename }).await
      .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
  {
    Some(file) => Ok(HttpResponse::Ok().body(file.content)),
    None => Ok(HttpResponse::NotFound().body("Error 404 file not found")),
  }
}

#[get("/contract/{address}/files/catall")]
async fn contract_files_cat_all(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let addr = path.into_inner();
  let mut files_cursor = ctx.db.cv_source_codes
    .find(doc! { "addr": &addr }).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let mut results = Vec::new();
  while let Some(f) = files_cursor.next().await {
    let file = f.map_err(|_| RespErr::InternalErr { msg: String::from("Failed to parse file") })?;
    results.push(json!({"name": file.fname, "content": file.content}));
  }
  Ok(HttpResponse::Ok().json(results))
}
