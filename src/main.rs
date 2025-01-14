use actix_web::{ App, HttpServer };
use env_logger;
mod config;
mod server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let config = &config::Config;
  if std::env::var("RUST_LOG").is_err() {
    std::env::set_var("RUST_LOG", config.log_level.clone().unwrap_or(String::from("info")));
  }
  env_logger::init();
  HttpServer::new(|| { App::new().service(server::hello).service(server::echo).service(server::hey) })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
