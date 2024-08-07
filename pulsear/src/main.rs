#![allow(unused_imports)]
#![allow(dead_code)]

use actix_web::rt::time;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use bytes::Buf;
use mysql::params;
use mysql::prelude::*;
use mysql::TxOpts;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, RwLock};
use std::time::*;

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub enum ResponseCode {
  #[default]
  Ok,
  Err(String),
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct StreamBasicInfo {
  time_stamp: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginRequest {
  basic_info: StreamBasicInfo,
  login_info: LoginInfo,
}

#[derive(std::fmt::Debug)]
pub struct User {
  id: i32,
  username: String,
  token: String,
  config: UserConfig,
}

impl PartialEq for UserConfig {
  fn eq(&self, other: &Self) -> bool {
    self.theme == other.theme
  }
}

impl PartialEq for User {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.username == other.username
      && self.token == other.token
      && self.config == other.config
  }
}

#[derive(serde::Deserialize, serde::Serialize, std::fmt::Debug)]
pub struct UserConfig {
  id: i32,
  theme: String,
}

impl Default for UserConfig {
  fn default() -> Self {
    Self {
      id: 0,
      theme: "dark".to_string(),
    }
  }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct LoginResponse {
  basic_info: StreamBasicInfo,
  token: String,
  config: UserConfig,
  code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub enum SocketMetadataType {
  #[default]
  ESTABLISH,
  CONTENT,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutRequest {
  basic_info: StreamBasicInfo,
  config: UserConfig,
  username: String,
  token: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutResponse {
  basic_info: StreamBasicInfo,
  code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum LoginChoice {
  Token(String),
  Password(String),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginInfo {
  username: String,
  choice: LoginChoice,
}

#[derive(Hash)]
pub struct TokenGenerator {
  s: String,
}

#[derive(Clone)]
pub struct UserCtx {
  username: String,
  token: String,
  user_agent: String,
  establish_t: Time,
  session: Option<actix::Addr<WsSession>>,
}

impl UserCtx {
  fn hash(&self) -> String {
    gen_random_token(&TokenGenerator {
      s: format!("{}{}{}", self.username, self.token, self.establish_t),
    })
  }
}

impl PartialEq for UserCtx {
  fn eq(&self, other: &Self) -> bool {
    self.username == other.username
      && self.token == other.token
      && self.establish_t == other.establish_t
      && self.user_agent == other.user_agent
  }
}

impl fmt::Display for UserCtx {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "UserCtx{{ username: '{}', agent: '{}', create_at: '{}' }}",
      self.username, self.user_agent, self.establish_t
    )
  }
}

#[derive(Clone)]
struct Time(SystemTime);
impl Time {
  pub fn now() -> Self {
    Self(SystemTime::now())
  }

  pub fn from(st: SystemTime) -> Self {
    Self(st)
  }

  pub fn milli(&self) -> u64 {
    let since_the_epoch = self
      .0
      .duration_since(std::time::UNIX_EPOCH)
      .expect("Time went backwards");
    since_the_epoch.as_millis() as u64
  }

  pub fn nano(&self) -> u64 {
    let since_the_epoch = self
      .0
      .duration_since(std::time::UNIX_EPOCH)
      .expect("Time went backwards");
    since_the_epoch.as_nanos() as u64
  }

  pub fn system_time(&self) -> SystemTime {
    self.0
  }

  pub fn as_fmt(&self, fmt: &str) -> String {
    use chrono::DateTime;
    let datetime: DateTime<chrono::Local> = self.0.into();
    format!("{}", datetime.format(fmt))
  }
}

impl PartialEq for Time {
  fn eq(&self, other: &Self) -> bool {
    self.nano() == other.nano()
  }
}

impl fmt::Display for Time {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    use chrono::DateTime;
    let datetime: DateTime<chrono::Local> = self.0.into();
    write!(f, "{}", datetime.to_rfc2822())
  }
}

pub fn gen_random_token<TokenGenerator: std::hash::Hash>(gen: &TokenGenerator) -> String {
  use std::hash::{DefaultHasher, Hasher};
  let mut hasher = DefaultHasher::new();
  gen.hash(&mut hasher);
  hasher.finish().to_string()
}

pub fn get_user_token(param: &web::Json<LoginRequest>) -> String {
  match &(param.login_info).choice {
    LoginChoice::Token(token) => token.clone(),
    LoginChoice::Password(password) => gen_random_token::<TokenGenerator>(&TokenGenerator {
      s: format!("{}{}", &param.login_info.username, password),
    }),
  }
}

/// should only be used by one thread
struct SqlHandler {
  dbpool: mysql::Pool,
}
impl SqlHandler {
  /// prerequisity: user_config table created
  /// returned users: with all field filled
  fn get_users(&self) -> Result<Vec<User>, Box<dyn std::error::Error>> {
    let mut dbconn = self.dbpool.get_conn()?;
    let mut users: Vec<User> = vec![];
    dbconn.query_map(
      r"SELECT user.id, username, token, theme, user_config.id 
			  from user, user_config 
			  where user.id = user_config.user_id",
      |row| {
        let elems: (i32, String, String, String, i32) = row;
        let user = User {
          id: elems.0,
          username: elems.1,
          token: elems.2,
          config: UserConfig {
            id: elems.4,
            theme: elems.3,
          },
        };
        users.push(user);
      },
    )?;
    Ok(users)
  }

  /// prerequisity: user_config table created
  /// returned user: with all field filled
  fn get_user_by_name(
    &self,
    username: &String,
  ) -> Result<Option<User>, Box<dyn std::error::Error>> {
    let mut dbconn = self.dbpool.get_conn()?;
    let stmt = dbconn.prep(
      r"SELECT user.id, username, token, theme, user_config.id 
			  from user, user_config 
			  where user.id = user_config.user_id and
				    username = :name",
    )?;
    let rows: Vec<mysql::Row> = dbconn.exec(&stmt, params! { "name" => &username })?;
    if rows.len() == 0 {
      return Ok(None);
    } else if rows.len() > 1 {
      return Err(Box::from("multiple use found"));
    }
    let row: (i32, String, String, String, i32) =
      mysql::from_row_opt(rows.first().unwrap().to_owned())?;
    let user = User {
      id: row.0,
      username: row.1,
      token: row.2,
      config: UserConfig {
        id: row.4,
        theme: row.3,
      },
    };
    Ok(Some(user))
  }

  /// user: username, token, config
  /// returned user: id, ..., config_id
  fn add_user(&self, user: &User) -> Result<Option<User>, Box<dyn std::error::Error>> {
    match self.get_user_by_name(&user.username)? {
      Some(u) => return Err(Box::from(format!("user exists: {:?}", u))),
      None => (),
    }

    let mut dbconn = self.dbpool.start_transaction(TxOpts::default())?;
    let stmt = dbconn.prep(
      r"INSERT INTO user(username, token)
			  VALUES (:username, :token)",
    )?;
    dbconn.exec_drop(
      &stmt,
      params! { "username" => &user.username, "token" => &user.token },
    )?;
    let user_id: i32 = dbconn
      .exec_first(
        r"SELECT id from user
			  WHERE username = ?",
        (&user.username,),
      )?
      .expect("user should exists after insertion");

    let stmt = dbconn.prep(
      r"INSERT INTO user_config(user_id, theme)
			  VALUES (:user_id, :theme)",
    )?;
    dbconn.exec_drop(
      &stmt,
      params! { "user_id" => &user_id, "theme" => &user.config.theme },
    )?;
    dbconn.commit()?;
    self.get_user_by_name(&user.username)
  }

  fn delete_user_by_name(&self, username: &String) -> Result<(), Box<dyn std::error::Error>> {
    match self.get_user_by_name(username)? {
      Some(u) => log::info!("delete user[{:?}]", u),
      None => return Err(Box::from(format!("user does not exist: {}", username))),
    }

    let mut dbconn = self.dbpool.start_transaction(TxOpts::default())?;
    dbconn.exec_drop(
      r"DELETE FROM user_config 
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )",
      (username,),
    )?;
    dbconn.exec_drop(
      r"DELETE FROM user 
			  WHERE username = ?",
      (username,),
    )?;
    dbconn.commit()?;
    Ok(())
  }

  fn update_user_config_by_name(
    &self,
    username: &String,
    config: &UserConfig,
  ) -> Result<(), Box<dyn std::error::Error>> {
    match self.get_user_by_name(username)? {
      Some(u) => log::info!("update user[{:?}]'s config as {:?}", u, config),
      None => return Err(Box::from(format!("user does not exist: {}", username))),
    }
    let mut dbconn = self.dbpool.get_conn()?;
    dbconn.exec_drop(
      r"UPDATE user_config SET theme=?
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )",
      (&config.theme, &username),
    )?;
    Ok(())
  }

  /// change last login time
  fn user_login(&self, username: &String) -> Result<(), Box<dyn std::error::Error>> {
    match self.get_user_by_name(username)? {
      Some(u) => log::info!("update user[{:?}]'s login time", u),
      None => return Err(Box::from(format!("user does not exist: {}", username))),
    }
    let mut dbconn = self.dbpool.get_conn()?;
    dbconn.exec_drop(
      r"UPDATE user SET last_login_time=NOW()
			  WHERE username = ?",
      (&username,),
    )?;
    Ok(())
  }
}

pub async fn index() -> HttpResponse {
  let html_str = match std::fs::read_to_string("pulsear-ui/ui/index.html") {
    Ok(s) => s,
    Err(e) => {
      let errmsg = format!("error: {} of index.html", e);
      log::info!("{}", &errmsg);
      return HttpResponse::InternalServerError().body(errmsg);
    }
  };
  HttpResponse::Ok().body(html_str)
}

#[get("/resources/{path}")]
pub async fn resources(p: web::Path<String>) -> HttpResponse {
  let path = format!("pulsear-ui/ui/{}", p);
  log::info!("read resources {}", path);
  let res = std::fs::read_to_string(&path).unwrap_or_else(|e| {
    println!("error: {} of {}", e, path);
    e.to_string()
  });
  HttpResponse::Ok().body(res)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileListElem {
  name: String,
  size: String,
  create_t: String,
  access_t: String,
  modify_t: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileList {
  files: Vec<FileListElem>
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileListRequest {
  username: String,
  token: String
}

pub fn do_get_files(param: web::Json<FileListRequest>, data: web::Data<Arc<Server>>) 
  -> Result<HttpResponse, Box<dyn std::error::Error>> {
  let mut list = FileList {
    files: vec![]
  };
  let sqlhandler = SqlHandler {
    dbpool: data.dbpool.clone(),
  };
  let user = match sqlhandler.get_user_by_name(&param.username)? {
    Some(u) => {
      assert_eq!(&u.username, &param.username);
      assert_eq!(&u.token, &param.token);
      u
    }
    None => {
      return Err(Box::from("user not exists"));
    }
  };

  let storage = std::path::PathBuf::from("inner/storage");
  let userfolder = storage.join(&user.username);
  if !userfolder.exists() {
    std::fs::create_dir_all(&userfolder)?;
  }
  let read_dir = std::fs::read_dir(userfolder)?;
  for path in read_dir {
    let entry = path?;
    let name = entry.file_name().into_string().unwrap();
    let size_in_bytes = entry.metadata()?.size();
    let size: String = |bytes| -> String {
      if bytes < 1024 {
        return format!("{}b", bytes);
      } else if bytes < 1024 * 1024 {
        return format!("{:.1}Kb", bytes as f64 / 1024.0);
      } else if bytes < 1024 * 1024 * 1024 {
        return format!("{:.3}Mb", bytes as f64 / 1024.0 / 1024.0);
      } else {
        return format!("{:.5}Gb", bytes as f64 / 1024.0 / 1024.0 / 1024.0);
      }
    } (size_in_bytes);
    let create_t = Time::from(entry.metadata()?.created()?).as_fmt("%Y-%m-%d %H:%M:%S");
    let modify_t = Time::from(entry.metadata()?.modified()?).as_fmt("%Y-%m-%d %H:%M:%S");
    let access_t = Time::from(entry.metadata()?.accessed()?).as_fmt("%Y-%m-%d %H:%M:%S");
    let file_elem = FileListElem {
      name,
      size,
      create_t,
      modify_t,
      access_t
    };
    list.files.push(file_elem);
  }
  Ok(HttpResponse::Ok().body(serde_json::to_string(&list).unwrap()))
}

#[post("/files")]
pub async fn get_files(param: web::Json<FileListRequest>, data: web::Data<Arc<Server>>) -> HttpResponse {
  log::info!("user try get file: {}", serde_json::to_string(&param).unwrap());
  let resp: HttpResponse;
  match do_get_files(param, data) {
    Ok(response) => resp = response,
    Err(e) => {
      resp = HttpResponse::BadRequest().body(e.to_string());
    }
  }
  log::debug!("Server get file resp with {:?}", resp);
  resp 
}

pub fn do_login(
  param: &web::Json<LoginRequest>,
  data: &web::Data<Arc<Server>>,
) -> Result<LoginResponse, Box<dyn std::error::Error>> {
  log::info!("user try login: {}", serde_json::to_string(param).unwrap());
  let sqlhandler = SqlHandler {
    dbpool: data.dbpool.clone(),
  };

  let token = get_user_token(param);
  let user: User;
  {
    // lock begin
    user = match sqlhandler.get_user_by_name(&param.login_info.username)? {
      Some(u) => {
        if &u.token != &token {
          return Err(Box::from("password not true or is has been changed"));
        }
        u
      }
      None => {
        let u = sqlhandler
          .add_user(&User {
            id: 0,
            username: param.login_info.username.clone(),
            token: token.clone(),
            config: UserConfig::default(),
          })?
          .expect("add user error!");
        assert_eq!(&u.token, &token);
        assert_eq!(&u.username, &param.login_info.username);
        assert_eq!(&u.config, &UserConfig::default());
        u
      }
    };
  } // lock end

  let login_response = LoginResponse {
    token: user.token,
    basic_info: StreamBasicInfo {
      time_stamp: Time::now().milli(),
    },
    config: user.config,
    code: ResponseCode::Ok,
  };
  sqlhandler.user_login(&user.username)?;
  Ok(login_response)
}

/// login, if username does not exist, signup and login.
#[post("/login")]
pub async fn login(param: web::Json<LoginRequest>, data: web::Data<Arc<Server>>) -> HttpResponse {
  let resp: HttpResponse;
  match do_login(&param, &data) {
    Ok(response) => resp = HttpResponse::Ok().json(response),
    Err(e) => {
      // TODO: add more http status code
      let mut response = LoginResponse::default();
      response.code = ResponseCode::Err(e.to_string());
      resp = HttpResponse::Ok().json(response)
    }
  }
  log::debug!("Server login resp with {:?}", resp);
  resp
}

pub fn do_logout(
  param: &web::Json<LogoutRequest>,
  data: &web::Data<Arc<Server>>,
) -> Result<LogoutResponse, Box<dyn std::error::Error>> {
  log::info!(
    "user try logout: {}",
    serde_json::to_string(&param).unwrap()
  );
  let sqlhandler = SqlHandler {
    dbpool: data.dbpool.clone(),
  };

  match sqlhandler.get_user_by_name(&param.username)? {
    Some(u) => {
      assert_eq!(&u.username, &param.username);
      assert_eq!(&u.token, &param.token);
      u
    }
    None => {
      return Err(Box::from("logout user not exists"));
    }
  };

  let logout_response = LogoutResponse {
    basic_info: StreamBasicInfo {
      time_stamp: Time::now().milli(),
    },
    code: ResponseCode::Ok,
  };
  sqlhandler.update_user_config_by_name(&param.username, &param.config)?;
  Ok(logout_response)
}

#[post("/logout")]
pub async fn logout(param: web::Json<LogoutRequest>, data: web::Data<Arc<Server>>) -> HttpResponse {
  let resp: HttpResponse;
  match do_logout(&param, &data) {
    Ok(response) => resp = HttpResponse::Ok().json(response),
    Err(e) => {
      let mut response = LoginResponse::default();
      response.code = ResponseCode::Err(e.to_string());
      resp = HttpResponse::Ok().json(response)
    }
  }
  log::debug!("Server logout resp with {:?}", resp);
  resp
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct WsClient {
  username: String,
  user_ctx_hash: String,
}

impl WsClient {
  fn new(user_ctx: &UserCtx) -> Self {
    Self {
      username: user_ctx.username.clone(),
      user_ctx_hash: user_ctx.hash(),
    }
  }
}

// all thing about a file and its transfer
// binary package:
//  bytes:   |   32      |  4        |  slice_size  |
//  meaning: |  file_hash| slice_idx | file content |
#[derive(serde::Deserialize, serde::Serialize)]
struct FileRequest {
  username: String,
  name: String,
  size: u64,
  slice_size: u64,
  last_modified_t: u64,
  file_hash: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
enum FileResponseStatus {
  Ok,
  Resend,
  Fatalerr,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct FileResponse {
  sha256: String,
  slice_idx: u64,
  status: FileResponseStatus,
}

#[derive(serde::Deserialize, serde::Serialize)]
enum WsMessageClass {
  Establish,                  // two direction
  Leave,                      // on logout
  FileSendable((String, bool)),       // come out
  FileResponse(FileResponse), // come out
  FileRequest(FileRequest),   // come in
  Text(String),               // two direction
  Notify(String),
  Errjson(String), //
}

#[derive(serde::Deserialize, serde::Serialize)]
enum WsDispatchType {
  Broadcast,
  BroadcastSameUser,
  Server,
  Targets(Vec<WsClient>),
}

#[derive(serde::Deserialize, serde::Serialize)]
enum WsSender {
  Server,
  User(WsClient),
  Manager(WsClient),
}

#[derive(Message)]
#[rtype(result = "()")]
struct WsBinMessage(bytes::Bytes);

#[derive(Message)]
#[rtype(result = "()")]
struct WsTextMessage(String);

#[derive(Message)]
#[rtype(result = "()")]
#[derive(serde::Deserialize, serde::Serialize)]
struct WsMessage {
  sender: WsSender,
  msg: WsMessageClass,
  policy: WsDispatchType,
}

#[derive(Message)]
#[rtype(result = "()")]
#[derive(serde::Deserialize, serde::Serialize)]
struct WsMessageInner {
  sender: WsSender,
  msg: WsMessageClass,
  policy: WsDispatchType,
}

struct WsSession {
  server: Arc<Server>,
  hb_t: Time,
  user_ctx: UserCtx,
}

struct FileWorker {}
struct FileJob {
  request: FileRequest,
  file: std::fs::File,
}

impl FileJob {
  fn new(req: FileRequest) -> Self {
    let storage = std::path::PathBuf::from("inner/storage");
    let userfolder = storage.join(&req.username);
    if !userfolder.exists() {
      std::fs::create_dir_all(&userfolder).expect("should create dir successfully");
    }
    let filepath = userfolder.join(&req.name);
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath).unwrap();
    Self {
      request: req,
      file: f
    }
  }
}

pub struct FileHandler {
  workers: Vec<FileWorker>,
  requests: RwLock<HashMap<String, FileJob>>
}

impl FileHandler {
  fn new() -> Self {
    Self { 
      workers: vec![],
      requests: RwLock::new(HashMap::new())
    }
  }

  fn register_file_response_callback(&self, f: &dyn Fn(&UserCtx, FileResponse)) {

  }

  fn add(&self, req: FileRequest) -> bool {
    let mut requests = self.requests.write().unwrap();
    requests.insert(req.file_hash.clone(), FileJob::new(req));
    true
  }

  fn send(&self, bytes: bytes::Bytes) {
    let hashval = bytes.slice(0..32);
    let hashstr: String = hashval.iter().map(|b| {
      format!("{:02x}", b).to_string()
    }).collect();

    let mut request = self.requests.write().unwrap();
    let job = request.get_mut(&hashstr).unwrap();
    use std::io::{self, Seek, SeekFrom, Write};

    let index: u64 = bytes.slice(32..36).get_u32_le() as u64;
    log::info!("write index {}", index);
    let slice_size: u64 = job.request.slice_size;

    job.file.seek(SeekFrom::Start(slice_size*index)).unwrap();
    job.file.write_all(&bytes.slice(36..)).unwrap();
  }
}

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

use actix::prelude::*;
impl actix::Actor for WsSession {
  type Context = ws::WebsocketContext<Self>;
  fn started(&mut self, ctx: &mut Self::Context) {
    log::info!("actor started");
    ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
      if SystemTime::now()
        .duration_since(act.hb_t.system_time())
        .unwrap()
        > CLIENT_TIMEOUT
      {
        log::warn!("Websocket Client heartbeat failed, disconnecting!");
        ctx.stop();
        return;
      }
      ctx.ping(b""); // ping will send to Self
    });
    self
      .server
      .file_handler
      .register_file_response_callback(&|user_ctx, pkg_resp| {
        user_ctx.session.clone().unwrap().do_send(WsMessage {
          sender: WsSender::Server,
          msg: WsMessageClass::FileResponse(pkg_resp),
          policy: WsDispatchType::Targets(vec![WsClient::new(user_ctx)]),
        })
      });
  }

  fn stopping(&mut self, _ctx: &mut Self::Context) -> actix::Running {
    actix::Running::Stop
  }

  fn stopped(&mut self, _ctx: &mut Self::Context) {
    assert!(
      self.server.w_remove_user_ctx(&self.user_ctx),
      "should have user_ctx"
    );
    log::info!("actor stopped");
  }
}
impl Handler<WsTextMessage> for WsSession {
  type Result = ();

  // dispatch message
  fn handle(&mut self, text: WsTextMessage, ctx: &mut Self::Context) {
    log::info!("send: {}", text.0);
    ctx.text(text.0);
  }
}

impl Handler<WsMessage> for WsSession {
  type Result = ();

  // dispatch message
  fn handle(&mut self, ws_message: WsMessage, ctx: &mut Self::Context) {
    log::info!(
      "handle wsmessage {}",
      serde_json::to_string(&ws_message).expect("ws message must be deserializable")
    );
    let pred: Box<dyn Fn(&UserCtx) -> bool>;
    match &ws_message.policy {
      WsDispatchType::Broadcast => {
        pred = Box::new(|user_ctx| user_ctx != &self.user_ctx);
      }
      WsDispatchType::BroadcastSameUser => {
        pred = Box::new(|user_ctx| {
          user_ctx != &self.user_ctx && user_ctx.username == self.user_ctx.username
        });
      }
      WsDispatchType::Server => {
        ctx.address().do_send(WsMessageInner {
          sender: ws_message.sender,
          msg: ws_message.msg,
          policy: ws_message.policy,
        });
        return;
      }
      WsDispatchType::Targets(clients) => {
        pred = Box::new(|user_ctx| {
          for client in clients.iter() {
            if user_ctx.hash() == client.user_ctx_hash {
              return true;
            }
          }
          false
        });
      }
    }
    let user_ctxs = self.server.r_user_ctxs();
    for pair in user_ctxs.iter() {
      let ctx_vec = pair.1;
      for user_ctx in ctx_vec.iter() {
        // must send to self
        if pred(user_ctx) {
          let new_ws_message: WsMessage =
            serde_json::from_str(serde_json::to_string(&ws_message).unwrap().as_str()).unwrap();
          user_ctx.session.clone().unwrap().do_send(WsMessageInner {
            sender: new_ws_message.sender,
            msg: new_ws_message.msg,
            policy: WsDispatchType::Targets(vec![WsClient::new(user_ctx)]),
          });
        }
      }
    }
  }
}

impl Handler<WsMessageInner> for WsSession {
  type Result = ();

  fn handle(&mut self, ws_message: WsMessageInner, ctx: &mut Self::Context) {
    match &ws_message.policy {
      WsDispatchType::Server => (),
      WsDispatchType::Targets(clients) => {
        if clients.len() != 1 {
          panic!("unexpected");
        }
      }
      _ => {
        panic!("unexpected");
      }
    }

    log::info!(
      "handle wsmessage inner {}",
      serde_json::to_string(&ws_message).expect("ws message must be deserializable")
    );

    match ws_message.msg {
      WsMessageClass::Establish => {
        match &ws_message.policy {
          WsDispatchType::Server => (),
          _ => {
            panic!("unexpected");
          }
        }

        let username = match ws_message.sender {
          WsSender::User(u) => u.username.clone(),
          _ => {
            log::error!("unexpected");
            return;
          }
        };
        let sqlhandler = SqlHandler {
          dbpool: self.server.dbpool.clone(),
        };
        let token = sqlhandler
          .get_user_by_name(&username)
          .expect("should has user")
          .expect("should has user")
          .token;

        self.user_ctx.token = token.clone();
        self.user_ctx.username = username.clone();
        self.user_ctx.session = Some(ctx.address());
        log::info!("add new user_ctx: {}", self.user_ctx);
        self.server.w_add_user_ctx(self.user_ctx.clone());

        // manager login will broadcast to all clients
        if self.server.r_is_manager(&username) {
          ctx.address().do_send(WsMessage {
            sender: WsSender::Manager(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("Enter the site!".into()),
            policy: WsDispatchType::Broadcast,
          });
        }
        // user login will broadcast to all clients with same user
        else {
          ctx.address().do_send(WsMessage {
            sender: WsSender::User(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("your account login at another place!".into()),
            policy: WsDispatchType::BroadcastSameUser,
          });
        }

        // response establish with user_ctx_hash
        ctx.address().do_send(WsTextMessage(
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::Establish,
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap(),
        ));
      }
      WsMessageClass::Leave => {
        match &ws_message.policy {
          WsDispatchType::Server => (),
          _ => {
            panic!("unexpected");
          }
        }

        if self.server.r_is_manager(&self.user_ctx.username) {
          ctx.address().do_send(WsMessage {
            sender: WsSender::Manager(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("Leave the site!".into()),
            policy: WsDispatchType::Broadcast,
          });
        }
        // user login will broadcast to all clients with same user
        else {
          ctx.address().do_send(WsMessage {
            sender: WsSender::User(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("your account leave at another place!".into()),
            policy: WsDispatchType::BroadcastSameUser,
          });
        }

        // response establish with user_ctx_hash
        ctx.address().do_send(WsTextMessage(
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::Leave,
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap(),
        ));
      }
      WsMessageClass::FileRequest(pkg) => {
        ctx.address().do_send(WsTextMessage(
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::FileSendable((pkg.file_hash.clone(), self.server.file_handler.add(pkg))),
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap(),
        ));
      }
      WsMessageClass::Text(_) => {
        ctx.address().do_send(WsTextMessage(serde_json::to_string(&ws_message).unwrap()));
      }
      WsMessageClass::Errjson(e) => {
        log::error!("err json: {e}");
      }
      WsMessageClass::Notify(_) => {
        ctx.address().do_send(WsTextMessage(serde_json::to_string(&ws_message).unwrap()));
      }
      t => {
        log::error!("unexpect msg: {}", serde_json::to_string(&t).unwrap());
      }
    }
  }
}

impl Handler<WsBinMessage> for WsSession {
  type Result = ();
  fn handle(&mut self, b: WsBinMessage, _ctx: &mut Self::Context) {
    self.server.file_handler.send(b.0);
  }
}

impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
  fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
    let msg = match msg {
      Err(e) => {
        log::error!("ws msg is Err: {}", e);
        return;
      }
      Ok(msg) => msg,
    };
    match msg {
      ws::Message::Ping(msg) => {
        self.hb_t = Time::now();
        log::debug!("ws: {:?}", msg);
        ctx.pong(&msg);
      }
      ws::Message::Pong(_) => {
        self.hb_t = Time::now();
      }
      ws::Message::Text(text) => {
        // new client connected
        log::info!("ws receive text from client: {}", text);
        let ws_message: WsMessage = match serde_json::from_str(&text) {
          Ok(m) => m,
          Err(e) => {
            ctx.address().do_send(WsTextMessage(
              serde_json::to_string(&WsMessage {
                sender: WsSender::Server,
                msg: WsMessageClass::Errjson(e.to_string()),
                policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
              })
              .unwrap(),
            ));
            return;
          }
        };
        // send Self for more function
        ctx.address().do_send(ws_message);
      }
      ws::Message::Binary(b) => {
        ctx.address().do_send(WsBinMessage(b));
      }
      ws::Message::Close(reason) => {
        log::info!("ws receive close: {:?}", reason);
        ctx.close(reason);
        ctx.stop();
      }
      ws::Message::Continuation(_) => {
        ctx.stop();
      }
      ws::Message::Nop => {}
    }
  }
}

pub async fn ws(
  req: HttpRequest,
  stream: web::Payload,
  data: web::Data<Arc<Server>>,
) -> Result<HttpResponse, actix_web::Error> {
  log::debug!("ws request from a user: {:?}", req);
  ws::start(
    WsSession {
      server: data.get_ref().clone(),
      hb_t: Time::now(),
      user_ctx: UserCtx {
        establish_t: Time::now(),
        user_agent: req
          .headers()
          .get("user-agent")
          .unwrap()
          .to_str()
          .unwrap()
          .to_string(),
        username: String::new(),
        token: String::new(),
        session: None,
      },
    },
    &req,
    stream,
  )
}

#[derive(serde::Deserialize, Clone)]
pub struct ServerConfig {
  pub loglevel: String,
  pub cwd: String,
  pub inner_addr: String,
  pub worker_num: i32,
  pub https: bool,
  pub managers: Vec<String>,
}

pub struct Server {
  pub config: RwLock<ServerConfig>,
  // map username to a list of UserCtx
  pub user_ctxs: RwLock<HashMap<String, Vec<UserCtx>>>,
  pub file_handler: FileHandler,
  pub dbpool: mysql::Pool,
}

impl Server {
  fn r_config(&self) -> ServerConfig {
    self.config.read().unwrap().clone()
  }

  fn r_is_manager(&self, username: &String) -> bool {
    self.r_config().managers.contains(&username)
  }

  fn r_user_ctxs(&self) -> HashMap<String, Vec<UserCtx>> {
    self.user_ctxs.read().unwrap().clone()
  }

  fn r_user_ctxs_by_username(&self, username: &String) -> Vec<UserCtx> {
    self
      .user_ctxs
      .read()
      .unwrap()
      .get(username)
      .unwrap()
      .clone()
  }

  fn r_user_ctxs_exclude_self(&self, user_ctx: &UserCtx) -> Vec<UserCtx> {
    let mut ctx_vec = self.r_user_ctxs_by_username(&user_ctx.username);
    let index = ctx_vec.iter().position(|x| *x == *user_ctx).unwrap();
    ctx_vec.remove(index);
    ctx_vec
  }

  // add a user_ctx to server state, must not exist
  fn w_add_user_ctx(&self, user_ctx: UserCtx) {
    let mut user_ctxs = self.user_ctxs.write().unwrap();
    if let Some(ctx_vec) = user_ctxs.get_mut(&user_ctx.username) {
      assert!(!ctx_vec.contains(&user_ctx), "must not exist");
      ctx_vec.push(user_ctx.clone());
    } else {
      assert!(user_ctxs
        .insert(user_ctx.username.clone(), vec![user_ctx.clone()])
        .is_none());
    }
  }

  // if removed return true, else false
  fn w_remove_user_ctx(&self, user_ctx: &UserCtx) -> bool {
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
          true
        }
        None => false,
      }
    } else {
      false
    }
  }
}

fn read_server_config() -> Option<ServerConfig> {
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

async fn start(server: Arc<Server>, use_config_thread: bool) -> std::io::Result<()> {
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
        .service(get_files)
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
        .service(get_files)
    })
    .bind(server_config.inner_addr)?
    .workers(server_config.worker_num as usize)
    .run()
    .await
  }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let server = Arc::new(Server {
    file_handler: FileHandler::new(),
    config: RwLock::new(read_server_config().unwrap()),
    user_ctxs: RwLock::new(HashMap::new()),
    dbpool: {
      if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
        mysql::Pool::new(url.as_str()).unwrap()
      } else {
        panic!("please set env PLUSEAR_DATABASE_URL");
      }
    },
  });
  start(server, true).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashSet;

  #[test]
  fn mysql_conn() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
      let pool = mysql::Pool::new(url.as_str())?;
      let _ = pool.get_conn()?;
    } else {
      return Err(Box::from("please set env PLUSEAR_DATABASE_URL"));
    }
    Ok(())
  }

  #[test]
  fn basic_test() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let set = HashSet::from([1, 3, 21]);
    let content: Vec<&i32> = set.iter().collect();
    assert!(content.contains(&&1));
    assert!(content.contains(&&3));
    assert!(content.contains(&&21));
    let mut map = HashMap::<i32, i32>::new();
    assert!(map.insert(3, 4).is_none());
    assert!(map.insert(3, 4).is_some());
    assert_eq!(map.get(&3).unwrap(), &4);
    Ok(())
  }

  #[test]
  fn sqlhandler() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
      let handler = SqlHandler {
        dbpool: mysql::Pool::new(url.as_str())?,
      };
      let name = String::from("userggh0");
      handler.delete_user_by_name(&name).unwrap_or(());
      assert!(handler.get_user_by_name(&name)?.is_none());

      let token = String::from("token0");
      let theme = String::from("dark");
      handler
        .add_user(&User {
          id: 0,
          username: name.clone(),
          token: token.clone(),
          config: UserConfig {
            id: 0,
            theme: theme.clone(),
          },
        })?
        .unwrap();
      assert!(handler.get_user_by_name(&name)?.is_some());

      let user = handler.get_user_by_name(&name)?.unwrap();
      assert_eq!(&name, &user.username);
      assert_eq!(&token, &user.token);
      assert_eq!(&theme, &user.config.theme);

      handler.update_user_config_by_name(
        &name,
        &UserConfig {
          id: 0,
          theme: String::from("light"),
        },
      )?;
      let user0 = handler.get_user_by_name(&name)?.unwrap();
      assert_eq!(&name, &user0.username);
      assert_eq!(&token, &user0.token);
      assert_eq!("light", &user0.config.theme);

      let name1 = String::from("userggh1");
      handler.delete_user_by_name(&name1).unwrap_or(());
      assert!(handler.get_user_by_name(&name1)?.is_none());
      let token = String::from("token1");
      let theme = String::from("dark");
      let user1 = handler
        .add_user(&User {
          id: 0,
          username: name1.clone(),
          token: token.clone(),
          config: UserConfig {
            id: 0,
            theme: theme.clone(),
          },
        })?
        .unwrap();
      assert!(handler.get_user_by_name(&name1)?.is_some());

      assert_eq!(
        handler
          .get_users()?
          .iter()
          .filter(|u| { *u == &user0 || *u == &user1 })
          .count(),
        2
      );

      handler.delete_user_by_name(&name1)?;
      assert_eq!(
        handler
          .get_users()?
          .iter()
          .filter(|u| { *u == &user0 || *u == &user1 })
          .count(),
        1
      );

      handler.delete_user_by_name(&name)?;
      assert_eq!(
        handler
          .get_users()?
          .iter()
          .filter(|u| { *u == &user0 || *u == &user1 })
          .count(),
        0
      );
    } else {
      return Err(Box::from("please set env PLUSEAR_DATABASE_URL"));
    }
    Ok(())
  }

  // mock a client call server's api
  struct ApiClient {
    addr: String,
    stream: TcpStream,
  }

  impl ApiClient {
    fn new(server_addr: &String) -> io::Result<Self> {
      Ok(Self {
        addr: server_addr.clone(),
        stream: TcpStream::connect(server_addr.to_string())?,
      })
    }

    // return server response
    fn index(&mut self) -> io::Result<String> {
      // TODO: add a more scaleable method to create a stream
      self.stream = TcpStream::connect(self.addr.to_string())?;

      let request = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", self.addr);
      self.stream.write_all(request.as_bytes())?;
      let mut response = String::new();
      self.stream.read_to_string(&mut response)?;
      Ok(response)
    }

    fn check_response_str(&self, resp: &String) -> bool {
      resp.contains("HTTP/1.1 200 OK")
    }

    fn login(&mut self, req: &LoginRequest) -> io::Result<LoginResponse> {
      self.stream = TcpStream::connect(self.addr.to_string())?;

      let json_body = serde_json::to_string(req).unwrap();
      let request = format!(
				"POST /login HTTP/1.1\r\nHost: localhost:9999\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
				json_body.len(),
				json_body
			);
      self.stream.write_all(request.as_bytes())?;
      let mut response = String::new();
      let _ = self.stream.read_to_string(&mut response)?;
      log::info!("Server LOGIN RESPONSE {response}");
      assert!(self.check_response_str(&response), "{response}");

      let (headers, body) = response.split_once("\r\n\r\n").ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid response",
      ))?;
      let headers = headers.trim_end();
      let content_length = headers
        .lines()
        .find(|line| line.starts_with("content-length"))
        .and_then(|line| {
          line
            .split_once(':')
            .and_then(|(_, v)| v.trim().parse().ok())
        })
        .unwrap_or(0);

      let json_body = body.get(..content_length).unwrap_or("");
      Ok(
        serde_json::from_str(json_body)
          .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
      )
    }

    fn check_login_resp(&self, resp: &LoginResponse) {
      match &resp.code {
        ResponseCode::Ok => (),
        ResponseCode::Err(e) => {
          assert!(false, "{e}")
        }
      }
    }

    fn logout(&mut self, req: &LogoutRequest) -> io::Result<LogoutResponse> {
      self.stream = TcpStream::connect(self.addr.to_string())?;

      let json_body = serde_json::to_string(req).unwrap();
      let request = format!(
				"POST /logout HTTP/1.1\r\nHost: localhost:9999\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
				json_body.len(),
				json_body
			);
      self.stream.write_all(request.as_bytes())?;
      let mut response = String::new();
      let _ = self.stream.read_to_string(&mut response)?;
      log::info!("Server LOGIN RESPONSE {response}");
      assert!(self.check_response_str(&response), "{response}");

      let (headers, body) = response.split_once("\r\n\r\n").ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid response",
      ))?;
      let headers = headers.trim_end();
      let content_length = headers
        .lines()
        .find(|line| line.starts_with("content-length"))
        .and_then(|line| {
          line
            .split_once(':')
            .and_then(|(_, v)| v.trim().parse().ok())
        })
        .unwrap_or(0);

      let json_body = body.get(..content_length).unwrap_or("");
      Ok(
        serde_json::from_str(json_body)
          .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
      )
    }

    fn check_logout_resp(&self, resp: &LogoutResponse) {
      match &resp.code {
        ResponseCode::Ok => (),
        ResponseCode::Err(e) => {
          assert!(false, "{e}")
        }
      }
    }
  }

  struct TestServer {
    server: Arc<Server>,
  }

  impl TestServer {
    fn new(loglevel: &String, addr: &String) -> Self {
      let server_config = ServerConfig {
        loglevel: loglevel.clone(),
        cwd: String::from("/home/wu/repository/pulsear"),
        inner_addr: addr.clone(),
        worker_num: 4,
        managers: vec![],
        https: false,
      };
      let server = Arc::new(Server {
        file_handler: FileHandler::new(),
        config: RwLock::new(server_config),
        user_ctxs: RwLock::new(HashMap::new()),
        dbpool: {
          if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
            mysql::Pool::new(url.as_str()).unwrap()
          } else {
            panic!("please set env PLUSEAR_DATABASE_URL");
          }
        },
      });
      return TestServer {
        server: server.clone(),
      };
    }

    async fn run(&self) {
      let server_ret = self.server.clone();
      actix_web::rt::spawn(async move { start(server_ret, false).await });
      actix_web::rt::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    fn current_online_user_ctx_by_name(&self, username: &String) -> Option<Vec<UserCtx>> {
      let user_ctxs = self.server.user_ctxs.read().unwrap();
      match user_ctxs.get(username) {
        Some(v) => Some(v.clone()),
        None => None,
      }
    }

    fn current_online_user_num_by_name(&self, username: &String) -> usize {
      match self.current_online_user_ctx_by_name(username) {
        Some(v) => v.len(),
        None => 0,
      }
    }
  }

  use std::io::{self, Read, Write};
  use std::net::TcpStream;
  #[actix_web::test]
  async fn indexhtml() -> std::io::Result<()> {
    let addr = "0.0.0.0:9999";
    let server = TestServer::new(&String::from("info"), &addr.to_string());
    server.run().await;
    let mut client = ApiClient::new(&addr.to_string())?;
    assert!(client.index()?.contains("HTTP/1.1 200 OK"));
    Ok(())
  }

  #[actix_web::test]
  async fn login_logout() -> std::io::Result<()> {
    let addr = "0.0.0.0:9999";
    let server = TestServer::new(&String::from("info"), &addr.to_string());
    server.run().await;
    let mut client = ApiClient::new(&addr.to_string())?;
    let username = String::from("test0");
    let login_request = LoginRequest {
      basic_info: StreamBasicInfo {
        time_stamp: Time::now().milli(),
      },
      login_info: LoginInfo {
        username: username.clone(),
        choice: LoginChoice::Password(username.clone()),
      },
    };
    let resp = client.login(&login_request)?;
    client.check_login_resp(&resp);
    assert_eq!(server.current_online_user_num_by_name(&username), 1);
    let resp = client.login(&login_request)?;
    client.check_login_resp(&resp);
    assert_eq!(server.current_online_user_num_by_name(&username), 2);

    let logout_request = LogoutRequest {
      basic_info: StreamBasicInfo {
        time_stamp: Time::now().milli(),
      },
      config: resp.config,
      username: username.clone(),
      token: resp.token.clone(),
    };
    let resp = client.logout(&logout_request)?;
    client.check_logout_resp(&resp);
    assert_eq!(server.current_online_user_num_by_name(&username), 1);
    let resp = client.logout(&logout_request)?;
    client.check_logout_resp(&resp);
    assert_eq!(server.current_online_user_num_by_name(&username), 0);
    Ok(())
  }
}
