use actix_web::{get, post, web, HttpResponse};

pub async fn index() -> HttpResponse {
  let res = std::fs::read_to_string("pulsear-ui/ui/index.html")
                    .unwrap_or_else(|e| {
                        println!("error: {} of index.html", e);
                        e.to_string()
                    });
  HttpResponse::Ok().body(res)
}

#[get("/resources/{path}")]
pub async fn resources(path: web::Path<String>) -> HttpResponse {
  let res = std::fs::read_to_string(format!("pulsear-ui/ui/{}", path))
                    .unwrap_or_else(|e| {
                        println!("error: {} of {}", e, path);
                        e.to_string()
                    });
  HttpResponse::Ok().body(res)
}

#[derive(serde::Deserialize, Hash)]
struct LoginInfo {
  username: String,
  password: String
}

#[derive(serde::Serialize)]
struct UserCtx {
  username: String,
  token: String,
}

fn gen_random_token<LoginInfo: std::hash::Hash>(login_info: LoginInfo) -> String {
  use std::hash::{Hasher, DefaultHasher};
  let mut hasher = DefaultHasher::new();
  login_info.hash(&mut hasher);
  hasher.finish().to_string()
}

#[post("/login")]
pub async fn login(param: web::Json<LoginInfo>) -> HttpResponse {
  log::info!("login in username: {}", param.username);
  HttpResponse::Ok().json(UserCtx {
    username: param.username.clone(), 
    token: gen_random_token(param.into_inner())
  })
}
