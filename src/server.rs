use actix_web::{ get, post, HttpResponse, Responder };
use log::info;

#[get("/")]
async fn hello() -> impl Responder {
  HttpResponse::Ok().body("Hello world!")
}

#[get("/hey")]
async fn hey() -> impl Responder {
  info!("/hey called");
  HttpResponse::Ok().body("Hey there!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
  HttpResponse::Ok().body(req_body)
}
