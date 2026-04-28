mod cache;
mod fetchers;
mod helpers;
mod resolver;
mod static_routes;
mod template;

use std::sync::Arc;
use std::time::Duration;

use actix_web::{ get, http::Method, web, HttpRequest, HttpResponse, Responder };

use crate::{ config::OgConf, types::server::Context };

use cache::TtlCache;
use fetchers::OgState;
use resolver::resolve_meta;
use template::render_html;

pub struct OgShared {
  pub state: OgState,
  pub cache: TtlCache,
  pub canonical_origin: String,
  pub og_image_url: String,
}

impl OgShared {
  pub fn new(conf: &OgConf) -> Self {
    let timeout = Duration::from_millis(conf.upstream_timeout_ms.unwrap_or(3000));
    let ttl = Duration::from_secs(conf.cache_ttl_sec.unwrap_or(60));
    let http_client = reqwest::Client
      ::builder()
      .timeout(timeout)
      .build()
      .expect("failed to build og http client");
    Self {
      state: OgState {
        http_client,
        gql_api_url: conf.gql_api_url.clone(),
        hasura_url: conf.hasura_url.clone(),
      },
      cache: TtlCache::new(ttl, 1000),
      canonical_origin: conf.canonical_origin.trim_end_matches('/').to_string(),
      og_image_url: conf.og_image_url.clone(),
    }
  }
}

fn resolve_origin(req: &HttpRequest, fallback: &str) -> String {
  let headers = req.headers();
  let host = headers
    .get("x-forwarded-host")
    .and_then(|v| v.to_str().ok())
    .or_else(|| headers.get("host").and_then(|v| v.to_str().ok()));
  let proto = headers
    .get("x-forwarded-proto")
    .and_then(|v| v.to_str().ok())
    .unwrap_or("https");
  match host {
    Some(h) => format!("{}://{}", proto, h),
    None => fallback.to_string(),
  }
}

#[get("/healthz")]
pub async fn healthz() -> impl Responder {
  HttpResponse::Ok().content_type("text/plain; charset=utf-8").body("ok")
}

async fn render(
  req: HttpRequest,
  tail: web::Path<String>,
  ctx: web::Data<Context>,
  og: web::Data<Arc<OgShared>>
) -> HttpResponse {
  let tail = tail.into_inner();
  let pathname = if tail.is_empty() { "/".to_string() } else { format!("/{}", tail) };
  let origin = resolve_origin(&req, &og.canonical_origin);
  let cache_key = format!("{}|{}", origin, pathname);

  let meta = if let Some(m) = og.cache.get(&cache_key) {
    m
  } else {
    let result = resolve_meta(&pathname, &origin, &og.og_image_url, &ctx.db, &og.state).await;
    og.cache.set(cache_key, result.clone());
    result
  };

  let is_head = req.method() == Method::HEAD;
  let html = render_html(&meta);
  let mut resp = HttpResponse::Ok();
  resp
    .content_type("text/html; charset=utf-8")
    .insert_header(("cache-control", "public, max-age=60"))
    .insert_header(("x-og-renderer", "1"));
  if is_head { resp.finish() } else { resp.body(html) }
}

pub fn render_resource() -> actix_web::Resource {
  web
    ::resource("/{tail:.*}")
    .route(web::get().to(render))
    .route(web::head().to(render))
}
