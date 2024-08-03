#[allow(unused_imports)]

use actix_web::rt::time;
use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use clap::Parser;
use mysql::params;
use mysql::prelude::*;
use mysql::TxOpts;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::sync::{Arc, RwLock};

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
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    login_info.hash(&mut hasher);
    hasher.finish().to_string()
}

pub fn load_user_config(req: &LoginRequest) -> UserConfig {
    return UserConfig {
		id: 3,
        theme: "dark".to_string(),
    };
}

pub fn store_user_config(req: &LogoutRequest) {}

struct SqlHandler;
impl SqlHandler {
	/// prerequisity: user_config table created
	/// returned users: with all field filled
	fn get_users(dbpool: &mysql::Pool)
		-> Result<Vec<User>, Box<dyn std::error::Error>> {
        let mut dbconn = dbpool.get_conn()?;
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
    fn get_user_by_name(dbpool: &mysql::Pool, username: &String) 
		-> Result<Option<User>, Box<dyn std::error::Error>> {
        let mut dbconn = dbpool.get_conn()?;
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
	fn add_user(dbpool: &mysql::Pool, user: &User) 
		-> Result<Option<User>, Box<dyn std::error::Error>> {
		match SqlHandler::get_user_by_name(dbpool, &user.username)? {
			Some(u) => return Err(Box::from(format!("user exists: {:?}", u))),
			None => ()
		}

        let mut dbconn = dbpool.start_transaction(TxOpts::default())?;
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
		SqlHandler::get_user_by_name(dbpool, &user.username)
    }

	fn delete_user_by_name(dbpool: &mysql::Pool, username: &String) 
		-> Result<(), Box<dyn std::error::Error>> {
		match SqlHandler::get_user_by_name(dbpool, username)? {
			Some(u) => log::info!("delete user[{:?}]", u),
			None => return Err(Box::from(format!("user does not exist: {}", username)))
		}

        let mut dbconn = dbpool.start_transaction(TxOpts::default())?;
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
		dbpool: &mysql::Pool, username: &String, config: &UserConfig)
		-> Result<(), Box<dyn std::error::Error>> {
		match SqlHandler::get_user_by_name(dbpool, username)? {
			Some(u) => log::info!("update user[{:?}]'s config as {:?}", u, config),
			None => return Err(Box::from(format!("user does not exist: {}", username)))
		}
        let mut dbconn = dbpool.get_conn()?;
		dbconn.exec_drop(
			r"UPDATE user_config SET theme=?
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )", (&config.theme, &username))?;
		Ok(())
	}
}

pub async fn index(data: web::Data<Arc<Server>>) -> HttpResponse {
    // like 192.168.31.126:4444
    let mut html_str = std::fs::read_to_string("pulsear-ui/ui/index.html").unwrap_or_else(|e| {
        println!("error: {} of index.html", e);
        e.to_string()
    });
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

#[post("/login")]
pub async fn login(param: web::Json<LoginRequest>, data: web::Data<Arc<Server>>) -> HttpResponse {
    log::info!("user try login: {}", serde_json::to_string(&param).unwrap());

    let username = param.login_info.username.clone();
    let login_response: LoginResponse;
    match &param.login_info.choice {
        LoginChoice::Token(token) => {
            login_response = LoginResponse {
                user_ctx: UserCtx {
                    username: username.clone(),
                    token: token.clone(),
                    state: UserCtxState::Conn,
                },
                basic_info: StreamBasicInfo {
                    time_stamp: get_system_timestamp_milli(),
                },
                config: load_user_config(&param.0),
                code: ResponseCode::Ok,
            };
        }
        LoginChoice::Password(_) => {
            login_response = LoginResponse {
                user_ctx: UserCtx {
                    username: username.clone(),
                    token: gen_random_token(&param.login_info),
                    state: UserCtxState::Conn,
                },
                basic_info: StreamBasicInfo {
                    time_stamp: get_system_timestamp_milli(),
                },
                config: load_user_config(&param.0),
                code: ResponseCode::Ok,
            };
        }
    }
    HttpResponse::Ok().json(login_response)
}

#[post("/logout")]
pub async fn logout(param: web::Json<LogoutRequest>, data: web::Data<Arc<Server>>) -> HttpResponse {
    log::info!("user logout: {}", serde_json::to_string(&param).unwrap());
    let logout_response = LogoutResponse {
        basic_info: StreamBasicInfo {
            time_stamp: get_system_timestamp_milli(),
        },
        code: ResponseCode::Ok,
    };
    store_user_config(&param.0);
    HttpResponse::Ok().json(logout_response)
}

#[derive(Parser)]
struct Args {}

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
    let server_config = read_server_config().unwrap();

    let server = Arc::new(Server {
        config: RwLock::new(server_config.clone()),
        user_ctxs: RwLock::new(vec![]),
        dbpool: {
            if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
                mysql::Pool::new(url.as_str()).unwrap()
            } else {
                panic!("please set env PLUSEAR_DATABASE_URL");
            }
        },
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

#[cfg(test)]
mod tests {
	use super::*;

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
    fn sqlhandler() -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
            let dbpool = mysql::Pool::new(url.as_str())?;
			let name = String::from("userggh0");
			SqlHandler::delete_user_by_name(&dbpool, &name).unwrap_or(());
			assert!(SqlHandler::get_user_by_name(&dbpool, &name)?.is_none());

			let token = String::from("token0");
			let theme = String::from("dark");
			SqlHandler::add_user(&dbpool, &User {
				id: 0,
				username: name.clone(),
				token: token.clone(),
				config: UserConfig {
					id: 0,
					theme: theme.clone()
				}
			})?.unwrap();
			assert!(SqlHandler::get_user_by_name(&dbpool, &name)?.is_some());

			let user = SqlHandler::get_user_by_name(&dbpool, &name)?.unwrap();
			assert_eq!(&name, &user.username);
			assert_eq!(&token, &user.token);
			assert_eq!(&theme, &user.config.theme);

			SqlHandler::update_user_config_by_name(&dbpool, &name, &UserConfig {
				id: 0,
				theme: String::from("light")
			})?;
			let user0 = SqlHandler::get_user_by_name(&dbpool, &name)?.unwrap();
			assert_eq!(&name, &user0.username);
			assert_eq!(&token, &user0.token);
			assert_eq!("light", &user0.config.theme);

			let name1 = String::from("userggh1");
			SqlHandler::delete_user_by_name(&dbpool, &name1).unwrap_or(());
			assert!(SqlHandler::get_user_by_name(&dbpool, &name1)?.is_none());
			let token = String::from("token1");
			let theme = String::from("dark");
			let user1 = SqlHandler::add_user(&dbpool, &User {
				id: 0,
				username: name1.clone(),
				token: token.clone(),
				config: UserConfig {
					id: 0,
					theme: theme.clone()
				}
			})?.unwrap();
			assert!(SqlHandler::get_user_by_name(&dbpool, &name1)?.is_some());

			assert_eq!(SqlHandler::get_users(&dbpool)?.iter().filter(|u| {
				*u == &user0 || *u == &user1
			}).count(), 2);

			SqlHandler::delete_user_by_name(&dbpool, &name1)?;
			assert_eq!(SqlHandler::get_users(&dbpool)?.iter().filter(|u| {
				*u == &user0 || *u == &user1
			}).count(), 1);

			SqlHandler::delete_user_by_name(&dbpool, &name)?;
			assert_eq!(SqlHandler::get_users(&dbpool)?.iter().filter(|u| {
				*u == &user0 || *u == &user1
			}).count(), 0);
        } else {
			return Err(Box::from("please set env PLUSEAR_DATABASE_URL"));
        }
        Ok(())
    }
}
