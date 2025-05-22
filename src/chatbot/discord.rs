use crate::{ constants::VSC_BLOCKS_HOME, config::DiscordConf, helpers::db::get_props, mongo::MongoDB };
use log::info;
use tokio;
use poise::{ serenity_prelude::{ self as serenity, CreateEmbed, Timestamp }, CreateReply };
use num_format::{ Locale, ToFormattedString };

struct Data {
  pub db: MongoDB,
} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

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
    let intents = serenity::GatewayIntents::non_privileged();
    let db = self.db.clone();
    let framework = poise::Framework
      ::builder()
      .options(poise::FrameworkOptions {
        commands: vec![stats()],
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
      let client = serenity::ClientBuilder::new(token, intents).framework(framework).await;
      client.unwrap().start().await.unwrap();
    });
  }
}
