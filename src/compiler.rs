use tokio::sync::Mutex;
use tokio_postgres::types::Type;
use bollard::Docker;
use bollard::container::{ Config, CreateContainerOptions, WaitContainerOptions };
use bollard::models::{ HostConfig, ContainerWaitResponse };
use futures_util::StreamExt;
use serde_json;
use chrono::Utc;
use ipfs_dag::put_dag;
use std::{ error::Error, fs, path::Path, process, sync::Arc };
use log::{ info, debug, error };
use crate::db::DbPool;
use crate::config::config;

fn delete_if_exists(path: &str) -> Result<(), Box<dyn Error>> {
  let p = Path::new(path);
  if p.exists() {
    if p.is_dir() {
      fs::remove_dir_all(path)?;
    } else {
      fs::remove_file(path)?;
    }
  }
  Ok(())
}

fn delete_dir_contents(read_dir_res: Result<fs::ReadDir, std::io::Error>) {
  if let Ok(dir) = read_dir_res {
    for entry in dir {
      if let Ok(entry) = entry {
        let path = entry.path();
        if path.is_dir() {
          fs::remove_dir_all(path).expect("Failed to remove a dir");
        } else {
          fs::remove_file(path).expect("Failed to remove a file");
        }
      }
    }
  }
}

#[derive(Clone)]
pub struct Compiler {
  db: DbPool,
  running: Arc<Mutex<bool>>,
  docker: Arc<Docker>,
}

impl Compiler {
  pub fn init(db_pool: &DbPool) -> Self {
    let docker = match Docker::connect_with_local_defaults() {
      Ok(d) => d,
      Err(e) => {
        error!("Failed to connect to docker: {}", e);
        process::exit(1)
      }
    };
    return Compiler { db: db_pool.clone(), running: Arc::new(Mutex::new(false)), docker: Arc::new(docker) };
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
    let docker = Arc::clone(&self.docker);
    debug!("Spawning new compiler thread");
    tokio::spawn(async move {
      let mut r = running.lock().await;
      *r = true;
      'mainloop: loop {
        let next_contract = db.query(
          "SELECT contract_addr, bytecode_cid, lang, dependencies FROM vsc_cv.contracts WHERE status = 1::SMALLINT ORDER BY request_ts ASC LIMIT 1",
          &[]
        ).await;
        if next_contract.is_err() {
          error!("Failed to get next contract in queue: {}", next_contract.unwrap_err());
          break;
        }
        let next_contract = next_contract.unwrap();
        if next_contract.len() == 0 {
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
          break;
        }
        let files = files.unwrap();
        if files.len() == 0 {
          // this should not happen
          // TODO: we should probably update the status to failed
          error!("Contract returned 0 files");
          break;
        }
        for f in files {
          let written = fs::write(
            format!("{}/src/{}", config.ascompiler.src_dir, f.get::<usize, &str>(0)),
            f.get::<usize, &str>(1)
          );
          if written.is_err() {
            break 'mainloop;
          }
        }
        if next_contract[0].get::<usize, i16>(2) == 0 {
          // assemblyscript
          let cont_name = "as-compiler";
          let mut pkg_json: serde_json::Value = serde_json
            ::from_str(include_str!("../as_compiler/package-template.json"))
            .unwrap();
          pkg_json["dependencies"] = next_contract[0].get::<usize, serde_json::Value>(3);
          let pkg_json_w = fs::write(
            format!("{}/package.json", config.ascompiler.src_dir),
            serde_json::to_string_pretty(&pkg_json).unwrap()
          );
          if pkg_json_w.is_err() {
            break;
          }
          // run the compiler
          let cont_conf = Config {
            image: Some(config.ascompiler.image.as_str()), // Image name
            host_config: Some(HostConfig {
              // Volume mount
              binds: Some(vec![format!("{}:/workdir/compiler", config.ascompiler.src_dir)]),
              // Auto-remove container on exit (equivalent to --rm)
              auto_remove: Some(true),
              ..Default::default()
            }),
            ..Default::default()
          };
          // Create the container with a specific name
          let cont_opt = CreateContainerOptions {
            name: cont_name,
            platform: Some("linux/arm64"),
          };
          let container = docker.create_container(Some(cont_opt), cont_conf).await.unwrap();
          docker.start_container::<String>(&container.id, None).await.unwrap();
          // Wait for the container to finish and retrieve the exit code
          let mut stream = docker.wait_container(cont_name, Some(WaitContainerOptions { condition: "not-running" }));
          if let Some(Ok(ContainerWaitResponse { status_code, .. })) = stream.next().await {
            info!("Compiler exited with status code: {}", status_code);
            if status_code == 0 {
              let output = fs::read(format!("{}/build/build.wasm", config.ascompiler.src_dir));
              if output.is_err() {
                error!("build.wasm not found");
                break;
              }
              let output = output.unwrap();
              let output_cid = put_dag(output.as_slice());
              let cid_match = output_cid == next_contract[0].get::<usize, String>(1);
              info!("Contract bytecode match: {}", cid_match.to_string().to_ascii_uppercase());
              if cid_match {
                let exports: serde_json::Value = serde_json
                  ::from_str(fs::read_to_string(format!("{}/build/exports.json", config.ascompiler.src_dir)).unwrap().as_str())
                  .unwrap();
                let _ = db
                  .query(
                    "INSERT INTO vsc_cv.source_code(contract_addr, fname, content) VALUES ($1,$2,$3);",
                    &[
                      (&next_addr, Type::VARCHAR),
                      (&"pnpm-lock.yaml".to_string(), Type::VARCHAR),
                      (&fs::read_to_string(format!("{}/pnpm-lock.yaml", config.ascompiler.src_dir)).unwrap(), Type::VARCHAR),
                    ]
                  ).await
                  .map_err(|e| { error!("Failed to insert pnpm-lock.yaml: {}", e) });
                let updated_status = db.query(
                  "UPDATE vsc_cv.contracts SET status=3::SMALLINT, exports=$2::JSONB, verified_ts=$3 WHERE contract_addr=$1;",
                  &[
                    (&next_addr, Type::VARCHAR),
                    (&exports, Type::JSONB),
                    (&Utc::now().naive_utc(), Type::TIMESTAMP),
                  ]
                ).await;
                if updated_status.is_err() {
                  error!("Failed to update status after compilation: {}", updated_status.unwrap_err());
                  break;
                }
                debug!("Exports: {}", exports);
              } else {
                let updated_status = db.query(
                  "UPDATE vsc_cv.contracts SET status=5::SMALLINT WHERE contract_addr=$1;",
                  &[(&next_addr, Type::VARCHAR)]
                ).await;
                if updated_status.is_err() {
                  error!("Failed to update status for bytecode mismatch: {}", updated_status.unwrap_err());
                  break;
                }
              }
            } else {
              let updated_status = db.query(
                "UPDATE vsc_cv.contracts SET status=4::SMALLINT WHERE contract_addr=$1;",
                &[(&next_addr, Type::VARCHAR)]
              ).await;
              if updated_status.is_err() {
                error!("Failed to update status after failed compilation: {}", updated_status.unwrap_err());
                break;
              }
            }
          }
          debug!("Deleting build artifacts");
          let _ = delete_if_exists(format!("{}/node_modules", config.ascompiler.src_dir).as_str());
          let _ = delete_if_exists(format!("{}/package.json", config.ascompiler.src_dir).as_str());
          let _ = delete_if_exists(format!("{}/pnpm-lock.yaml", config.ascompiler.src_dir).as_str());
          delete_dir_contents(fs::read_dir(format!("{}/src", config.ascompiler.src_dir)));
          delete_dir_contents(fs::read_dir(format!("{}/build", config.ascompiler.src_dir)));
        }
      }
      debug!("Closing compiler thread");
      *r = false;
    });
  }
}
