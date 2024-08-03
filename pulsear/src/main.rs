#[allow(unused_imports)]

use actix_web::rt::time;
use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use clap::Parser;
use mysql::params;
use mysql::prelude::*;
use mysql::TxOpts;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

use std::hash::Hash;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

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
        self.id == other.id && self.username == other.username &&
		self.token == other.token && self.config == other.config
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
			theme: "dark".to_string()
		}
	}
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
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
	s: String
}

#[derive(serde::Deserialize, serde::Serialize, Default, Clone)]
#[derive(PartialEq, Eq, Hash)]
pub enum UserCtxState {
	#[default]
    Conn,
    Disconn,
}

#[derive(serde::Deserialize, serde::Serialize, Default, Clone)]
#[derive(PartialEq, Eq, Hash)]
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

pub fn gen_random_token<TokenGenerator: std::hash::Hash>(gen: &TokenGenerator) -> String {
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
	gen.hash(&mut hasher);
    hasher.finish().to_string()
}

pub fn get_user_token(param: &web::Json<LoginRequest>) -> String {
	match &(param.login_info).choice {
		LoginChoice::Token(token) => {
			token.clone()
		},
		LoginChoice::Password(password) => {
			gen_random_token::<TokenGenerator>(&TokenGenerator {
				s: format!("{}{}", &param.login_info.username, password)
			})
		}
	}
}

/// should only be used by one thread
struct SqlHandler {
	dbpool: mysql::Pool
}
impl SqlHandler {
	/// prerequisity: user_config table created
	/// returned users: with all field filled
	fn get_users(&self)
		-> Result<Vec<User>, Box<dyn std::error::Error>> {
        let mut dbconn = self.dbpool.get_conn()?;
		let mut users: Vec<User> = vec![];
        dbconn.query_map(
			r"SELECT user.id, username, token, theme, user_config.id 
			  from user, user_config 
			  where user.id = user_config.user_id", |row| {
				let elems: (i32, String, String, String, i32) = row;
				let user = User {
					id: elems.0,
					username: elems.1,
					token: elems.2,
					config: UserConfig {
						id: elems.4,
						theme: elems.3
					}
				};
				users.push(user);
			})?;
		Ok(users)
	}

	/// prerequisity: user_config table created
	/// returned user: with all field filled
    fn get_user_by_name(&self, username: &String) 
		-> Result<Option<User>, Box<dyn std::error::Error>> {
        let mut dbconn = self.dbpool.get_conn()?;
        let stmt = dbconn.prep(
			r"SELECT user.id, username, token, theme, user_config.id 
			  from user, user_config 
			  where user.id = user_config.user_id and
				    username = :name")?;
        let rows: Vec<mysql::Row> = dbconn.exec(&stmt, params! { "name" => &username })?;
		if rows.len() == 0 {
			return Ok(None);
		} else if rows.len() > 1 {
			return Err(Box::from("multiple use found"));
		}
		let row: (i32, String, String, String, i32) = mysql::from_row_opt(
			rows.first().unwrap().to_owned())?;
		let user = User {
			id: row.0,
			username: row.1,
			token: row.2,
			config: UserConfig {
				id: row.4,
				theme: row.3
			}
		};
		Ok(Some(user))
    }

	/// user: username, token, config
	/// returned user: id, ..., config_id
	fn add_user(&self, user: &User) 
		-> Result<Option<User>, Box<dyn std::error::Error>> {
		match self.get_user_by_name(&user.username)? {
			Some(u) => return Err(Box::from(format!("user exists: {:?}", u))),
			None => ()
		}

        let mut dbconn = self.dbpool.start_transaction(TxOpts::default())?;
        let stmt = dbconn.prep(
			r"INSERT INTO user(username, token)
			  VALUES (:username, :token)")?;
        dbconn.exec_drop(&stmt, params! { "username" => &user.username, "token" => &user.token })?;
        let user_id: i32 = dbconn.exec_first(
			r"SELECT id from user
			  WHERE username = ?", (&user.username,))?.expect("user should exists after insertion");

        let stmt = dbconn.prep(
			r"INSERT INTO user_config(user_id, theme)
			  VALUES (:user_id, :theme)")?;
        dbconn.exec_drop(&stmt, params! { "user_id" => &user_id, "theme" => &user.config.theme })?;
		dbconn.commit()?;
		self.get_user_by_name(&user.username)
    }

	fn delete_user_by_name(&self, username: &String) 
		-> Result<(), Box<dyn std::error::Error>> {
		match self.get_user_by_name(username)? {
			Some(u) => log::info!("delete user[{:?}]", u),
			None => return Err(Box::from(format!("user does not exist: {}", username)))
		}

        let mut dbconn = self.dbpool.start_transaction(TxOpts::default())?;
		dbconn.exec_drop(
			r"DELETE FROM user_config 
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )", (username,))?;
		dbconn.exec_drop(
			r"DELETE FROM user 
			  WHERE username = ?", (username,))?;
		dbconn.commit()?;
		Ok(())
	}

	fn update_user_config_by_name(
		&self, username: &String, config: &UserConfig)
		-> Result<(), Box<dyn std::error::Error>> {
		match self.get_user_by_name(username)? {
			Some(u) => log::info!("update user[{:?}]'s config as {:?}", u, config),
			None => return Err(Box::from(format!("user does not exist: {}", username)))
		}
        let mut dbconn = self.dbpool.get_conn()?;
		dbconn.exec_drop(
			r"UPDATE user_config SET theme=?
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )", (&config.theme, &username))?;
		Ok(())
	}

	/// change last login time
	fn user_login(&self, username: &String) 
		-> Result<(), Box<dyn std::error::Error>> {
		match self.get_user_by_name(username)? {
			Some(u) => log::info!("update user[{:?}]'s login time", u),
			None => return Err(Box::from(format!("user does not exist: {}", username)))
		}
        let mut dbconn = self.dbpool.get_conn()?;
		dbconn.exec_drop(
			r"UPDATE user SET last_login_time=NOW()
			  WHERE username = ?", (&username,))?;
		Ok(())
	}
}

pub async fn index(data: web::Data<Arc<Server>>) -> HttpResponse {
    // like 192.168.31.126:4444
    let mut html_str = match std::fs::read_to_string("pulsear-ui/ui/index.html") {
		Ok(s) => s,
		Err(e) => {
			let errmsg = format!("error: {} of index.html", e);
			log::info!("{}", &errmsg);
			return HttpResponse::InternalServerError().body(errmsg);
		}
    };
    // replace the ipaddr in html_str to actual one
    html_str = html_str.replace(
        "giocdanewla",
        format!("https://{}/", data.config.read().unwrap().outer_addr).as_str(),
    );
    HttpResponse::Ok().body(html_str)
}

#[get("/resources/{path}")]
pub async fn resources(path: web::Path<String>) -> HttpResponse {
    log::info!("visit path {}", path);
    let res = std::fs::read_to_string(format!("pulsear-ui/ui/{}", path)).unwrap_or_else(|e| {
        println!("error: {} of {}", e, path);
        e.to_string()
    });
    HttpResponse::Ok().body(res)
}

pub fn do_login(param: &web::Json<LoginRequest>, data: &web::Data<Arc<Server>>) 
	-> Result<LoginResponse, Box<dyn std::error::Error>> {
    log::info!("user try login: {}", serde_json::to_string(param).unwrap());
	let sqlhandler = SqlHandler {
		dbpool: data.dbpool.clone()
	};

	let token = get_user_token(param);
	let user_ctx: UserCtx;
	let user: User;
	{ // lock begin
		let mut user_ctxs = data.user_ctxs.write().unwrap();
		user = match sqlhandler.get_user_by_name(&param.login_info.username)? {
			Some(u) => {
				if &u.token != &token {
					return Err(Box::from("password not true or is has been changed"));
				}
				u
			},
			None => {
				let u = sqlhandler.add_user(&User {
					id: 0,
					username: param.login_info.username.clone(),
					token: token.clone(),
					config: UserConfig::default()
				})?.expect("add user error!");
				assert_eq!(&u.token, &token);
				assert_eq!(&u.username, &param.login_info.username);
				assert_eq!(&u.config, &UserConfig::default());
				u
			}
		};

		user_ctx = UserCtx {
			username: user.username.clone(),
			token: user.token,
			state: UserCtxState::Conn,
		};
		// NOTE: can login twice
		if let Some(ctx_vec) = user_ctxs.get_mut(&user.username) {
			ctx_vec.push(user_ctx.clone());
		} else {
			assert!(user_ctxs.insert(user.username.clone(), vec![user_ctx.clone()]).is_none());
		}
	} // lock end

	let login_response = LoginResponse {
		user_ctx: user_ctx,
		basic_info: StreamBasicInfo {
			time_stamp: get_system_timestamp_milli(),
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
	log::info!("Server resp with {:?}", resp);
	resp
}

pub fn do_logout(param: &web::Json<LogoutRequest>, data: &web::Data<Arc<Server>>)
	-> Result<LogoutResponse, Box<dyn std::error::Error>> {
    log::info!("user try logout: {}", serde_json::to_string(&param).unwrap());
	let sqlhandler = SqlHandler {
		dbpool: data.dbpool.clone()
	};

	match sqlhandler.get_user_by_name(&param.user_ctx.username)? {
		Some(u) => {
			assert_eq!(&u.username, &param.user_ctx.username);
			assert_eq!(&u.token, &param.user_ctx.token);
			u
		},
		None => {
			return Err(Box::from("logout user not exists"));
		}
	};

    let logout_response = LogoutResponse {
        basic_info: StreamBasicInfo {
            time_stamp: get_system_timestamp_milli(),
        },
        code: ResponseCode::Ok,
    };
	let mut user_ctxs = data.user_ctxs.write().unwrap();
	let ctx_vec = user_ctxs.get_mut(&param.user_ctx.username).expect("logout with no user_ctx");
	// now do not have other information, just remove the first
	ctx_vec.remove(0);
	sqlhandler.update_user_config_by_name(&param.user_ctx.username, &param.config)?;
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
	log::info!("Server resp with {:?}", resp);
	resp
}

#[derive(Parser)]
struct Args {}

#[derive(serde::Deserialize, Clone)]
pub struct ServerConfig {
	pub loglevel: String,
	pub cwd: String,
    pub outer_addr: String,
    pub inner_addr: String,
    pub worker_num: i32,
    pub https: bool,
}

pub struct Server {
    pub config: RwLock<ServerConfig>,
	// map username to a list of UserCtx
    pub user_ctxs: RwLock<HashMap<String, Vec<UserCtx>>>,
    pub dbpool: mysql::Pool,
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

async fn start(
	server: Arc<Server>,
	use_config_thread: bool
) -> std::io::Result<()> {
	let server_config = { server.config.read().unwrap().clone() };
    env_logger::init_from_env(env_logger::Env::new().default_filter_or(server_config.loglevel));
	// set current cwd to project root such that the static file path work find
	let cwd = std::path::Path::new(&server_config.cwd);
    if std::env::set_current_dir(cwd).is_err() {
        panic!("Failed to change directory");
    } else {
		log::info!("cwd has been set to {}", std::env::current_dir().unwrap().display());
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let server = Arc::new(Server {
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
		let set = HashSet::from([1,3,21]);
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
				dbpool: mysql::Pool::new(url.as_str())?
			};
			let name = String::from("userggh0");
			handler.delete_user_by_name(&name).unwrap_or(());
			assert!(handler.get_user_by_name(&name)?.is_none());

			let token = String::from("token0");
			let theme = String::from("dark");
			handler.add_user(&User {
				id: 0,
				username: name.clone(),
				token: token.clone(),
				config: UserConfig {
					id: 0,
					theme: theme.clone()
				}
			})?.unwrap();
			assert!(handler.get_user_by_name(&name)?.is_some());

			let user = handler.get_user_by_name(&name)?.unwrap();
			assert_eq!(&name, &user.username);
			assert_eq!(&token, &user.token);
			assert_eq!(&theme, &user.config.theme);

			handler.update_user_config_by_name(&name, &UserConfig {
				id: 0,
				theme: String::from("light")
			})?;
			let user0 = handler.get_user_by_name(&name)?.unwrap();
			assert_eq!(&name, &user0.username);
			assert_eq!(&token, &user0.token);
			assert_eq!("light", &user0.config.theme);

			let name1 = String::from("userggh1");
			handler.delete_user_by_name(&name1).unwrap_or(());
			assert!(handler.get_user_by_name(&name1)?.is_none());
			let token = String::from("token1");
			let theme = String::from("dark");
			let user1 = handler.add_user(&User {
				id: 0,
				username: name1.clone(),
				token: token.clone(),
				config: UserConfig {
					id: 0,
					theme: theme.clone()
				}
			})?.unwrap();
			assert!(handler.get_user_by_name(&name1)?.is_some());

			assert_eq!(handler.get_users()?.iter().filter(|u| {
				*u == &user0 || *u == &user1
			}).count(), 2);

			handler.delete_user_by_name(&name1)?;
			assert_eq!(handler.get_users()?.iter().filter(|u| {
				*u == &user0 || *u == &user1
			}).count(), 1);

			handler.delete_user_by_name(&name)?;
			assert_eq!(handler.get_users()?.iter().filter(|u| {
				*u == &user0 || *u == &user1
			}).count(), 0);
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
				stream: TcpStream::connect(server_addr.to_string())?
			})
		}

		// return server response
		fn index(&mut self) -> io::Result<String> {
			// TODO: add a more scaleable method to create a stream
			self.stream = TcpStream::connect(self.addr.to_string())?;

			let request = format!(
				"GET / HTTP/1.1\r\nHost: {}\r\n\r\n", self.addr
			);
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

			let (headers, body) = response
				.split_once("\r\n\r\n")
				.ok_or(io::Error::new(io::ErrorKind::InvalidData, "Invalid response"))?;
			let headers = headers.trim_end(); 
			let content_length = headers.lines()
				.find(|line| line.starts_with("content-length"))
				.and_then(|line| line.split_once(':').and_then(|(_, v)| v.trim().parse().ok()))
				.unwrap_or(0);

			let json_body = body.get(..content_length).unwrap_or("");
			Ok(serde_json::from_str(json_body)
				.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?)
		}

		fn check_login_resp(&self, resp: &LoginResponse) {
			match &resp.code {
				ResponseCode::Ok => (),
				ResponseCode::Err(e) => { assert!(false, "{e}") }
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

			let (headers, body) = response
				.split_once("\r\n\r\n")
				.ok_or(io::Error::new(io::ErrorKind::InvalidData, "Invalid response"))?;
			let headers = headers.trim_end(); 
			let content_length = headers.lines()
				.find(|line| line.starts_with("content-length"))
				.and_then(|line| line.split_once(':').and_then(|(_, v)| v.trim().parse().ok()))
				.unwrap_or(0);

			let json_body = body.get(..content_length).unwrap_or("");
			Ok(serde_json::from_str(json_body)
				.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?)
		}

		fn check_logout_resp(&self, resp: &LogoutResponse) {
			match &resp.code {
				ResponseCode::Ok => (),
				ResponseCode::Err(e) => { assert!(false, "{e}") }
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
				outer_addr: addr.clone(),
				inner_addr: addr.clone(),
				worker_num: 4,
				https: false,
			};
			let server = Arc::new(Server {
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
				server: server.clone()
			}
		}

		async fn run(&self) {
			let server_ret = self.server.clone();
			actix_web::rt::spawn(async move {
				start(server_ret, false).await	
			});
			actix_web::rt::time::sleep(std::time::Duration::from_millis(300)).await;
		}

		fn current_online_user_ctx_by_name(&self, username: &String) -> Option<Vec<UserCtx>> {
			let user_ctxs = self.server.user_ctxs.read().unwrap();
			match user_ctxs.get(username) {
				Some(v) => Some(v.clone()),
				None => None
			}
		}

		fn current_online_user_num_by_name(&self, username: &String) -> usize {
			match self.current_online_user_ctx_by_name(username) {
				Some(v) => v.len(),
				None => 0
			}
		}
	}

	use std::io::{self, Write, Read};
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
				time_stamp: get_system_timestamp_milli()
			},
			login_info: LoginInfo {
				username: username.clone(),
				choice: LoginChoice::Password(username.clone()),
			}
		};
		let resp = client.login(&login_request)?;
		client.check_login_resp(&resp);
		assert_eq!(server.current_online_user_num_by_name(&username), 1);
		let resp = client.login(&login_request)?;
		client.check_login_resp(&resp);
		assert_eq!(server.current_online_user_num_by_name(&username), 2);

		let logout_request = LogoutRequest {
			basic_info: StreamBasicInfo {
				time_stamp: get_system_timestamp_milli()
			},
			config: resp.config,
			user_ctx: resp.user_ctx.clone()
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
