use std::sync::atomic::AtomicU64;

use crate::*;
use api::*;

#[derive(serde::Deserialize, Clone)]
pub struct ServerConfig {
  pub loglevel: String,
  pub cwd: String,
  pub inner_addr: String,
  pub worker_num: i32,
  pub https: bool,
  pub managers: Vec<String>,
  pub file_worker_num: i32,
  pub sql_url: String
}

#[derive(Default)]
struct ServerInfoInner {
  online_user: AtomicU64,
  online_client: AtomicU64,
}

pub struct ServerInfo {
  pub online_user: u64,
  pub online_client: u64,
}

pub struct Server {
  pub config: RwLock<ServerConfig>,
  // map username to a list of UserCtx
  pub user_ctxs: RwLock<HashMap<String, Vec<UserCtx>>>,
  pub file_handler: FileHandler,
  pub dbpool: mysql::Pool,
  info: ServerInfoInner
}

impl Server {
  pub fn from(server_config: ServerConfig) -> Self {
    Self {
      file_handler: FileHandler::new(server_config.file_worker_num),
      user_ctxs: RwLock::new(HashMap::new()),
      dbpool: mysql::Pool::new(server_config.sql_url.as_str()).unwrap(),
      config: RwLock::new(server_config),
      info: ServerInfoInner::default()
    }
  }

  pub fn r_server_info(&self) -> ServerInfo {
    ServerInfo {
      online_user: self.info.online_user.load(std::sync::atomic::Ordering::Relaxed),
      online_client: self.info.online_client.load(std::sync::atomic::Ordering::Relaxed),
    } 
  }

  pub fn r_config(&self) -> ServerConfig {
    self.config.read().unwrap().clone()
  }

  pub fn r_is_manager(&self, username: &String) -> bool {
    self.r_config().managers.contains(&username)
  }

  pub fn r_user_ctxs(&self) -> HashMap<String, Vec<UserCtx>> {
    self.user_ctxs.read().unwrap().clone()
  }

  pub fn r_user_ctxs_by_username(&self, username: &String) -> Vec<UserCtx> {
    self
      .user_ctxs
      .read()
      .unwrap()
      .get(username)
      .unwrap()
      .clone()
  }

  pub fn r_user_ctxs_exclude_self(&self, user_ctx: &UserCtx) -> Vec<UserCtx> {
    let mut ctx_vec = self.r_user_ctxs_by_username(&user_ctx.username);
    let index = ctx_vec.iter().position(|x| *x == *user_ctx).unwrap();
    ctx_vec.remove(index);
    ctx_vec
  }

  // add a user_ctx to server state, must not exist because user_ctx depend on connected time
  pub fn w_add_user_ctx(&self, user_ctx: UserCtx) {
    let mut user_ctxs = self.user_ctxs.write().unwrap();
    if let Some(ctx_vec) = user_ctxs.get_mut(&user_ctx.username) {
      assert!(!ctx_vec.contains(&user_ctx), "must not exist");
      self.info.online_client.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      ctx_vec.push(user_ctx.clone());
    } else {
      self.info.online_user.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      self.info.online_client.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      assert!(user_ctxs
        .insert(user_ctx.username.clone(), vec![user_ctx.clone()])
        .is_none());
    }
  }

  // if removed return true, else false
  pub fn w_remove_user_ctx(&self, user_ctx: &UserCtx) -> bool {
    let mut user_ctxs = self.user_ctxs.write().unwrap();
    if let Some(ctx_vec) = user_ctxs.get_mut(&user_ctx.username) {
      let mut target: Option<usize> = None;
      for (i, ctx) in ctx_vec.iter().enumerate() {
        if *ctx == *user_ctx {
          assert_eq!(target, None);
          target = Some(i);
        }
      }
      match target {
        Some(i) => {
          ctx_vec.remove(i);
          self.info.online_user.fetch_sub((ctx_vec.len() == 0) as u64, std::sync::atomic::Ordering::Relaxed);
          self.info.online_client.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
          true
        }
        None => false,
      }
    } else {
      false
    }
  }
}

pub fn read_server_config() -> Option<ServerConfig> {
  match std::fs::read_to_string("inner/config.json") {
    Ok(config_str) => match serde_json::from_str::<ServerConfig>(&config_str) {
      Ok(config_income) => Some(config_income),
      Err(e) => {
        eprintln!("Failed to parse config: {}", e);
        None
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

pub async fn start(server: Arc<Server>, use_config_thread: bool) -> std::io::Result<()> {
  let server_config = { server.config.read().unwrap().clone() };
  use std::io::Write;
  env_logger::builder()
    .parse_env(env_logger::Env::new().default_filter_or(server_config.loglevel))
    .format(|buf, record| {
      let module = record.module_path().unwrap_or("");
      let fileline = match record.file() {
        Some(path) => {
          format!("{}:{}", path, record.line().unwrap_or(0))
        }
        None => "".to_string(),
      };
      writeln!(
        buf,
        "[{} {} {}] {}",
        record.level(),
        fileline,
        module,
        record.args()
      )
    })
    .init();
  // set current cwd to project root such that the static file path work find
  let cwd = std::path::Path::new(&server_config.cwd);
  if std::env::set_current_dir(cwd).is_err() {
    panic!("Failed to change directory");
  } else {
    log::info!(
      "cwd has been set to {}",
      std::env::current_dir().unwrap().display()
    );
  }
  if use_config_thread {
    launch_config_thread(server.clone());
  }

  if server_config.https {
    // load TLS keys
    // to create a self-signed temporary cert for testing, run this in pulsear/inner:
    // `openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'`
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
      .set_private_key_file("inner/key.pem", SslFiletype::PEM)
      .unwrap();
    builder
      .set_certificate_chain_file("inner/cert.pem")
      .unwrap();
    HttpServer::new(move || {
      App::new()
        .wrap(actix_web::middleware::Logger::default())
        .app_data(web::Data::new(server.clone()))
        .route("/", web::get().to(index))
        .route("/index.html", web::get().to(index))
        .route("/ws", web::get().to(ws))
        .service(resources)
        .service(login)
        .service(logout)
        .service(get_file_elem)
        .service(get_file_list)
        .service(download_raw)
        .service(get_download_url)
        .service(download_by_url)
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
        .route("/ws", web::get().to(ws))
        .service(resources)
        .service(login)
        .service(logout)
        .service(get_file_elem)
        .service(get_file_list)
        .service(download_raw)
        .service(get_download_url)
        .service(download_by_url)
    })
    .bind(server_config.inner_addr)?
    .workers(server_config.worker_num as usize)
    .run()
    .await
  }
}

