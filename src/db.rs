use deadpool_postgres::{ Config, CreatePoolError, Manager, ManagerConfig, RecyclingMethod, Runtime };
use deadpool::managed::Pool;
use tokio_postgres::{ types::ToSql, Error, NoTls, Row };
use crate::config;

pub fn init_pool() -> Result<Pool<Manager>, CreatePoolError> {
  let mut cfg = Config::new();
  cfg.url = Some(config::Config.psql_url.clone());
  cfg.manager = Some(ManagerConfig {
    recycling_method: RecyclingMethod::Fast,
  });
  let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
  Ok(pool)
}

pub async fn query(pool: Pool<Manager>, statement: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
  let client = pool.get().await.unwrap();
  let rows = client.query(statement, params).await?;
  Ok(rows)
}
