use actix_web::{ web, App, HttpServer };
use reqwest;
use env_logger;
use std::process;
use log::error;
mod config;
mod constants;
mod db;
mod server;
mod vsc_types;
mod compiler;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let config = &config::config;
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", config.log_level.clone().unwrap_or(String::from("info")));
  }
  env_logger::init();
  let db_pool = match db::DbPool::init() {
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
  let server_ctx = server::Context { db: db_pool, compiler, http_client: reqwest::Client::new() };
  HttpServer::new(move || {
    App::new()
      .app_data(web::Data::new(server_ctx.clone()))
      .service(server::hello)
      .service(server::verify_new)
      .service(server::upload_file)
      .service(server::upload_complete)
      .service(server::list_langs)
      .service(server::list_licenses)
  })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
