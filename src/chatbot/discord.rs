use crate::{
  constants::VSC_BLOCKS_HOME,
  config::DiscordConf,
  helpers::db::{ get_props, get_witness, get_witness_stats },
  mongo::MongoDB,
};
use log::info;
use tokio;
use poise::{ serenity_prelude::{ self, CreateEmbed, Timestamp }, CreateReply };
use num_format::{ Locale, ToFormattedString };
use chrono::{ DateTime, Utc };

struct Data {
  pub db: MongoDB,
} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

pub fn time_ago(date: &str, one: bool) -> String {
  // Append 'Z' if missing to ensure UTC timezone
  let mut date_str = date.to_string();
  if !date.ends_with('Z') {
    date_str.push('Z');
  }

  // Parse date and convert to UTC
  let parsed_date = DateTime::parse_from_rfc3339(&date_str)
    .map(|dt| dt.with_timezone(&Utc))
    .expect("Invalid date format");

  let now = Utc::now();
  let duration = (now - parsed_date).abs(); // Get absolute duration

  // Calculate time components
  let days = duration.num_days();
  let hours = duration.num_hours() % 24;
  let minutes = duration.num_minutes() % 60;
  let seconds = duration.num_seconds() % 60;

  // Format output based on largest time component
  if days > 0 {
    if one { format!("{} days ago", days) } else { format!("{} days {} hrs ago", days, hours) }
  } else if hours > 0 {
    if one { format!("{} hrs ago", hours) } else { format!("{} hrs {} mins ago", hours, minutes) }
  } else if minutes > 0 {
    format!("{} mins ago", minutes)
  } else {
    format!("{} secs ago", seconds)
  }
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
        ("Last Update", time_ago(wit.ts.as_str(), false), true),
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
        commands: vec![stats(), witness()],
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
