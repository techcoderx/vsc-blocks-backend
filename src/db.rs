use deadpool_postgres::{ Config, CreatePoolError, Manager, ManagerConfig, RecyclingMethod, Runtime };
use deadpool::managed::Pool;
use tokio_postgres::{ types::ToSql, NoTls, Row };
use std::{ fmt, error };
use crate::config;

#[derive(Debug)]
pub struct DbQueryError {
  message: String,
}

impl error::Error for DbQueryError {}
impl fmt::Display for DbQueryError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

#[derive(Clone)]
pub struct DbPool {
  pool: Pool<Manager>,
}

impl DbPool {
  pub fn init() -> Result<Self, CreatePoolError> {
    let mut cfg = Config::new();
    cfg.url = Some(config::Config.psql_url.clone());
    cfg.manager = Some(ManagerConfig {
      recycling_method: RecyclingMethod::Fast,
    });
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
    Ok(Self { pool })
  }

  pub async fn query(&self, statement: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, DbQueryError> {
    let client = match self.pool.get().await {
      Ok(v) => v,
      Err(e) => {
        return Err(DbQueryError { message: e.to_string() });
      }
    };
    match client.query(statement, params).await {
      Ok(rows) => Ok(rows),
      Err(e) => Err(DbQueryError { message: e.to_string() }),
    }
  }
}
