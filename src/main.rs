use actix_web::{ web, middleware::NormalizePath, App, HttpServer };
use actix_cors::Cors;
use clap::Parser;
use reqwest;
use env_logger;
use std::process;
use log::{ error, info };
mod config;
mod constants;
mod db;
mod mongo;
mod types;
mod endpoints;
mod indexer;
mod compiler;
use types::server::Context;
use endpoints::{ be_api, cv_api };

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  if config::Args::parse().dump_config {
    config::TomlConfig::dump_config_file();
  }
  let config = &config::config;
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", config.log_level.clone().unwrap_or(String::from("info")));
  }
  env_logger::init();
  info!("Version: {}", env!("CARGO_PKG_VERSION"));
  let db_pool = match db::DbPool::init(config.psql_url.clone()) {
    Ok(p) => p,
    Err(e) => {
      error!("Failed to initialize db pool: {}", e.to_string());
      process::exit(1);
    }
  };
  match db_pool.setup().await {
    Ok(_) => (),
    Err(e) => {
      error!("Failed to setup db: {}", e.to_string());
      process::exit(1);
    }
  }
  let vsc_db = match mongo::MongoDB::init(config.mongo_url.clone()).await {
    Ok(d) => d,
    Err(e) => {
      error!("Failed to initialize VSC db pool: {}", e.to_string());
      process::exit(1);
    }
  };
  let compiler = compiler::Compiler::init(&db_pool);
  compiler.notify();
  let http_client = reqwest::Client::new();
  if config.be_indexer.unwrap_or(false) {
    let l2_block_indexer = indexer::blocks::BlockIndexer::init(
      http_client.clone(),
      vsc_db.blocks.clone(),
      vsc_db.elections.clone(),
      vsc_db.indexer2.clone()
    );
    l2_block_indexer.start();
  }
  let server_ctx = Context { db: db_pool, vsc_db, compiler, http_client: http_client.clone() };
  HttpServer::new(move || {
    let cors = Cors::default().allow_any_origin().allow_any_method().allow_any_header().max_age(3600);
    App::new()
      .wrap(cors)
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
      .service(
        web
          ::scope("/be-api/v1")
          .service(be_api::hello)
          .service(be_api::props)
          .service(be_api::list_witnesses)
          .service(be_api::get_witness)
          .service(be_api::get_balance)
          .service(be_api::list_epochs)
          .service(be_api::get_epoch)
          .service(be_api::list_blocks)
          .service(be_api::get_block)
          .service(be_api::get_block_by_cid)
          .service(be_api::get_block_by_slot)
          .service(be_api::get_blocks_in_epoch)
          .service(be_api::get_tx_output)
          .service(be_api::list_contracts)
          .service(be_api::get_contract)
          .service(be_api::search)
      )
  })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
