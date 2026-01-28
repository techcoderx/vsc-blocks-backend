use std::ops::{ Div, Sub };

use crate::{
  config::{ config, DiscordConf },
  constants::NetworkConsts,
  helpers::db::{ get_props, get_user_balance, get_user_cons_unstaking, get_witness, get_witness_stats },
  mongo::MongoDB,
  types::{ hive::DgpAtBlock, vsc::ElectionMember },
};
use log::info;
use formatter::thousand_separator;
use mongodb::bson::doc;
use tokio;
use poise::{ serenity_prelude::{ self, CreateEmbed, Timestamp }, CreateReply };

struct Data {
  pub consts: NetworkConsts,
  pub db: MongoDB,
  pub http_client: reqwest::Client,
} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

fn time(timestamp: String, style: char) -> String {
  let mut ts_str = timestamp.clone();
  if !ts_str.ends_with('Z') {
    ts_str.push('Z');
  }
  format!("<t:{}:{style}>", Timestamp::parse(&ts_str).unwrap().timestamp())
}

fn l1_be_block_route(net_name: String) -> String {
  format!("{}", match net_name.as_str() {
    "mainnet" => "b",
    "testnet" => "block",
    _ => "",
  })
}

#[poise::command(
  slash_command,
  name_localized("en-US", "stats"),
  description_localized("en-US", "Retrieve Magi network general stats")
)]
async fn stats(ctx: Context<'_>) -> Result<(), Error> {
  ctx.defer().await?;
  let props = get_props(&ctx.data().db).await?;
  let embed = CreateEmbed::new()
    .title("Magi Network Info")
    .url(&ctx.data().consts.magi_explorer_url)
    .fields(
      vec![
        ("Hive Block Height", thousand_separator(props.last_processed_block), true),
        ("Magi Block Height", thousand_separator(props.l2_block_height), true),
        ("Transactions", thousand_separator(props.transactions), true),
        ("Epoch", thousand_separator(props.epoch), true),
        ("Witnesses", thousand_separator(props.witnesses), true),
        ("Contracts", thousand_separator(props.contracts), true)
      ]
    )
    .timestamp(Timestamp::now());
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[poise::command(slash_command, name_localized("en-US", "witness"), description_localized("en-US", "Retrieve Magi witness info"))]
async fn witness(
  ctx: Context<'_>,
  #[description = "L1 account username"] #[min_length = 3] #[max_length = 16] username: String
) -> Result<(), Error> {
  ctx.defer().await?;
  let wit_info = get_witness(&ctx.data().db, username.clone()).await?;
  if wit_info.is_none() {
    ctx.reply(format!("Witness {} does not exist.", username)).await?;
    return Ok(());
  }
  let stats = get_witness_stats(&ctx.data().db, username.clone()).await?;
  let wit = wit_info.unwrap();
  let magi_be_url = ctx.data().consts.magi_explorer_url.clone();
  let embed = CreateEmbed::new()
    .title("Magi Witness Info")
    .url(format!("{}/address/hive:{}/witness", magi_be_url, username.clone()))
    .fields(
      vec![
        ("Username", username, true),
        ("Enabled", wit.enabled.to_string(), true),
        ("Last Update", time(wit.ts.clone(), 'f'), true),
        ("Blocks Produced", thousand_separator(stats.block_count.unwrap_or(0)), true),
        ("Elections Held", thousand_separator(stats.election_count.unwrap_or(0)), true),
        (
          "Last Block",
          stats.last_block
            .map(|v| format!("[{}]({}/block/{})", thousand_separator(v), magi_be_url, v))
            .unwrap_or(String::from("N/A")),
          true,
        ),
        (
          "Last Epoch",
          stats.last_epoch
            .map(|v| format!("[{}]({}/epoch/{})", thousand_separator(v), magi_be_url, v))
            .unwrap_or(String::from("N/A")),
          true,
        )
      ]
    )
    .timestamp(Timestamp::now());
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[poise::command(
  slash_command,
  name_localized("en-US", "epoch"),
  description_localized("en-US", "Retrieve election results of a Magi epoch")
)]
async fn epoch(ctx: Context<'_>, #[description = "Magi epoch number"] #[min = 0] epoch_num: u32) -> Result<(), Error> {
  ctx.defer().await?;
  let epoch = ctx.data().db.elections.find_one(doc! { "epoch": epoch_num as i32 }).await?;
  if epoch.is_none() {
    ctx.reply(format!("Epoch {} does not exist.", epoch_num)).await?;
    return Ok(());
  }
  let epoch = epoch.unwrap();
  let mut members = epoch.members.clone();
  if members.len() > 40 {
    members.truncate(40);
    members.push(ElectionMember { account: format!("..."), key: format!("") });
  }
  let magi_be_url = ctx.data().consts.magi_explorer_url.clone();
  let embed = CreateEmbed::new()
    .title("Magi Epoch")
    .url(format!("{}/epoch/{}", magi_be_url, epoch_num))
    .fields(
      vec![
        ("Epoch", thousand_separator(epoch_num), true),
        (
          "Timestamp",
          epoch.be_info
            .clone()
            .map(|d| time(d.ts, 'f'))
            .unwrap_or(String::from("*Indexing...*")),
          true,
        ),
        (
          "L1 Block",
          format!(
            "[{}]({}/{}/{})",
            thousand_separator(epoch.block_height),
            &ctx.data().consts.l1_explorer_url,
            l1_be_block_route(ctx.data().consts.name.clone()),
            epoch.block_height
          ),
          true,
        ),
        (
          &format!("Elected Members ({})", epoch.members.len()),
          members
            .iter()
            .map(|m| m.account.clone())
            .collect::<Vec<String>>()
            .join(", "),
          false,
        ),
        ("Election Data CID", epoch.data, false),
        ("Proposed in Tx", format!("[{}]({}/tx/{})", epoch.tx_id, magi_be_url, epoch.tx_id), false),
        ("Proposer", epoch.proposer, true),
        (
          "Participation",
          match epoch_num {
            0 => String::from("N/A"),
            _ =>
              epoch.be_info
                .clone()
                .map(|d| format!("{:.2}%", (100.0 * (d.voted_weight as f64)) / (d.eligible_weight as f64)))
                .unwrap_or(String::from("*Indexing...*")),
          },
          true,
        )
      ]
    );
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[poise::command(slash_command, name_localized("en-US", "block"), description_localized("en-US", "Retrieve a Magi block"))]
async fn block(ctx: Context<'_>, #[description = "Magi block number"] #[min = 1] block_num: u32) -> Result<(), Error> {
  ctx.defer().await?;
  let block = ctx.data().db.blocks.find_one(doc! { "be_info.block_id": block_num as i32 }).await?;
  if block.is_none() {
    ctx.reply(format!("Block {} does not exist.", block_num)).await?;
    return Ok(());
  }
  let block = block.unwrap();
  let magi_be_url = ctx.data().consts.magi_explorer_url.clone();
  let embed = CreateEmbed::new()
    .title("Magi Block")
    .url(format!("{}/block/{}", magi_be_url, block_num))
    .fields(
      vec![
        ("Block Number", thousand_separator(block_num), true),
        ("Timestamp", time(block.ts.clone(), 'f'), true),
        (
          "Slot Height",
          format!(
            "[{}]({}/{}/{})",
            thousand_separator(block.slot_height),
            &ctx.data().consts.l1_explorer_url,
            l1_be_block_route(ctx.data().consts.name.clone()),
            block.slot_height
          ),
          true,
        ),
        ("Block CID", block.block, false),
        ("Proposed In Tx", format!("[{}]({}/tx/{})", block.id, magi_be_url, block.id), false),
        ("Proposer", block.proposer, true),
        (
          "Participation",
          block.be_info
            .map(|d| format!("{:.2}%", (100.0 * (d.voted_weight as f64)) / (d.eligible_weight as f64)))
            .unwrap_or(String::from("*Indexing...*")),
          true,
        )
      ]
    )
    .timestamp(Timestamp::now());
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[poise::command(
  slash_command,
  name_localized("en-US", "balance"),
  description_localized("en-US", "Retrieve address balance on Magi")
)]
async fn balance(
  ctx: Context<'_>,
  #[description = "Address"] #[min_length = 5] #[max_length = 150] address: String
) -> Result<(), Error> {
  if !address.starts_with("hive:") && !address.starts_with("did:") {
    let reply = CreateReply::default().ephemeral(true).content("Address must start with hive: or did:");
    ctx.send(reply).await?;
    return Ok(());
  }
  ctx.defer().await?;
  let bal = get_user_balance(&ctx.data().db, address.clone()).await?;
  let cons_unstaking = get_user_cons_unstaking(&ctx.data().db, address.clone()).await?;
  let magi_be_url = ctx.data().consts.magi_explorer_url.clone();
  let embed = CreateEmbed::new()
    .title("Magi Address Balance")
    .url(format!("{}/address/{}", magi_be_url, &address))
    .fields(
      vec![
        ("Address", &address, false),
        ("HIVE", &thousand_separator((bal.hive as f64).div(1000.0)), true),
        ("HBD", &thousand_separator((bal.hbd as f64).div(1000.0)), true),
        ("Staked HBD", &thousand_separator((bal.hbd_savings as f64).div(1000.0)), true),
        ("Consensus Stake", &format!("{} HIVE", thousand_separator((bal.hive_consensus as f64).div(1000.0))), true),
        ("Consensus Unstaking", &format!("{} HIVE", thousand_separator((cons_unstaking as f64).div(1000.0))), true)
      ]
    )
    .timestamp(Timestamp::now());
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[poise::command(slash_command, name_localized("en-US", "tx"), description_localized("en-US", "Retrieve a Magi transaction"))]
async fn tx(
  ctx: Context<'_>,
  #[description = "Transaction ID"] #[min_length = 40] #[max_length = 100] tx_id: String
) -> Result<(), Error> {
  ctx.defer().await?;
  let trx = ctx.data().db.tx_pool.find_one(doc! { "id": &tx_id }).await?;
  if trx.is_none() {
    ctx.reply(format!("Transaction {} does not exist.", tx_id)).await?;
    return Ok(());
  }
  let trx = trx.unwrap();
  let status_text = match trx.status.as_str() {
    "PENDING" => "Pending :hourglass_flowing_sand:",
    "INCLUDED" => "Included :hourglass_flowing_sand:",
    "CONFIRMED" => "Confirmed :white_check_mark:",
    "FAILED" => "Failed :x:",
    _ => ":thinking: Unknown",
  };
  let signers_text = match trx.required_auths.len() {
    0 => String::from("*None*"),
    1 => trx.required_auths[0].clone(),
    _ => format!("{} *(+{})*", trx.required_auths[0], trx.required_auths.len().sub(1)),
  };
  let tx_type_text = match trx.ops.len() {
    0 => String::from("*None*"),
    1 => trx.ops[0].clone().r#type,
    _ => String::from("*Multiple*"),
  };
  let dgp_req = ctx
    .data()
    .http_client.get(format!("{}/hafah-api/global-state?block-num={}", config.hive_rpc, trx.anchored_height))
    .send().await?;
  let dgp = dgp_req.json::<DgpAtBlock>().await?;
  let magi_be_url = ctx.data().consts.magi_explorer_url.clone();
  let embed = CreateEmbed::new()
    .title("Magi Transaction")
    .url(format!("{}/tx/{}", magi_be_url, tx_id))
    .fields(
      vec![
        ("Transaction ID", tx_id, false),
        ("Timestamp", time(dgp.created_at.clone(), 'f'), true),
        ("L1 Block", thousand_separator(trx.anchored_height), true),
        ("Position In Block", thousand_separator(trx.anchored_index), true),
        ("Type", tx_type_text, true),
        ("Signers", signers_text, true),
        ("Status", String::from(status_text), true)
      ]
    )
    .timestamp(Timestamp::now());
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[derive(Clone)]
pub struct DiscordBot {
  pub conf: DiscordConf,
  pub consts: NetworkConsts,
  pub db: MongoDB,
  pub http_client: reqwest::Client,
}

impl DiscordBot {
  pub fn init(conf: &DiscordConf, consts: &NetworkConsts, db: &MongoDB, http_client: &reqwest::Client) -> DiscordBot {
    return DiscordBot { conf: conf.clone(), consts: consts.clone(), db: db.clone(), http_client: http_client.clone() };
  }

  pub fn start(&self) {
    let token = self.conf.token.clone();
    let intents = serenity_prelude::GatewayIntents::non_privileged();
    let consts = self.consts.clone();
    let db = self.db.clone();
    let http_client = self.http_client.clone();
    let framework = poise::Framework
      ::builder()
      .options(poise::FrameworkOptions {
        commands: vec![stats(), witness(), epoch(), block(), balance(), tx()],
        ..Default::default()
      })
      .setup(|ctx, _ready, framework| {
        Box::pin(async move {
          // Uncomment to delete global commands once
          // you will need to import poise::serenity_prelude::CacheHttp
          // let http = ctx.http();
          // let global_commands = http.get_global_commands().await?;
          // for cmd in global_commands {
          //   http.delete_global_command(cmd.id.into()).await?;
          // }
          poise::builtins::register_globally(ctx, &framework.options().commands).await?;
          Ok(Data { consts, db, http_client })
        })
      })
      .build();
    tokio::spawn(async move {
      info!("Starting Discord bot");
      let client = serenity_prelude::ClientBuilder::new(token, intents).framework(framework).await;
      client.unwrap().start().await.unwrap();
    });
  }
}
