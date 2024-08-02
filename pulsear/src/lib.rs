pub mod api;

#[derive(serde::Deserialize, serde::Serialize)]
enum ResponseCode {
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
pub struct LoginResponse {
    basic_info: StreamBasicInfo,
    user_ctx: UserCtx,
    code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutRequest {
    basic_info: StreamBasicInfo,
    user_ctx: UserCtx,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogoutResponse {
    basic_info: StreamBasicInfo,
    code: ResponseCode,
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
enum LoginChoice {
    Token(String),
    Password(String),
}

#[derive(serde::Deserialize, serde::Serialize, Hash)]
pub struct LoginInfo {
  username: String,
  choice: LoginChoice,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct UserCtx {
  username: String,
  token: String,
}

pub struct Server {
    user_ctxs: Vec<UserCtx>,
}

pub fn get_system_timestamp_milli() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    // 转换为UNIX时间戳（自1970年1月1日以来的秒数）
    let since_the_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    // 转换为毫秒
    since_the_epoch.as_millis() as u64    
}

fn gen_random_token<LoginInfo: std::hash::Hash>(login_info: &LoginInfo) -> String {
  use std::hash::{Hasher, DefaultHasher};
  let mut hasher = DefaultHasher::new();
  login_info.hash(&mut hasher);
  hasher.finish().to_string()
}
