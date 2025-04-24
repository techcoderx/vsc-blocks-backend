use serde::Deserialize;
use serde_json::{ self, json, Value };
use actix_web::{ middleware::NormalizePath, test, web, App };
use std::env;
use vsc_blocks_backend::{
  compiler::Compiler,
  db::DbPool,
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
  // pub username: Option<String>,
  // pub request_ts: Option<String>,
  pub verified_ts: Option<String>,
  pub status: Option<String>,
  pub exports: Option<Vec<String>>,
  pub files: Option<Vec<String>>,
  pub lockfile: Option<String>,
  pub license: Option<String>,
  pub lang: Option<String>,
  pub dependencies: Option<Value>,
}

async fn setup_db() -> (DbPool, MongoDB) {
  // vsc cv db
  let db_pool = DbPool::init(String::from("postgres://postgres:mysecretpassword@127.0.0.1:5432/postgres")).expect(
    "failed to init postgres db pool"
  );
  db_pool.execute_file("DROP SCHEMA IF EXISTS vsc_cv CASCADE;").await.expect("failed to drop vsc_cv schema");
  db_pool.setup().await.expect("failed to setup postgres db pool");

  // vsc db
  let vsc_db = MongoDB::init(String::from("mongodb://127.0.0.1:27017")).await.expect("failed to setup mongodb database");
  vsc_db.contracts.drop().await.expect("failed to drop contracts collection");

  // insert example contract
  let contract_data: Contract = serde_json::from_str(include_str!("contract.json")).expect("Failed to parse contract.json");
  vsc_db.contracts.insert_one(contract_data).await.expect("Failed to insert contract");
  return (db_pool, vsc_db);
}

#[actix_web::test]
async fn test_e2e_verify_contract_assemblyscript() {
  let contract_id = "vs41q9c3yg82k4q76nprqk89xzk2n8zhvedrz5prnrrqpy6v9css3seg2q43uvjdc500";
  let dbs = setup_db().await;
  let http_client = reqwest::Client::new();
  let asc_dir = env
    ::var("VSC_CV_TEST_ASC_DIR")
    .unwrap_or_else(|_| String::from("/Users/techcoderx/vsc-blocks-backend/as_compiler"));
  let compiler = Compiler::init(&dbs.0, String::from("as-compiler"), asc_dir);
  compiler.notify();
  let server_ctx = Context { db: dbs.0, vsc_db: dbs.1, compiler: compiler.clone(), http_client: http_client.clone() };
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
          .service(cv_api::upload_file)
          .service(cv_api::upload_complete)
          .service(cv_api::list_langs)
          .service(cv_api::list_licenses)
          .service(cv_api::contract_info)
          .service(cv_api::contract_files_ls)
          .service(cv_api::contract_files_cat)
          .service(cv_api::contract_files_cat_all)
          .service(cv_api::bytecode_lookup_addr)
      )
  ).await;
  let req_verify_new = test::TestRequest
    ::post()
    .set_json(
      json!({
        "license": "MIT",
        "lang": "assemblyscript",
        "dependencies": {
          "@vsc.eco/sdk": "^0.1.4",
          "assemblyscript": "^0.27.31",
          "assemblyscript-json": "^1.1.0",
          "@vsc.eco/contract-testing-utils": "^0.1.4"
        }
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
  assert_eq!(&resp.status.unwrap(), "pending");
  assert_eq!(resp.dependencies.is_some(), true);
  assert_eq!(resp.files.is_some(), true);
  assert_eq!(resp.files.unwrap().len(), 0);
  assert_eq!(&resp.lang.unwrap(), "assemblyscript");
  assert_eq!(&resp.license.unwrap(), "MIT");

  // Test file upload
  let file_content = include_str!("../tests/index.ts");
  let payload =
    format!("--boundary\r\n\
    Content-Disposition: form-data; name=\"filename\"\r\n\r\n\
    index.ts\r\n\
    --boundary\r\n\
    Content-Disposition: form-data; name=\"file\"; filename=\"index.ts\"\r\n\
    Content-Type: application/octet-stream\r\n\r\n\
    {}\r\n\
    --boundary--\r\n", file_content);
  let req_upload = test::TestRequest
    ::post()
    .uri(format!("/cv-api/v1/verify/{}/upload", contract_id).as_str())
    .insert_header(("Content-Type", "multipart/form-data; boundary=boundary"))
    .set_payload(payload)
    .to_request();
  let resp: SuccessResp = test::call_and_read_body_json(&app, req_upload).await;
  assert_eq!(resp.error, None);
  assert_eq!(resp.success.unwrap(), true);

  // Verify file was added
  let req_get_contract = test::TestRequest::get().uri(format!("/cv-api/v1/contract/{}", contract_id).as_str()).to_request();
  let resp: CVResp = test::call_and_read_body_json(&app, req_get_contract).await;
  assert_eq!(resp.error, None);
  assert_eq!(resp.files.unwrap().len(), 1);

  // Call complete endpoint
  let req_complete = test::TestRequest::post().uri(format!("/cv-api/v1/verify/{}/complete", contract_id).as_str()).to_request();
  let resp: SuccessResp = test::call_and_read_body_json(&app, req_complete).await;
  assert_eq!(resp.error, None);
  assert_eq!(resp.success.unwrap(), true);

  // Poll contract status until success
  use std::collections::HashSet;
  use std::time::{ Instant, Duration };
  let mut seen_statuses = HashSet::new();
  let start_time = Instant::now();
  let timeout = Duration::from_secs(120);
  let poll_interval = Duration::from_millis(1000);

  loop {
    // Get current contract status
    let req_status = test::TestRequest::get().uri(format!("/cv-api/v1/contract/{}", contract_id).as_str()).to_request();
    let resp: CVResp = test::call_and_read_body_json(&app, req_status).await;

    let current_status = resp.status.unwrap();
    seen_statuses.insert(current_status.clone());

    // Check for success
    if current_status == "success" {
      // There should be one export named dumpEnv and a lockfile
      assert_eq!(resp.exports.is_some(), true);
      assert_eq!(resp.lockfile.is_some(), true);
      assert_eq!(resp.verified_ts.is_some(), true);

      let exports = resp.exports.unwrap();
      assert_eq!(exports.len(), 1);
      assert_eq!(exports.get(0).unwrap(), "dumpEnv");
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

  // Verify we saw expected status progression
  assert!(
    seen_statuses.contains("queued") || seen_statuses.contains("in progress"),
    "Should have seen at least one intermediate status"
  );
  assert!(seen_statuses.contains("success"), "Must end with success status");
}
