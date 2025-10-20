use bson::doc;
use ipfs_dag::put_dag_raw;
use mongodb::options::FindOneOptions;
use mongodb::results::UpdateResult;
use tokio::sync::Mutex;
use bollard::Docker;
use bollard::container::{ Config, CreateContainerOptions, WaitContainerOptions };
use bollard::models::{ HostConfig, ContainerWaitResponse };
use futures_util::StreamExt;
use chrono::Utc;
use tokio::time::{ sleep, Duration };
use git2::Repository;
use wasm_utils::list_exports;
use std::{ error::Error, fs, io, path::Path, process::{ self, Command }, sync::Arc };
use log::{ info, debug, error };
use crate::config::{ CompilerConf, GoCompilerConf };
use crate::mongo::MongoDB;
use crate::types::cv::{ CVStatus, GithubBranchInfo, GithubRepoInfo };

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

fn create_dir_if_not_exists(path: String) -> io::Result<()> {
  fs::create_dir(path).or_else(|e| {
    if e.kind() == io::ErrorKind::AlreadyExists { Ok(()) } else { Err(e) }
  })
}

fn chown(path: &String, uid: usize, gid: usize) {
  let _ = Command::new("chown").arg("-R").arg(format!("{}:{}", uid, gid)).arg(path.clone()).status();
}

async fn update_status(db: &MongoDB, addr: &str, status: CVStatus) -> Result<UpdateResult, mongodb::error::Error> {
  db.clone().cv_contracts.update_one(doc! { "_id": addr }, doc! { "$set": {"status": status.to_string() } }).await
}

/// Go contract compiler
#[derive(Clone)]
pub struct Compiler {
  db: MongoDB,
  running: Arc<Mutex<bool>>,
  docker: Docker,
  http_client: reqwest::Client,
  options: CompilerConf,
  go_options: GoCompilerConf,
}

impl Compiler {
  pub fn init(db: &MongoDB, http_client: &reqwest::Client, go_options: &GoCompilerConf, options: &CompilerConf) -> Self {
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
      docker: docker,
      http_client: http_client.clone(),
      options: options.clone(),
      go_options: go_options.clone(),
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
    let docker = self.docker.clone();
    let http_client = self.http_client.clone();
    let go_options = self.go_options.clone();
    let options = self.options.clone();
    let mkdir = create_dir_if_not_exists(go_options.output_dir.clone());
    if mkdir.is_err() {
      error!("Failed to create output directory");
      return;
    }
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
        let _ = delete_if_exists(go_options.src_dir.clone().as_str());
        let _ = create_dir_if_not_exists(go_options.src_dir.clone());
        if &next_contract.lang == "go" {
          // golang
          let repo_info = match
            http_client
              .get(format!("https://api.github.com/repos/{}", next_contract.repo_name))
              .bearer_auth(options.github_api_key.clone().expect("Missing Github API key"))
              .header("User-Agent", "VSC Blocks Contract Verifier")
              .header("X-GitHub-Api-Version", "2022-11-28")
              .send().await
          {
            Ok(r) => {
              if r.status() == 404 {
                error!("Repository {} does not exist", next_contract.repo_name);
                let _ = update_status(&db, &next_contract.id, CVStatus::Failed).await;
                continue 'mainloop;
              } else if r.status() != 200 {
                error!("Failed to fetch repository info from GitHub with status code {}", r.status());
                sleep(Duration::from_secs(600)).await;
                continue 'mainloop;
              }
              let i = r.json::<GithubRepoInfo>().await;
              if i.is_err() {
                error!("Failed to parse repository info");
                let _ = update_status(&db, &next_contract.id, CVStatus::Failed).await;
                continue 'mainloop;
              }
              let i = i.unwrap();
              if i.size > 10240 {
                error!("Repository is too large, size is {}", i.size);
                let _ = update_status(&db, &next_contract.id, CVStatus::Failed).await;
                continue 'mainloop;
              }
              i
            }
            Err(e) => {
              error!("Failed to fetch repository info from GitHub: {}", e.to_string());
              sleep(Duration::from_secs(600)).await;
              continue 'mainloop;
            }
          };
          // let _ = update_status(&db, &next_contract.id, CVStatus::InProgress).await;
          let mut branch_name = next_contract.repo_branch.clone();
          if branch_name.len() == 0 {
            branch_name = repo_info.default_branch;
          }
          let branch_info = match
            http_client
              .get(format!("https://api.github.com/repos/{}/branches/{}", next_contract.repo_name, branch_name))
              .bearer_auth(options.github_api_key.clone().expect("Missing Github API key"))
              .header("User-Agent", "VSC Blocks Contract Verifier")
              .header("X-GitHub-Api-Version", "2022-11-28")
              .send().await
          {
            Ok(b) => {
              if b.status() == 404 {
                error!("Branch {} does not exist", branch_name);
                let _ = update_status(&db, &next_contract.id, CVStatus::Failed).await;
                continue 'mainloop;
              } else if b.status() != 200 {
                error!("Failed to fetch repository info from GitHub with status code {}", b.status());
                sleep(Duration::from_secs(600)).await;
                continue 'mainloop;
              }
              let b = b.json::<GithubBranchInfo>().await;
              if b.is_err() {
                error!("Failed to parse branch info");
                let _ = update_status(&db, &next_contract.id, CVStatus::Failed).await;
                continue 'mainloop;
              }
              b.unwrap()
            }
            Err(e) => {
              error!("Failed to fetch branch info from GitHub: {}", e.to_string());
              sleep(Duration::from_secs(600)).await;
              continue 'mainloop;
            }
          };
          let git_commit = branch_info.commit.sha;
          let repo = match
            Repository::clone(
              format!("https://github.com/{}", next_contract.repo_name).as_str(),
              go_options.src_dir.clone().as_str()
            )
          {
            Ok(r) => r,
            Err(e) => {
              error!("Failed to clone repository: {}", e.to_string());
              sleep(Duration::from_secs(600)).await;
              continue 'mainloop;
            }
          };
          let checkout = repo.revparse_ext(&git_commit).and_then(|(object, reference)| {
            repo.checkout_tree(&object, None).and_then(|_| {
              match reference {
                Some(gref) => repo.set_head(gref.name().unwrap()),
                None => repo.set_head_detached(object.id()),
              }
            })
          });
          chown(&go_options.src_dir, 1000, 1000);
          if checkout.is_err() {
            error!("Failed to checkout commit");
            let _ = update_status(&db, &next_contract.id, CVStatus::Failed);
            continue 'mainloop;
          }
          let cont_conf = Config {
            image: Some(format!("tinygo/tinygo:{}", next_contract.tinygo_version)),
            host_config: Some(HostConfig {
              binds: Some(
                vec![
                  format!("{}:/home/tinygo", go_options.src_host_dir.clone().unwrap_or(go_options.src_dir.clone())),
                  format!("{}:/out", go_options.output_host_dir.clone().unwrap_or(go_options.output_dir.clone()))
                ]
              ),
              auto_remove: Some(true),
              // network_mode: Some(format!("none")),
              // readonly_rootfs: Some(false),
              memory: Some(2147483648),
              ..Default::default()
            }),
            cmd: Some(
              vec![
                format!("timeout"),
                format!("{}", go_options.timeout),
                format!("tinygo"),
                format!("build"),
                format!("-gc=custom"),
                format!("-scheduler=none"),
                format!("-panic=trap"),
                format!("-no-debug"),
                format!("-target=wasm-unknown"),
                format!("-o=/out/build.wasm"),
                format!("./contract")
              ]
            ),
            ..Default::default()
          };
          // Create the container with a specific name
          let cont_name = "go-compiler";
          let cont_opt = CreateContainerOptions {
            name: cont_name,
            platform: None,
          };
          let container = docker.create_container(Some(cont_opt), cont_conf).await.unwrap();
          let start_container = docker.start_container::<String>(&container.id, None).await;
          if start_container.is_err() {
            let _ = update_status(&db, next_addr, CVStatus::Failed).await;
            debug!("Failed to start compiler: {}", start_container.unwrap_err().to_string());
            continue 'mainloop;
          }
          // Wait for the container to finish and retrieve the exit code
          let mut stream = docker.wait_container(cont_name, Some(WaitContainerOptions { condition: "not-running" }));
          if let Some(Ok(ContainerWaitResponse { status_code, .. })) = stream.next().await {
            info!("Compiler exited with status code: {}", status_code);
            if status_code == 0 {
              let mut output = fs::read(format!("{}/build.wasm", go_options.output_dir));
              if output.is_err() {
                let _ = update_status(&db, next_addr, CVStatus::Failed).await;
                error!("build.wasm not found");
                continue 'mainloop;
              }
              let strip_success = match next_contract.strip_tool.clone() {
                Some(tool) => {
                  let mut success = false;
                  if &tool == "wabt" {
                    let strip_status = Command::new(options.wasm_strip.clone())
                      .arg("-o")
                      .arg(format!("{}/build-striped.wasm", go_options.output_dir))
                      .arg(format!("{}/build.wasm", go_options.output_dir))
                      .status();
                    success = strip_status.is_ok() && strip_status.unwrap().success();
                  } else if &tool == "wasm-tools" {
                    let strip_status = Command::new(options.wasm_tools.clone())
                      .arg("strip")
                      .arg("-o")
                      .arg(format!("{}/build-striped.wasm", go_options.output_dir))
                      .arg(format!("{}/build.wasm", go_options.output_dir))
                      .status();
                    success = strip_status.is_ok() && strip_status.unwrap().success();
                  }
                  success
                }
                None => true,
              };
              if next_contract.strip_tool.is_some() && strip_success {
                output = fs::read(format!("{}/build-striped.wasm", go_options.output_dir));
                if output.is_err() {
                  // this should not happen
                  let _ = update_status(&db, next_addr, CVStatus::Failed).await;
                  error!("build-striped.wasm not found");
                  continue 'mainloop;
                }
              }
              let output = output.unwrap();
              let output_cid = put_dag_raw(output.as_slice());
              let cid_match = &output_cid == &next_contract.code;
              info!("Contract bytecode match: {}", cid_match.to_string().to_ascii_uppercase());
              let exports = list_exports(&output)
                .map(|e| Some(e))
                .unwrap_or(None);
              if cid_match {
                let _ = db.cv_contracts.update_one(
                  doc! { "_id": next_contract.id },
                  doc! { "$set": {
                    "status": CVStatus::Success.to_string(),
                    "verified_ts": bson::DateTime::from_chrono(Utc::now()),
                    "git_commit": git_commit,
                    "license": repo_info.license,
                    "exports": exports
                  }}
                ).await;
              } else {
                let _ = update_status(&db, next_addr, CVStatus::NotMatch).await;
              }
            } else {
              info!("Compilation failed with exit code {}", status_code);
              let _ = update_status(&db, next_addr, CVStatus::Failed).await;
            }
          } else {
            info!("Compilation failed with unknown exit code");
            let _ = update_status(&db, next_addr, CVStatus::Failed).await;
          }
          debug!("Deleting build artifacts");
          let _ = delete_if_exists(go_options.src_dir.clone().as_str());
          delete_dir_contents(fs::read_dir(go_options.output_dir.clone()));
        }
      }
      debug!("Closing compiler thread");
      *r = false;
    });
  }
}
