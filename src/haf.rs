use std::{ collections::HashMap, error::Error };
use chrono::{ DateTime, NaiveDateTime, Utc };
use deadpool_postgres::{ Config, Pool, Runtime };
use log::info;
use tokio_postgres::NoTls;

#[derive(Clone, Debug)]
pub struct HAFTx {
  pub timestamp: NaiveDateTime,
  pub json: String,
}

#[derive(Clone)]
pub struct HAFDB {
  pool: Pool,
}

impl HAFDB {
  pub async fn init(url: &str) -> Result<HAFDB, Box<dyn Error + Send + Sync>> {
    let mut cfg = Config::new();
    cfg.url = Some(url.to_string());
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
    // fail fast if the database is unreachable or the URL is invalid
    let _ = pool.get().await?;
    info!("Connected to HAF database successfully");
    return Ok(HAFDB { pool });
  }

  pub async fn get_custom_json_txs(
    &self,
    trx_hashes: &[Vec<u8>]
  ) -> Result<HashMap<Vec<u8>, HAFTx>, Box<dyn Error + Send + Sync>> {
    let mut txs = HashMap::new();
    if trx_hashes.is_empty() {
      return Ok(txs);
    }
    let client = self.pool.get().await?;
    let hashes = trx_hashes.to_vec();
    let rows = client
      .query(
        "SELECT t.trx_hash, o.timestamp, o.body_value->>'json' AS json
         FROM hive.irreversible_transactions_view t
         JOIN hive.irreversible_operations_view_extended o
           ON o.block_num = t.block_num AND o.trx_in_block = t.trx_in_block
         WHERE t.trx_hash = ANY($1) AND o.op_type_id = 18
         ORDER BY t.block_num, t.trx_in_block, o.op_pos",
        &[&hashes]
      )
      .await?;
    for row in rows {
      let hash: Vec<u8> = row.get(0);
      let json: Option<String> = row.get(2);
      let json = match json {
        Some(j) => j,
        None => continue,
      };
      txs.entry(hash).or_insert(HAFTx { timestamp: row.get(1), json });
    }
    return Ok(txs);
  }

  pub async fn get_block_time(
    &self,
    block_num: u32
  ) -> Result<(u32, DateTime<Utc>), Box<dyn Error + Send + Sync>> {
    let client = self.pool.get().await?;
    let row = client
      .query_opt(
        "SELECT num, created_at FROM hive.irreversible_blocks_view
         WHERE num <= $1 ORDER BY num DESC LIMIT 1",
        &[&(block_num as i32)]
      )
      .await?;
    match row {
      Some(r) => {
        let num: i32 = r.get(0);
        let created_at: NaiveDateTime = r.get(1);
        return Ok((num as u32, created_at.and_utc()));
      }
      None => return Err("No blocks found in HAF database".into()),
    }
  }

  pub async fn get_first_block_at_or_after(
    &self,
    ts: NaiveDateTime
  ) -> Result<Option<u32>, Box<dyn Error + Send + Sync>> {
    let client = self.pool.get().await?;
    let row = client
      .query_opt(
        "SELECT num FROM hive.irreversible_blocks_view
         WHERE created_at >= $1 ORDER BY num ASC LIMIT 1",
        &[&ts]
      )
      .await?;
    return Ok(row.map(|r| r.get::<_, i32>(0) as u32));
  }
}
