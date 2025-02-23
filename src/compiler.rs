use tokio::sync::Mutex;
use tokio_postgres::types::Type;
use std::{ fs, sync::Arc };
use log::{ info, debug, error };
use crate::db::DbPool;
use crate::config::config;

#[derive(Clone)]
pub struct Compiler {
  db: DbPool,
  running: Arc<Mutex<bool>>,
}

impl Compiler {
  pub fn init(db_pool: &DbPool) -> Self {
    return Compiler { db: db_pool.clone(), running: Arc::new(Mutex::new(false)) };
  }

  pub fn notify(&self) {
    if let Ok(r) = self.running.try_lock() {
      if !*r {
        self.run();
      }
    }
  }

  fn run(&self) {
    let db = self.db.clone();
    let running = Arc::clone(&self.running);
    debug!("Spawning new compiler thread");
    tokio::spawn(async move {
      let mut r = running.lock().await;
      *r = true;
      'eachcontract: while *r {
        let next_contract = db.query(
          "SELECT contract_addr, dependencies FROM vsc_cv.contracts WHERE status = 1::SMALLINT ORDER BY request_ts ASC LIMIT 1",
          &[]
        ).await;
        if next_contract.is_err() {
          error!("Failed to get next contract in queue: {}", next_contract.unwrap_err());
          *r = false;
          break;
        }
        let next_contract = next_contract.unwrap();
        if next_contract.len() == 0 {
          *r = false;
          break;
        }
        let next_addr: &str = next_contract[0].get(0);
        info!("Compiling contract {}", next_addr);
        let files = db.query(
          "SELECT fname, content FROM vsc_cv.source_code WHERE contract_addr=$1;",
          &[(&next_addr, Type::VARCHAR)]
        ).await;
        if files.is_err() {
          error!("Failed to retrieve files: {}", files.unwrap_err());
          *r = false;
          break;
        }
        let files = files.unwrap();
        if files.len() == 0 {
          // this should not happen
          // TODO: we should probably update the status to failed
          error!("Contract returned 0 files");
          *r = false;
          break;
        }
        for f in files {
          let written = fs::write(
            format!("{}/src/{}", config.ascompiler.src_dir, f.get::<usize, &str>(0)),
            f.get::<usize, &str>(1)
          );
          if written.is_err() {
            *r = false;
            break 'eachcontract;
          }
        }
        *r = false;
      }
      debug!("Closing compiler thread");
    });
  }
}
