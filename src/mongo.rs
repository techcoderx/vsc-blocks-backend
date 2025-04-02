use mongodb::{ options::ClientOptions, Client, Collection };
use std::error::Error;
use log::info;
use crate::vsc_types::Witnesses;

#[derive(Clone)]
pub struct MongoDB {
  pub witnesses: Collection<Witnesses>,
}

impl MongoDB {
  pub async fn init(url: String) -> Result<MongoDB, Box<dyn Error>> {
    let client_options = ClientOptions::parse(url).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("go-vsc");
    info!("Connected to VSC MongoDB database successfully");
    Ok(MongoDB {
      witnesses: db.collection("witnesses"),
    })
  }
}
