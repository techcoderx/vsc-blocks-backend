use actix_web::{ web, App, HttpServer };
use env_logger;
mod config;
mod db;
mod server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let config = &config::Config;
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", config.log_level.clone().unwrap_or(String::from("info")));
  }
  env_logger::init();
  let db_pool = db::init_pool().unwrap();
  HttpServer::new(move || {
    App::new().app_data(web::Data::new(db_pool.clone())).service(server::hello).service(server::echo).service(server::hey)
  })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
