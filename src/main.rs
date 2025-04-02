use actix_web::{ web, middleware::NormalizePath, App, HttpServer };
use actix_cors::Cors;
use clap::Parser;
use reqwest;
use env_logger;
use std::{ process, path::Path };
use log::{ error, info, warn };
mod config;
mod constants;
mod db;
mod server_types;
mod endpoints;
mod vsc_types;
mod compiler;
use endpoints::cv_api;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  if config::Args::parse().dump_config {
    std::env::set_var("RUST_LOG", String::from("info"));
    env_logger::init();
    if !Path::new(&config::Args::parse().config_file).exists() {
      config::TomlConfig::dump_config_file();
      info!("Dumped sample config file to config.toml");
    } else {
      warn!("Config file already exists, doing nothing.");
    }
    process::exit(0);
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
  let compiler = compiler::Compiler::init(&db_pool);
  compiler.notify();
  let server_ctx = server_types::Context { db: db_pool, compiler, http_client: reqwest::Client::new() };
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
  })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
