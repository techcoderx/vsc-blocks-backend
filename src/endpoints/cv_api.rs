use actix_web::{ get, post, web, HttpRequest, HttpResponse, Responder };
use mongodb::bson::{ doc, DateTime };
use serde::{ Serialize, Deserialize };
use serde_json::{ json, Number, Value };
use chrono::{ Utc, Duration };
use regex::Regex;
use hex;
use sha2::{ Digest, Sha256 };
use jsonwebtoken::{ Header, EncodingKey, DecodingKey, Algorithm, Validation, errors::ErrorKind };
use utoipa::{ OpenApi, ToSchema };
use log::debug;
use crate::{
  config::config,
  types::{
    cv::{ CVContract, CVContractResult, CVStatus, tinygo_versions },
    hive::{ DgpAtBlock, JsonRpcResp },
    server::{ Context, ErrorRes, RespErr, SuccessRes },
  },
};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6f";

#[get("")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().json(OpenApiDoc::openapi())
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

#[derive(Serialize, Deserialize, ToSchema)]
struct ReqVerifyNew {
  /// Link to GitHub repository
  repo_url: String,
  /// Branch or tag that should be checked out. Default branch will be used if not specified.
  repo_branch: Option<String>,
  /// Tinygo version
  tinygo_version: String,
  /// Tool used to strip output WASM file. Valid values: `wabt` or `wasm-tools`.
  strip_tool: Option<String>,
}

#[utoipa::path(
  post,
  path = "/verify/{address}/new",
  summary = "Create a new contract verification request",
  description = "Create a new contract verification request from a GitHub repository.",
  responses(
    (status = 200, description = "Contract verification request created successfully", body = SuccessRes),
    (status = 302, description = "Another contract with exact bytecode was already verified", body = ErrorRes),
    (status = 400, description = "Failed to create contract verification request", body = ErrorRes),
    (status = 404, description = "Contract or GitHub repository does not exist", body = ErrorRes)
  ),
  params(("address" = String, Path, description = "Contract address to verify")),
  request_body = ReqVerifyNew
)]
#[post("/verify/{address}/new")]
async fn verify_new(
  req: HttpRequest,
  path: web::Path<String>,
  req_data: web::Json<ReqVerifyNew>,
  ctx: web::Data<Context>
) -> Result<HttpResponse, RespErr> {
  if ctx.compiler.is_none() {
    return Err(RespErr::CvDisabled);
  }
  let username = verify_auth_token(&req)?;
  let address = path.into_inner();
  let contract = ctx.db.contracts.find_one(doc! { "id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if contract.is_none() {
    return Err(RespErr::ContractNotFound);
  }
  let contract = contract.unwrap();

  // only creator or owner can request verification if authentication enabled, or whitelisted users
  if config.auth.enabled {
    let whitelist = config.compiler.clone().expect("compiler config should be present").whitelist.clone();
    if !whitelist.is_empty() && whitelist.contains(&username) {
      return Err(RespErr::CvNotWhitelisted);
    } else if &username != &contract.creator && &username != &contract.owner {
      return Err(RespErr::CvNotAuthorized);
    }
  }

  // existing contract verification status check
  match ctx.db.cv_contracts.find_one(doc! { "_id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })? {
    Some(cv) => {
      let is_fail = cv.status == "failed" || cv.status == "not match";
      if cv.status != "pending" && !is_fail {
        return Err(RespErr::BadRequest { msg: String::from("Contract is already verified or being verified.") });
      } else if is_fail && cv.request_ts.to_chrono() + Duration::hours(12) > Utc::now() {
        return Err(RespErr::CvRetryLater);
      }
    }
    None => (),
  }
  // other contracts identical bytecode that have been previously verified
  match ctx.db.cv_contracts.find_one(doc! { "code": &contract.code }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })? {
    Some(similar) => {
      if similar.status == CVStatus::Success.to_string() {
        return Err(RespErr::CvSimilarMatch);
      }
    }
    None => (),
  }

  if &contract.runtime.value != "go" {
    return Err(RespErr::BadRequest { msg: String::from("Language is currently unsupported") });
  }
  let repo_url = req_data.repo_url.clone();
  if !repo_url.starts_with("https://github.com/") {
    return Err(RespErr::CvInvalidGitHubURL);
  }
  let repo = repo_url.replace("https://github.com/", "");
  let repo_id = repo.split("/").collect::<Vec<&str>>();
  let repo_branch = req_data.repo_branch.clone().unwrap_or(format!(""));
  if repo_id.len() != 2 || repo_id[0].len() > 39 || repo_id[1].len() > 100 {
    return Err(RespErr::CvInvalidGitHubURL);
  }
  let github_user_regex: Regex = Regex::new(r"^[A-Za-z0-9-]+$").expect("Invalid regex pattern");
  let github_repo_regex: Regex = Regex::new(r"^[A-Za-z0-9._-]+$").expect("Invalid regex pattern");
  if !github_user_regex.is_match(repo_id[0]) || !github_repo_regex.is_match(repo_id[1]) {
    return Err(RespErr::CvInvalidGitHubURL);
  }
  if repo_branch.len() > 255 {
    return Err(RespErr::CvInvalidGitBranch);
  }
  match req_data.strip_tool.clone() {
    Some(tool) => {
      if &tool != "wabt" && &tool != "wasm-tools" {
        return Err(RespErr::CvInvalidWasmStripTool);
      }
    }
    None => (),
  }
  if !tinygo_versions.contains_key(&req_data.tinygo_version) {
    return Err(RespErr::CvInvalidTinyGoVersion);
  }
  let tinygo_libs = tinygo_versions.get(&req_data.tinygo_version).unwrap().clone();
  ctx.db.cv_contracts.delete_one(doc! { "_id": &address }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let new_cv = CVContract {
    id: address.clone(),
    code: contract.code.clone(),
    verifier: match username.len() {
      0 => None,
      _ => Some(username),
    },
    request_ts: DateTime::from_chrono(Utc::now()),
    verified_ts: None,
    status: CVStatus::Queued.to_string(),
    repo_name: repo,
    repo_branch: repo_branch,
    git_commit: None,
    tinygo_version: req_data.tinygo_version.clone(),
    go_version: tinygo_libs.go,
    llvm_version: tinygo_libs.llvm,
    strip_tool: req_data.strip_tool.clone(),
    exports: None,
    license: None,
    lang: contract.runtime.value.clone(),
  };
  ctx.db.cv_contracts.insert_one(new_cv).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  ctx.compiler.clone().unwrap().notify();
  Ok(HttpResponse::Ok().json(SuccessRes { success: true }))
}

#[utoipa::path(
  get,
  path = "/contract/{address}",
  summary = "Lookup a contract's verification details",
  responses(
    (status = 200, description = "Contract verification details", body = CVContractResult),
    (status = 400, description = "Failed to query contract verification", body = ErrorRes),
    (status = 404, description = "Contract verification not found", body = ErrorRes)
  ),
  params(("address" = String, Path, description = "Contract address"))
)]
#[get("/contract/{address}")]
async fn contract_info(path: web::Path<String>, ctx: web::Data<Context>) -> Result<HttpResponse, RespErr> {
  let addr = path.into_inner();
  let contract = ctx.db.cv_contracts.find_one(doc! { "_id": &addr }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  if contract.is_some() {
    let contract = contract.unwrap();
    let result = CVContractResult {
      address: addr,
      code: contract.code,
      similar_match: None,
      verifier: contract.verifier,
      request_ts: contract.request_ts.to_chrono().format(TIMESTAMP_FORMAT).to_string(),
      verified_ts: contract.verified_ts.map(|t| t.to_chrono().format(TIMESTAMP_FORMAT).to_string()),
      status: contract.status.clone(),
      repo_name: contract.repo_name,
      repo_branch: contract.repo_branch,
      git_commit: contract.git_commit,
      tinygo_version: contract.tinygo_version,
      go_version: contract.go_version,
      llvm_version: contract.llvm_version,
      strip_tool: contract.strip_tool,
      exports: contract.exports,
      license: contract.license,
      lang: contract.lang.clone(),
    };
    return Ok(HttpResponse::Ok().json(result));
  }
  let deployed_contract = match
    ctx.db.contracts.find_one(doc! { "id": &addr }).await.map_err(|e| RespErr::DbErr { msg: e.to_string() })?
  {
    Some(c) => c,
    None => {
      return Err(RespErr::ContractNotFound);
    }
  };
  match
    ctx.db.cv_contracts
      .find_one(doc! { "code": &deployed_contract.code }).await
      .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
  {
    Some(similar) => {
      if similar.status == CVStatus::Success.to_string() {
        return Ok(
          HttpResponse::Ok().json(CVContractResult {
            address: addr,
            code: similar.code,
            similar_match: Some(similar.id),
            verifier: similar.verifier,
            request_ts: similar.request_ts.to_chrono().format(TIMESTAMP_FORMAT).to_string(),
            verified_ts: similar.verified_ts.map(|t| t.to_chrono().format(TIMESTAMP_FORMAT).to_string()),
            status: similar.status.clone(),
            repo_name: similar.repo_name,
            repo_branch: similar.repo_branch,
            git_commit: similar.git_commit,
            tinygo_version: similar.tinygo_version,
            go_version: similar.go_version,
            llvm_version: similar.llvm_version,
            strip_tool: similar.strip_tool,
            exports: similar.exports,
            license: similar.license,
            lang: similar.lang.clone(),
          })
        );
      }
    }
    None => (),
  }
  Err(RespErr::ContractNotFound)
}

#[derive(OpenApi)]
#[openapi(
  info(
    title = "VSC Contract Verifier",
    description = "Verifies VSC contracts by compiling the uploaded contract source code and comparing the resulting output bytecode against the deployed contract bytecode.",
    license(name = "MIT")
  ),
  paths(verify_new, contract_info),
  components(responses(ErrorRes, SuccessRes))
)]
struct OpenApiDoc;
