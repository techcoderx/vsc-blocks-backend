use actix_web::{ web, middleware::NormalizePath, App, HttpServer };
use actix_cors::Cors;
use clap::Parser;
use reqwest;
use env_logger;
use std::{ process, str::FromStr };
use log::{ error, info, LevelFilter };
mod config;
mod constants;
mod mongo;
mod types;
mod endpoints;
mod indexer;
mod compiler;
mod chatbot;
mod helpers;
use types::server::Context;
use endpoints::{ be_api, cv_api };

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  if config::Args::parse().dump_config {
    config::TomlConfig::dump_config_file();
  }
  let config = &config::config;
  let mut log_builder = env_logger::Builder::new();
  log_builder
    .filter(
      None,
      LevelFilter::from_str(
        std::env
          ::var("RUST_LOG")
          .unwrap_or(config.log_level.clone().unwrap_or(String::from("info")))
          .as_str()
      ).unwrap()
    )
    .filter_module("tracing", LevelFilter::Off)
    .filter_module("serenity", LevelFilter::Off)
    .filter_module("reqwest", LevelFilter::Off)
    .filter_module("h2", LevelFilter::Off)
    .filter_module("rustls", LevelFilter::Off)
    .filter_module("hyper", LevelFilter::Off)
    .filter_module("tungstenite", LevelFilter::Off)
    .default_format()
    .init();
  info!("Version: {}", env!("CARGO_PKG_VERSION"));
  let db = match mongo::MongoDB::init(&config.db).await {
    Ok(d) => d,
    Err(e) => {
      error!("Failed to initialize database: {}", e.to_string());
      process::exit(1);
    }
  };
  let http_client = reqwest::Client::new();
  let compiler = match
    config.compiler
      .clone()
      .map(|f| f.enabled.unwrap_or(false))
      .unwrap_or(false)
  {
    true => {
      let ccf = config.compiler.clone().unwrap();
      if ccf.github_api_key.is_none() {
        error!("Missing Github API key for compiler");
        process::exit(1);
      }
      Some(compiler::Compiler::init(&db, &http_client, &config.gocompiler, &config.compiler.clone().unwrap()))
    }
    false => None,
  };
  if config.be_indexer.unwrap_or(false) {
    let idxer = indexer::indexer::Indexer::init(&http_client, &db);
    idxer.start();
  }
  if config.discord.is_some() {
    let consts = constants::from_config();
    let discord_bot = chatbot::discord::DiscordBot::init(&config.discord.clone().unwrap(), &consts, &db, &http_client);
    discord_bot.start();
  }
  let server_ctx = Context { db, compiler, http_client: http_client.clone() };
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
          .service(cv_api::contract_info)
          .service(cv_api::gocompiler_versions)
      )
      .service(
        web
          ::scope("/be-api/v1")
          .service(be_api::hello)
          .service(be_api::props)
          .service(be_api::witness_stats)
          .service(be_api::get_active_witness_stats)
          .service(be_api::list_epochs)
          .service(be_api::get_epoch)
          .service(be_api::list_blocks)
          .service(be_api::get_block)
          .service(be_api::get_tx_output)
          .service(be_api::bridge_stats)
          .service(be_api::addr_stat)
          .service(be_api::addr_stats)
          .service(be_api::search)
          .service(be_api::network_stats)
      )
  })
    .bind((config.server.address.as_str(), config.server.port))?
    .run().await
}
