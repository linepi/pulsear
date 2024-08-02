use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use clap::Parser;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::sync::{RwLock, Arc};

#[derive(serde::Deserialize, serde::Serialize)]
pub enum ResponseCode {
    Ok,
    Err(String),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct StreamBasicInfo {
    time_stamp: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginRequest {
    basic_info: StreamBasicInfo,
    login_info: LoginInfo,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct UserConfig {
    theme: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginResponse {
    basic_info: StreamBasicInfo,
    user_ctx: UserCtx,
    config: UserConfig,
    code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutRequest {
    basic_info: StreamBasicInfo,
    config: UserConfig,
    user_ctx: UserCtx,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutResponse {
    basic_info: StreamBasicInfo,
    code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub enum LoginChoice {
    Token(String),
    Password(String),
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct LoginInfo {
  username: String,
  choice: LoginChoice,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum UserCtxState {
    Conn,
    Disconn,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct UserCtx {
  username: String,
  token: String,
  state: UserCtxState,
}

pub fn get_system_timestamp_milli() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    // 转换为UNIX时间戳（自1970年1月1日以来的秒数）
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    // 转换为毫秒
    since_the_epoch.as_millis() as u64    
}

pub fn gen_random_token<LoginInfo: std::hash::Hash>(login_info: &LoginInfo) -> String {
  use std::hash::{Hasher, DefaultHasher};
  let mut hasher = DefaultHasher::new();
  login_info.hash(&mut hasher);
  hasher.finish().to_string()
}

pub fn load_user_config(req: &LoginRequest) -> UserConfig {
    return UserConfig {
        theme: "dark".to_string()
    }
}

pub fn store_user_config(req: &LogoutRequest) {

}

pub async fn index(data: web::Data<Arc<Server>>) -> HttpResponse {
  // like 192.168.31.126:4444
  let mut html_str = std::fs::read_to_string("pulsear-ui/ui/index.html")
                    .unwrap_or_else(|e| {
                        println!("error: {} of index.html", e);
                        e.to_string()
                    });
  // replace the ipaddr in html_str to actual one 
  html_str = html_str.replace(
    "giocdanewla", 
    format!("https://{}/", data.config.read().unwrap().outer_addr).as_str()
  );
  HttpResponse::Ok().body(html_str)
}

#[get("/resources/{path}")]
pub async fn resources(path: web::Path<String>) -> HttpResponse {
  log::info!("visit path {}", path);
  let res = std::fs::read_to_string(format!("pulsear-ui/ui/{}", path))
                    .unwrap_or_else(|e| {
                        println!("error: {} of {}", e, path);
                        e.to_string()
                    });
  HttpResponse::Ok().body(res)
}

#[post("/login")]
pub async fn login(param: web::Json<LoginRequest>) -> HttpResponse {
  log::info!("user login: {}", serde_json::to_string(&param).unwrap());
  let login_response: LoginResponse;
  match &param.login_info.choice {
    LoginChoice::Token(token) => {
      login_response = LoginResponse {
        user_ctx: UserCtx {
          username: param.login_info.username.clone(), 
          token: token.clone(),
          state: UserCtxState::Conn
        },
        basic_info: StreamBasicInfo {
          time_stamp: get_system_timestamp_milli()
        },
        config: load_user_config(&param.0),
        code: ResponseCode::Ok
      };
    },
    LoginChoice::Password(_) => {
      login_response = LoginResponse {
        user_ctx: UserCtx {
          username: param.login_info.username.clone(), 
          token: gen_random_token(&param.login_info),
          state: UserCtxState::Conn
        },
        basic_info: StreamBasicInfo {
          time_stamp: get_system_timestamp_milli()
        },
        config: load_user_config(&param.0),
        code: ResponseCode::Ok
      };
    }
  }
  HttpResponse::Ok().json(login_response)
}

#[post("/logout")]
pub async fn logout(param: web::Json<LogoutRequest>) -> HttpResponse {
  log::info!("user logout: {}", serde_json::to_string(&param).unwrap());
  let logout_response = LogoutResponse {
    basic_info: StreamBasicInfo {
      time_stamp: get_system_timestamp_milli()
    },
    code: ResponseCode::Ok
  };
  store_user_config(&param.0);
  HttpResponse::Ok().json(logout_response)
}

#[derive(Parser)]
struct Args {

}

#[derive(serde::Deserialize, Clone)]
pub struct ServerConfig {
  pub outer_addr: String,
  pub inner_addr: String,
  pub worker_num: i32,
  pub https: bool,
}

pub struct Server {
  pub config: RwLock<ServerConfig>,
  pub user_ctxs: RwLock<Vec<UserCtx>>,
}

fn read_server_config() -> Option<ServerConfig> {
  match std::fs::read_to_string("inner/config.json") {
      Ok(config_str) => {
          match serde_json::from_str::<ServerConfig>(&config_str) {
              Ok(config_income) => Some(config_income),
              Err(e) => {
                eprintln!("Failed to parse config: {}", e);
                None
              }
          }
      },
      Err(e) => {
        eprintln!("Failed to read config file: {}", e);
        None
      }
  }
}

// periodly read config from file
fn launch_config_thread(server: Arc<Server>) {
  std::thread::spawn(move || loop {
    {
      let mut config = server.config.write().unwrap();
      match read_server_config() {
        Some(config_income) => *config = config_income,
        None => {}
      }
    }
    std::thread::sleep(std::time::Duration::from_millis(1000));
  });
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
  let server_config = read_server_config().unwrap();

  let server = Arc::new(Server {
    config: RwLock::new(server_config.clone()),
    user_ctxs: RwLock::new(vec![]),
  });
  launch_config_thread(server.clone());

  if server_config.https {
    // load TLS keys
    // to create a self-signed temporary cert for testing, run this in pulsear/inner:
    // `openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'`
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file("inner/key.pem", SslFiletype::PEM)
        .unwrap();
    builder.set_certificate_chain_file("inner/cert.pem").unwrap();
    HttpServer::new(move || { 
        App::new()
          .wrap(actix_web::middleware::Logger::default())
          .app_data(web::Data::new(server.clone()))
          .route("/", web::get().to(index))
          .route("/index.html", web::get().to(index))
          .service(resources)
          .service(login)
          .service(logout)
      })
      .bind_openssl(server_config.inner_addr, builder)?
      .workers(server_config.worker_num as usize)
      .run()
      .await
  } else {
    HttpServer::new(move || { 
        App::new()
          .wrap(actix_web::middleware::Logger::default())
          .app_data(web::Data::new(server.clone()))
          .route("/", web::get().to(index))
          .route("/index.html", web::get().to(index))
          .service(resources)
          .service(login)
          .service(logout)
      })
      .bind(server_config.inner_addr)?
      .workers(server_config.worker_num as usize)
      .run()
      .await
  }
}

mod test {
  #[test]
  fn mysql() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
      let pool = mysql::Pool::new(url.as_str())?;
      let _ = pool.get_conn()?;
    } else {
      println!("please set env PLUSEAR_DATABASE_URL");
    }
    Ok(())
  }
}

