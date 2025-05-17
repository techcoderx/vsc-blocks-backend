use bson::doc;
use mongodb::options::FindOneOptions;
use mongodb::results::UpdateResult;
use tokio::sync::Mutex;
use bollard::Docker;
use bollard::container::{ Config, CreateContainerOptions, WaitContainerOptions };
use bollard::models::{ HostConfig, ContainerWaitResponse };
use futures_util::StreamExt;
use serde_json;
use chrono::Utc;
use ipfs_dag::put_dag;
use std::{ error::Error, fs, path::Path, process, sync::Arc };
use log::{ info, debug, error };
use crate::mongo::MongoDB;
use crate::types::cv::{ CVContractCode, CVStatus };
use crate::types::vsc::json_to_bson;

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

async fn update_status(db: &MongoDB, addr: &str, status: CVStatus) -> Result<UpdateResult, mongodb::error::Error> {
  db.clone().cv_contracts.update_one(doc! { "_id": addr }, doc! { "$set": {"status": status.to_string() } }).await
}

#[derive(Clone)]
pub struct Compiler {
  db: MongoDB,
  running: Arc<Mutex<bool>>,
  docker: Arc<Docker>,
  image: String,
  compiler_dir: String,
}

impl Compiler {
  pub fn init(db: &MongoDB, image: String, compiler_dir: String) -> Self {
    let docker = match Docker::connect_with_local_defaults() {
      Ok(d) => d,
      Err(e) => {
        error!("Failed to connect to docker: {}", e);
        process::exit(1)
      }
    };
    return Compiler {
      db: db.clone(),
      running: Arc::new(Mutex::new(false)),
      docker: Arc::new(docker),
      image,
      compiler_dir,
    };
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
    let image = self.image.clone();
    let compiler_dir = self.compiler_dir.clone();
    debug!("Spawning new compiler thread");
    tokio::spawn(async move {
      let mut r = running.lock().await;
      *r = true;
      'mainloop: loop {
        let opt = FindOneOptions::builder()
          .sort(doc! { "request_ts": 1 })
          .build();
        let next_contract = db.cv_contracts.find_one(doc! { "status": "queued" }).with_options(opt).await;
        if next_contract.is_err() {
          error!("Failed to get next contract in queue");
          break;
        }
        let next_contract = next_contract.unwrap();
        if next_contract.is_none() {
          break;
        }
        let next_contract = next_contract.unwrap();
        let next_addr: &str = &next_contract.id;
        info!("Compiling contract {}", next_addr);
        let _ = update_status(&db, next_addr, CVStatus::InProgress).await;
        let files = db.cv_source_codes.find(doc! { "addr": next_addr }).await;
        if files.is_err() {
          error!("Failed to retrieve files: {}", files.unwrap_err());
          break;
        }
        let mut files_cursor = files.unwrap();
        let mut file_count = 0;
        while let Some(f) = files_cursor.next().await {
          match f {
            Ok(file) => {
              let written = fs::write(format!("{}/src/{}", compiler_dir, file.fname), file.content);
              if written.is_err() {
                let _ = update_status(&db, next_addr, CVStatus::Failed).await;
                error!("Failed to write files for contract {}", next_addr);
                break 'mainloop;
              }
              file_count += 1;
            }
            Err(_) => {
              let _ = update_status(&db, next_addr, CVStatus::Failed).await;
              error!("Failed to parse files for contract {}", next_addr);
              break 'mainloop;
            }
          }
        }
        if file_count == 0 {
          // this should not happen
          let _ = update_status(&db, next_addr, CVStatus::Failed).await;
          error!("There are no files to compile");
          break 'mainloop;
        }
        if &next_contract.lang == "assembly-script" {
          // assemblyscript
          let cont_name = "as-compiler";
          let mut pkg_json: serde_json::Value = serde_json
            ::from_str(include_str!("../as_compiler/package-template.json"))
            .unwrap();
          pkg_json["dependencies"] = next_contract.dependencies.unwrap();
          let pkg_json_w = fs::write(format!("{}/package.json", compiler_dir), serde_json::to_string_pretty(&pkg_json).unwrap());
          if pkg_json_w.is_err() {
            let _ = update_status(&db, next_addr, CVStatus::Failed).await;
            break;
          }
          // run the compiler
          let cont_conf = Config {
            image: Some(image.as_str()), // Image name
            host_config: Some(HostConfig {
              // Volume mount
              binds: Some(vec![format!("{}:/workdir/compiler", compiler_dir)]),
              // Auto-remove container on exit (equivalent to --rm)
              auto_remove: Some(true),
              ..Default::default()
            }),
            ..Default::default()
          };
          // Create the container with a specific name
          let cont_opt = CreateContainerOptions {
            name: cont_name,
            platform: None,
          };
          let container = docker.create_container(Some(cont_opt), cont_conf).await.unwrap();
          docker.start_container::<String>(&container.id, None).await.unwrap();
          // Wait for the container to finish and retrieve the exit code
          let mut stream = docker.wait_container(cont_name, Some(WaitContainerOptions { condition: "not-running" }));
          if let Some(Ok(ContainerWaitResponse { status_code, .. })) = stream.next().await {
            info!("Compiler exited with status code: {}", status_code);
            if status_code == 0 {
              let output = fs::read(format!("{}/build/build.wasm", compiler_dir));
              if output.is_err() {
                let _ = update_status(&db, next_addr, CVStatus::Failed).await;
                error!("build.wasm not found");
                break;
              }
              let output = output.unwrap();
              let output_cid = put_dag(output.as_slice());
              let cid_match = &output_cid == &next_contract.bytecode_cid;
              info!("Contract bytecode match: {}", cid_match.to_string().to_ascii_uppercase());
              if cid_match {
                let exports: serde_json::Value = serde_json
                  ::from_str(fs::read_to_string(format!("{}/build/exports.json", compiler_dir)).unwrap().as_str())
                  .unwrap();
                let _ = db.cv_source_codes
                  .insert_one(CVContractCode {
                    addr: String::from(next_addr),
                    fname: String::from("pnpm-lock.yaml"),
                    is_lockfile: true,
                    content: fs::read_to_string(format!("{}/pnpm-lock.yaml", compiler_dir)).unwrap(),
                  }).await
                  .map_err(|e| { error!("Failed to insert pnpm-lock.yaml: {}", e) });
                let updated_status = db.cv_contracts.update_one(
                  doc! { "_id": String::from(next_addr) },
                  doc! { "$set": {"status": CVStatus::Success.to_string(), "exports": json_to_bson(Some(&exports)), "verified_ts": bson::DateTime::from_chrono(Utc::now())} }
                ).await;
                if updated_status.is_err() {
                  error!("Failed to update status after compilation: {}", updated_status.unwrap_err());
                  break;
                }
                debug!("Exports: {}", exports);
              } else {
                let updated_status = update_status(&db, next_addr, CVStatus::NotMatch).await;
                if updated_status.is_err() {
                  error!("Failed to update status for bytecode mismatch: {}", updated_status.unwrap_err());
                  break;
                }
              }
            } else {
              let updated_status = update_status(&db, next_addr, CVStatus::Failed).await;
              if updated_status.is_err() {
                error!("Failed to update status after failed compilation: {}", updated_status.unwrap_err());
                break;
              }
            }
          }
          debug!("Deleting build artifacts");
          let _ = delete_if_exists(format!("{}/node_modules", compiler_dir).as_str());
          let _ = delete_if_exists(format!("{}/package.json", compiler_dir).as_str());
          let _ = delete_if_exists(format!("{}/pnpm-lock.yaml", compiler_dir).as_str());
          delete_dir_contents(fs::read_dir(format!("{}/src", compiler_dir)));
          delete_dir_contents(fs::read_dir(format!("{}/build", compiler_dir)));
        }
      }
      debug!("Closing compiler thread");
      *r = false;
    });
  }
}
