use actix_web::{get, HttpResponse};

pub async fn index() -> HttpResponse {
  let f = std::fs::read_to_string("pulsear-ui/ui/index.html").unwrap();
  HttpResponse::Ok().body(f)
}
