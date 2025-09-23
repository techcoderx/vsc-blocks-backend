use serde::Deserialize;
use serde_json::{ self, json };
use actix_web::{ middleware::NormalizePath, test, web, App };
use std::{ collections::HashSet, env, time::{ Instant, Duration } };
use vsc_blocks_backend::{
  compiler::Compiler,
  config::{ CompilerConf, GoCompilerConf },
  endpoints::cv_api,
  mongo::MongoDB,
  types::{ server::Context, vsc::Contract },
};

#[derive(Clone, Deserialize)]
struct SuccessResp {
  pub error: Option<String>,
  pub success: Option<bool>,
}

#[derive(Clone, Deserialize)]
struct CVResp {
  pub error: Option<String>,
  pub address: Option<String>,
  // pub code: Option<String>,
  // pub similar_match: Option<String>,
  // pub request_ts: Option<String>,
  pub verified_ts: Option<String>,
  pub status: Option<String>,
  pub repo_name: Option<String>,
  // pub repo_branch: Option<String>,
  pub git_commit: Option<String>,
  pub tinygo_version: Option<String>,
  // pub go_version: Option<String>,
  // pub llvm_version: Option<String>,
  // pub strip_tool: Option<String>,
  pub exports: Option<Vec<String>>,
  // pub license: Option<String>,
  pub lang: Option<String>,
}

async fn setup_db() -> MongoDB {
  // connect and drop existing db
  let db = MongoDB::init(String::from("mongodb://127.0.0.1:27017")).await.expect("failed to setup mongodb database");
  db.contracts.drop().await.expect("failed to drop contracts collection");
  db.cv_contracts.drop().await.expect("failed to drop cv contracts collection");
  // db.setup().await.expect("Failed to setup cv database");

  // insert example contract
  let contract_data: Contract = serde_json
    ::from_str(include_str!("contracts/hello-world.json"))
    .expect("Failed to parse contract json");
  let contract2_data: Contract = serde_json
    ::from_str(include_str!("contracts/hello-world-striped.json"))
    .expect("Failed to parse contract 2 json");
  db.contracts.insert_one(contract_data).await.expect("Failed to insert contract");
  db.contracts.insert_one(contract2_data).await.expect("Failed to insert contract 2");
  return db;
}

#[actix_web::test]
async fn test_e2e_verify_contract_go() {
  let db = setup_db().await;
  let http_client = reqwest::Client::new();
  let src_dir = env
    ::var("VSC_CV_TEST_SRC_DIR")
    .unwrap_or_else(|_| String::from("/Users/techcoderx/vsc-blocks-backend/go_compiler"));
  let out_dir = env
    ::var("VSC_CV_TEST_OUT_DIR")
    .unwrap_or_else(|_| String::from("/Users/techcoderx/vsc-blocks-backend/artifacts"));
  let compiler = Compiler::init(
    &db,
    &http_client,
    &(GoCompilerConf {
      src_dir: src_dir,
      src_host_dir: None,
      output_dir: out_dir,
      output_host_dir: None,
      timeout: 20,
    }),
    &(CompilerConf {
      enabled: Some(true),
      github_api_key: env
        ::var("VSC_CV_TEST_GITHUB_KEY")
        .map(|k| Some(k))
        .unwrap_or(None),
      wasm_strip: format!("/usr/local/bin/wasm-strip"),
      wasm_tools: format!("/usr/local/bin/wasm-tools"),
      whitelist: Vec::new(),
    })
  );
  let server_ctx = Context { db: db, compiler: Some(compiler), http_client: http_client.clone() };
  let app = test::init_service(
    App::new()
      .wrap(NormalizePath::trim())
      .app_data(web::Data::new(server_ctx.clone()))
      .service(
        web
          ::scope("/cv-api/v1")
          .service(cv_api::hello)
          .service(cv_api::login)
          .service(cv_api::verify_new)
          .service(cv_api::contract_info)
      )
  ).await;

  // hello world contract deployed at https://vsc.techcoderx.com/contract/vsc1Bem8RnoLgGPP7E2MBN52ekrdVqy2LNpSqF
  let contract_id = "vsc1Bem8RnoLgGPP7E2MBN52ekrdVqy2LNpSqF";
  let req_verify_new = test::TestRequest
    ::post()
    .set_json(
      json!({
        "repo_url": "https://github.com/techcoderx/go-contract-template",
        "repo_branch": "wip2",
        "tinygo_version": "0.38.0",
      })
    )
    .uri(format!("/cv-api/v1/verify/{}/new", contract_id).as_str())
    .to_request();
  let resp: SuccessResp = test::call_and_read_body_json(&app, req_verify_new).await;
  assert_eq!(resp.error, None);
  assert_eq!(resp.success.unwrap(), true);

  let req_get_inserted_contract = test::TestRequest
    ::get()
    .uri(format!("/cv-api/v1/contract/{}", contract_id).as_str())
    .to_request();
  let resp: CVResp = test::call_and_read_body_json(&app, req_get_inserted_contract).await;
  assert_eq!(resp.error, None);
  assert_eq!(&resp.address.unwrap(), contract_id);
  assert_eq!(&resp.status.unwrap(), "queued");
  assert_eq!(&resp.lang.unwrap(), "go");
  assert_eq!(&resp.tinygo_version.unwrap(), "0.38.0");
  assert_eq!(&resp.repo_name.unwrap(), "techcoderx/go-contract-template");

  // Poll contract status until success
  let mut seen_statuses = HashSet::new();
  let start_time = Instant::now();
  let timeout = Duration::from_secs(30);
  let poll_interval = Duration::from_millis(1000);

  loop {
    // Get current contract status
    let req_status = test::TestRequest::get().uri(format!("/cv-api/v1/contract/{}", contract_id).as_str()).to_request();
    let resp: CVResp = test::call_and_read_body_json(&app, req_status).await;

    let current_status = resp.status.unwrap();
    seen_statuses.insert(current_status.clone());

    // Check for success
    if current_status == "success" {
      assert_eq!(resp.exports.is_some(), true);
      assert_eq!(resp.verified_ts.is_some(), true);
      assert_eq!(resp.git_commit.is_some(), true);

      let exports = resp.exports.unwrap();
      assert_eq!(exports.len(), 2);
      assert_eq!(exports.contains(&String::from("entrypoint")), true);
      assert_eq!(exports.contains(&String::from("hello_world")), true);
      break;
    }

    // Check for failure states
    assert_ne!(current_status, "failed", "Contract verification failed");

    // Check timeout
    if start_time.elapsed() > timeout {
      panic!("Timeout waiting for verification to complete");
    }

    tokio::time::sleep(poll_interval).await;
  }

  assert!(seen_statuses.contains("success"), "Must end with success status");

  // hello world striped contract deployed at https://vsc.techcoderx.com/contract/vsc1BmW9jQdUAXsQXC9zQkAsfDcYq2KBadaZiG
  let contract_id = "vsc1BmW9jQdUAXsQXC9zQkAsfDcYq2KBadaZiG";
  let req_verify_new = test::TestRequest
    ::post()
    .set_json(
      json!({
        "repo_url": "https://github.com/techcoderx/go-contract-template",
        "repo_branch": "wip2",
        "tinygo_version": "0.38.0",
        "strip_tool": "wabt",
      })
    )
    .uri(format!("/cv-api/v1/verify/{}/new", contract_id).as_str())
    .to_request();
  let resp: SuccessResp = test::call_and_read_body_json(&app, req_verify_new).await;
  assert_eq!(resp.error, None);
  assert_eq!(resp.success.unwrap(), true);

  let req_get_inserted_contract = test::TestRequest
    ::get()
    .uri(format!("/cv-api/v1/contract/{}", contract_id).as_str())
    .to_request();
  let resp: CVResp = test::call_and_read_body_json(&app, req_get_inserted_contract).await;
  assert_eq!(resp.error, None);
  assert_eq!(&resp.address.unwrap(), contract_id);
  assert_eq!(&resp.status.unwrap(), "queued");
  assert_eq!(&resp.lang.unwrap(), "go");
  assert_eq!(&resp.tinygo_version.unwrap(), "0.38.0");
  assert_eq!(&resp.repo_name.unwrap(), "techcoderx/go-contract-template");

  // Poll contract status until success
  let mut seen_statuses = HashSet::new();
  let start_time = Instant::now();
  let timeout = Duration::from_secs(30);
  let poll_interval = Duration::from_millis(1000);

  loop {
    // Get current contract status
    let req_status = test::TestRequest::get().uri(format!("/cv-api/v1/contract/{}", contract_id).as_str()).to_request();
    let resp: CVResp = test::call_and_read_body_json(&app, req_status).await;

    let current_status = resp.status.unwrap();
    seen_statuses.insert(current_status.clone());

    // Check for success
    if current_status == "success" {
      assert_eq!(resp.exports.is_some(), true);
      assert_eq!(resp.verified_ts.is_some(), true);
      assert_eq!(resp.git_commit.is_some(), true);

      let exports = resp.exports.unwrap();
      assert_eq!(exports.len(), 2);
      assert_eq!(exports.contains(&String::from("entrypoint")), true);
      assert_eq!(exports.contains(&String::from("hello_world")), true);
      break;
    }

    // Check for failure states
    assert_ne!(current_status, "failed", "Contract verification failed");

    // Check timeout
    if start_time.elapsed() > timeout {
      panic!("Timeout waiting for verification to complete");
    }

    tokio::time::sleep(poll_interval).await;
  }
}
