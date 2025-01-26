use actix_web::{ web, App, HttpServer };
use env_logger;
use std::process;
use log::error;
mod config;
mod db;
mod server;
mod vsc_types;

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
  HttpServer::new(move || {
    App::new()
      .app_data(web::Data::new(db_pool.clone()))
      .service(server::hello)
      .service(server::verify_new)
      .service(server::upload_file)
      .service(server::list_langs)
      .service(server::list_licenses)
  })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
