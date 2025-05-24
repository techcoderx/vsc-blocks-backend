use crate::{
  config::DiscordConf,
  constants::{ L1_EXPLORER_URL, VSC_BLOCKS_HOME },
  helpers::db::{ get_props, get_witness, get_witness_stats },
  mongo::MongoDB,
  types::vsc::ElectionMember,
};
use log::info;
use mongodb::bson::doc;
use tokio;
use poise::{ serenity_prelude::{ self, CreateEmbed, Timestamp }, CreateReply };
use num_format::{ Locale, ToFormattedString };

struct Data {
  pub db: MongoDB,
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

#[poise::command(
  slash_command,
  name_localized("en-US", "stats"),
  description_localized("en-US", "Retrieve VSC network general stats")
)]
async fn stats(ctx: Context<'_>) -> Result<(), Error> {
  ctx.defer().await?;
  let props = get_props(&ctx.data().db).await?;
  let embed = CreateEmbed::new()
    .title("VSC Network Info")
    .url(VSC_BLOCKS_HOME)
    .fields(
      vec![
        ("Hive Block Height", props.last_processed_block.to_formatted_string(&Locale::en), true),
        ("VSC Block Height", props.l2_block_height.to_formatted_string(&Locale::en), true),
        ("Transactions", props.transactions.to_formatted_string(&Locale::en), true)
      ]
    )
    .fields(
      vec![
        ("Epoch", props.epoch.to_formatted_string(&Locale::en), true),
        ("Witnesses", props.witnesses.to_formatted_string(&Locale::en), true),
        ("Contracts", props.contracts.to_formatted_string(&Locale::en), true)
      ]
    )
    .timestamp(Timestamp::now());
  let reply = CreateReply::default().embed(embed);
  ctx.send(reply).await?;
  Ok(())
}

#[poise::command(slash_command, name_localized("en-US", "witness"), description_localized("en-US", "Retrieve VSC witness info"))]
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
  let embed = CreateEmbed::new()
    .title("VSC Witness Info")
    .url(format!("{}/address/hive:{}/witness", VSC_BLOCKS_HOME, username.clone()))
    .fields(
      vec![
        ("Username", username, true),
        ("Enabled", wit.enabled.to_string(), true),
        ("Last Update", time(wit.ts.clone(), 'R'), true),
        ("Blocks Produced", stats.block_count.unwrap_or(0).to_formatted_string(&Locale::en), true),
        ("Elections Held", stats.election_count.unwrap_or(0).to_formatted_string(&Locale::en), true),
        (
          "Last Block",
          stats.last_block
            .map(|v| format!("[{}]({}/block/{})", v.to_formatted_string(&Locale::en), VSC_BLOCKS_HOME, v))
            .unwrap_or(String::from("N/A")),
          true,
        ),
        (
          "Last Epoch",
          stats.last_epoch
            .map(|v| format!("[{}]({}/epoch/{})", v.to_formatted_string(&Locale::en), VSC_BLOCKS_HOME, v))
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
  description_localized("en-US", "Retrieve election results of a VSC epoch")
)]
async fn epoch(ctx: Context<'_>, #[description = "VSC epoch number"] #[min = 0] epoch_num: u32) -> Result<(), Error> {
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
  let embed = CreateEmbed::new()
    .title("VSC Epoch")
    .url(format!("{}/epoch/{}", VSC_BLOCKS_HOME, epoch_num))
    .fields(
      vec![
        ("Epoch", epoch_num.to_formatted_string(&Locale::en), true),
        (
          "Timestamp",
          epoch.be_info
            .clone()
            .map(|d| time(d.ts, 'R'))
            .unwrap_or(String::from("*Indexing...*")),
          true,
        ),
        (
          "L1 Block",
          format!("[{}]({}/b/{})", epoch.block_height.to_formatted_string(&Locale::en), L1_EXPLORER_URL, epoch.block_height),
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
        ("Proposed in Tx", format!("[{}]({}/tx/{})", epoch.tx_id, VSC_BLOCKS_HOME, epoch.tx_id), false),
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

#[poise::command(slash_command, name_localized("en-US", "block"), description_localized("en-US", "Retrieve a VSC block"))]
async fn block(ctx: Context<'_>, #[description = "VSC block number"] #[min = 1] block_num: u32) -> Result<(), Error> {
  ctx.defer().await?;
  let block = ctx.data().db.blocks.find_one(doc! { "be_info.block_id": block_num as i32 }).await?;
  if block.is_none() {
    ctx.reply(format!("Block {} does not exist.", block_num)).await?;
    return Ok(());
  }
  let block = block.unwrap();
  let embed = CreateEmbed::new()
    .title("VSC Block")
    .url(format!("{}/block/{}", VSC_BLOCKS_HOME, block_num))
    .fields(
      vec![
        ("Block Number", block_num.to_formatted_string(&Locale::en), true),
        ("Timestamp", time(block.ts.clone(), 'R'), true),
        (
          "Slot Height",
          format!("[{}]({}/b/{})", block.slot_height.to_formatted_string(&Locale::en), L1_EXPLORER_URL, block.slot_height),
          true,
        ),
        ("Block CID", block.block, false),
        ("Proposed In Tx", format!("[{}]({}/tx/{})", block.id, VSC_BLOCKS_HOME, block.id), false),
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

#[derive(Clone)]
pub struct DiscordBot {
  pub conf: DiscordConf,
  pub db: MongoDB,
}

impl DiscordBot {
  pub fn init(conf: &DiscordConf, db: &MongoDB) -> DiscordBot {
    return DiscordBot { conf: conf.clone(), db: db.clone() };
  }

  pub fn start(&self) {
    let token = self.conf.token.clone();
    let intents = serenity_prelude::GatewayIntents::non_privileged();
    let db = self.db.clone();
    let framework = poise::Framework
      ::builder()
      .options(poise::FrameworkOptions {
        commands: vec![stats(), witness(), epoch(), block()],
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
          Ok(Data { db })
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
