use actix_web::{ get, http::{ header::ContentType, StatusCode }, post, web, HttpRequest, HttpResponse, Responder };
use actix_multipart::form::{ tempfile::TempFile, MultipartForm, text::Text };
use derive_more::derive::{ Display, Error };
use tokio_postgres::types::Type;
use serde::{ Serialize, Deserialize };
use serde_json;
use log::{ error, debug };
use std::{ fmt, io::Read };
use crate::db::DbPool;
use crate::vsc_types;
use crate::config::config;

#[derive(Display, Error)]
enum RespErr {
  #[display("Unknown error occured when querying database")] DbErr {
    msg: String,
  },
  #[display("Failed to query VSC-HAF backend")] VscHafErr,
  #[display("{msg}")] BadRequest {
    msg: String,
  },
}

impl fmt::Debug for RespErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RespErr::DbErr { msg } => write!(f, "{}", msg),
      RespErr::VscHafErr => Ok(()),
      RespErr::BadRequest { .. } => Ok(()),
    }
  }
}

impl actix_web::error::ResponseError for RespErr {
  fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
    let e = format!("{:?}", self);
    if e.len() > 0 {
      error!("{}", e);
    }
    HttpResponse::build(self.status_code())
      .insert_header(ContentType::json())
      .json(serde_json::json!({ "error": self.to_string() }))
  }

  fn status_code(&self) -> StatusCode {
    match *self {
      RespErr::DbErr { .. } => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::VscHafErr => StatusCode::INTERNAL_SERVER_ERROR,
      RespErr::BadRequest { .. } => StatusCode::BAD_REQUEST,
    }
  }
}

#[get("/")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[derive(Serialize, Deserialize)]
struct ReqVerifyNew {
  address: String,
  username: String,
  license: String,
  lang: String,
  dependencies: serde_json::Value,
}

#[post("/verify/new")]
async fn verify_new(req_data: web::Json<ReqVerifyNew>, ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let ct_req_method = config.vsc_haf_url.clone() + "/get_contract_by_id?id=" + &req_data.address;
  let ct_det = ctx
    .get_http_client()
    .get(ct_req_method.as_str())
    .send().await
    .map_err(|_| RespErr::VscHafErr)?
    .json::<vsc_types::ContractById>().await
    .map_err(|_| RespErr::VscHafErr)?;
  if ct_det.error.is_some() {
    // as of now only error this api could return is contract not found with status code 200
    return Ok(HttpResponse::NotFound().json(serde_json::json!(ct_det)));
  }
  let can_verify: String = db
    .query(
      "SELECT vsc_cv.can_verify_new($1,$2,$3);",
      &[
        (&req_data.address, Type::VARCHAR),
        (&req_data.license, Type::VARCHAR),
        (&req_data.lang, Type::VARCHAR),
      ]
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    [0].get(0);
  if can_verify.len() > 0 {
    return Err(RespErr::BadRequest { msg: can_verify });
  }
  db
    .query(
      "INSERT INTO vsc_cv.contracts(contract_addr,bytecode_cid,hive_username,status,license,lang,dependencies) VALUES($1,$2,$3,0::SMALLINT,(SELECT id FROM vsc_cv.licenses WHERE name=$4),(SELECT id FROM vsc_cv.languages WHERE name=$5),$6);",
      &[
        (&ct_det.contract_id, Type::VARCHAR),
        (&ct_det.code, Type::VARCHAR),
        (&req_data.username, Type::VARCHAR),
        (&req_data.license, Type::VARCHAR),
        (&req_data.lang, Type::VARCHAR),
        (&req_data.dependencies, Type::JSONB),
      ]
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[derive(Debug, MultipartForm)]
struct VerifUploadForm {
  #[multipart(limit = "1MB")]
  file: TempFile,
  contract_address: Text<String>,
  filename: Text<String>,
}

#[post("/verify/upload")]
async fn upload_file(
  req: HttpRequest,
  MultipartForm(mut form): MultipartForm<VerifUploadForm>,
  ctx: web::Data<DbPool>
) -> Result<HttpResponse, RespErr> {
  if let Some(auth_header) = req.headers().get("Authorization") {
    let auth_value = auth_header.to_str().unwrap_or("");
    debug!("Authentication header: {}", auth_value);
    debug!("Request query {}", req.query_string());
    // TODO: authenticate user
  }
  debug!("Uploaded file {} with size: {}", form.file.file_name.unwrap(), form.file.size);
  debug!("Contract address {}, new filename: {}", &form.contract_address.0, &form.filename.0);
  if form.file.size > 1024 * 1024 {
    return Err(RespErr::BadRequest { msg: String::from("Uploaded file size exceeds 1MB limit") });
  }
  let mut contents = String::new();
  match form.file.file.read_to_string(&mut contents) {
    Ok(_) => (),
    Err(e) => {
      error!("Failed to read uploaded file: {}", e.to_string());
      return Err(RespErr::BadRequest {
        msg: String::from("Failed to process uploaded file, most likely file is not in UTF-8 format."),
      });
    }
  }
  let db = ctx.get_ref().clone();
  let can_upload: String = db
    .query(
      "SELECT vsc_cv.can_upload_file($1,$2);",
      &[
        (&form.contract_address.0, Type::VARCHAR),
        (&form.filename.0, Type::VARCHAR),
      ]
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?
    [0].get(0);
  if can_upload.len() > 0 {
    return Err(RespErr::BadRequest { msg: can_upload });
  }
  db
    .query(
      "INSERT INTO vsc_cv.source_code(contract_addr,fname,content) VALUES($1,$2,$3) ON CONFLICT(contract_addr,fname) DO UPDATE SET content=$3;",
      &[
        (&form.contract_address.0, Type::VARCHAR),
        (&form.filename.0, Type::VARCHAR),
        (&contents, Type::VARCHAR),
      ]
    ).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[get("/languages")]
async fn list_langs(ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let rows = db
    .query("SELECT jsonb_agg(name) FROM vsc_cv.languages;", &[]).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let result: serde_json::Value = rows[0].get(0);
  Ok(HttpResponse::Ok().json(result))
}

#[get("/licenses")]
async fn list_licenses(ctx: web::Data<DbPool>) -> Result<HttpResponse, RespErr> {
  let db = ctx.get_ref().clone();
  let rows = db
    .query("SELECT jsonb_agg(name) FROM vsc_cv.licenses;", &[]).await
    .map_err(|e| RespErr::DbErr { msg: e.to_string() })?;
  let result: serde_json::Value = rows[0].get(0);
  Ok(HttpResponse::Ok().json(result))
}
