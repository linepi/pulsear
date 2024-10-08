use crate::*;

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub enum ResponseCode {
  #[default]
  Ok,
  Err(String),
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct StreamBasicInfo {
  pub time_stamp: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginRequest {
  pub basic_info: StreamBasicInfo,
  pub login_info: LoginInfo,
}

#[derive(Default, std::fmt::Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum UserType {
  Master,
  Manager,
  Member,
  #[default]
  User,
  Visiter
}

impl UserType {
  pub fn from(s: &String) -> UserType {
    serde_json::from_str(&format!("\"{}\"", s)).unwrap() 
  }
}

impl ToString for UserType {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap().trim_matches('"').to_string()
  } 
}

pub struct UserRight {
  pub max_storage: u64
}

impl UserRight {
  pub fn from(t: UserType) -> Self {
    let  max_storage: u64;
    let one_g = 1024*1024*1024;
    match t {
      UserType::User => max_storage = one_g,
      UserType::Visiter => max_storage = 0,
      UserType::Manager => max_storage = 100*one_g,
      UserType::Master => max_storage = 1000*one_g,
      UserType::Member => max_storage = 10*one_g,
    }
    Self {
      max_storage
    }
  }
}

#[derive(std::fmt::Debug)]
pub struct User {
  pub id: i32,
  pub username: String,
  pub token: String,
  pub config: UserConfig,
  pub usertype: UserType,
}

impl PartialEq for User {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.username == other.username
      && self.token == other.token
      && self.config == other.config
  }
}
impl Eq for User {}

// map a path to its config
#[derive(serde::Deserialize, serde::Serialize, std::fmt::Debug, PartialEq, Clone)]
pub struct FileListConfig(HashMap<String, FileListPathConfig>);

#[derive(serde::Deserialize, serde::Serialize, std::fmt::Debug, Clone)]
pub struct FileListPathConfig {
  order_by: String,
  order_asc: bool,
  columns: Vec<String>
}

impl PartialEq for FileListPathConfig {
  fn eq(&self, other: &Self) -> bool {
    self.order_asc == other.order_asc && 
    self.order_by == other.order_by &&
    self.columns == other.columns
  }
}

#[derive(serde::Deserialize, serde::Serialize, std::fmt::Debug, Clone)]
pub struct UserConfig {
  pub id: i32,
  pub theme: String,
  pub filelist_config: FileListConfig,
  pub web_worker_num: i32
}

impl PartialEq for UserConfig {
  fn eq(&self, other: &Self) -> bool {
    self.theme == other.theme && 
    self.filelist_config ==  other.filelist_config && 
    self.web_worker_num == other.web_worker_num
  }
}
impl Eq for UserConfig {}


impl Default for UserConfig {
  fn default() -> Self {
    let columns = FileListElem::columns();
    Self {
      id: 0,
      theme: "dark".to_string(),
      web_worker_num: 4,
      filelist_config: FileListConfig(HashMap::from([
        (String::from("/"), FileListPathConfig {
          order_by: columns[0].clone(),
          columns,
          order_asc: true,
        }),
      ]))
    }
  }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct LoginResponse {
  pub basic_info: StreamBasicInfo,
  pub token: String,
  pub config: UserConfig,
  pub code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutRequest {
  pub basic_info: StreamBasicInfo,
  pub username: String,
  pub token: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutResponse {
  pub basic_info: StreamBasicInfo,
  pub code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum LoginChoice {
  Token(String),
  Password(String),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoginInfo {
  pub username: String,
  pub choice: LoginChoice,
}

fn get_user_token(param: &web::Json<LoginRequest>) -> String {
  match &(param.login_info).choice {
    LoginChoice::Token(token) => token.clone(),
    LoginChoice::Password(password) => 
      HashGenerator::new(format!("{}{}", &param.login_info.username, password)).token()
  }
}

pub fn do_login(
  param: &web::Json<LoginRequest>,
  data: &web::Data<Arc<Server>>,
) -> Result<LoginResponse, Err> {
  log::info!("user try login: {}", serde_json::to_string(param).unwrap());
  let sqlhandler = SqlHandler::new(data.dbpool.clone());

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
            usertype: UserType::default()
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


pub fn do_logout(
  param: &web::Json<LogoutRequest>,
  data: &web::Data<Arc<Server>>,
) -> Result<LogoutResponse, Err> {
  log::info!(
    "user try logout: {}",
    serde_json::to_string(&param).unwrap()
  );
  let sqlhandler = SqlHandler::new(data.dbpool.clone());

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
  Ok(logout_response)
}

#[cfg(test)]
mod tests {
  use super::*;
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
        file_worker_num: 4,
        sql_url: "mysql://wu:wi@localhost:3307/pulsear".to_string(),
      };
      let server = Arc::new(Server::from(server_config));
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

  #[test]
  fn usertype() {
    let t = UserType::Master;
    println!("{}", serde_json::to_string(&t).unwrap());
    let s = "\"Master\"";
    println!("{:?}", serde_json::from_str::<UserType>(s).unwrap());
  }
}
