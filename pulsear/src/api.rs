use actix_web::{get, post, web, HttpResponse, HttpRequest};
use crate::*;

pub async fn index(req: HttpRequest) -> HttpResponse {
  // like 192.168.31.126:4444
  let my_ipaddr = req.headers().get("host").unwrap().to_str().unwrap().to_string();
  log::info!("my_ipaddr: {}", my_ipaddr);
  let mut html_str = std::fs::read_to_string("pulsear-ui/ui/index.html")
                    .unwrap_or_else(|e| {
                        println!("error: {} of index.html", e);
                        e.to_string()
                    });
  // replace the ipaddr in html_str to actual one 
  html_str = html_str.replace("giocdanewla", format!("http://{}/", &my_ipaddr).as_str());
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
          token: gen_random_token(&param.login_info)
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
