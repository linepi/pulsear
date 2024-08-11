use crate::*;

pub async fn ws(
  req: HttpRequest,
  stream: web::Payload,
  data: web::Data<Arc<Server>>,
) -> Result<HttpResponse, actix_web::Error> {
  log::info!("ws request from a user: {:?}", req);
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

#[post("/download_raw")]
pub async fn download_raw(param: web::Json<DownloadRequest>, data: web::Data<Arc<Server>>) 
  -> Result<NamedFile, Err> {
  let sqlhandler = SqlHandler::new(data.dbpool.clone());
  match sqlhandler.get_user_by_name(&param.username)? {
    Some(u) => {
      assert_eq!(&u.username, &param.username);
      assert_eq!(&u.token, &param.token);
    }
    None => {
      return Err(Box::from("user not exists"));
    }
  };
  let storage = std::path::PathBuf::from("inner/storage");
  let userfile_path = storage.join(&param.username).join(&param.name);
  Ok(NamedFile::open(userfile_path)?)
}

#[post("/get_download_url")]
pub async fn get_download_url(param: web::Json<DownloadRequest>, data: web::Data<Arc<Server>>) 
  -> Result<HttpResponse, Err> {
  let sqlhandler = SqlHandler::new(data.dbpool.clone());
  match sqlhandler.get_user_by_name(&param.username)? {
    Some(u) => {
      assert_eq!(&u.username, &param.username);
      assert_eq!(&u.token, &param.token);
    }
    None => {
      return Err(Box::from("user not exists"));
    }
  };
  let code = data.file_handler.gen_download_code(param.into_inner());
  Ok(HttpResponse::Ok().body(code))
}

#[get("/download/{username}/{code}")]
pub async fn download_by_url(p: web::Path<(String, String)>, data: web::Data<Arc<Server>>) 
  -> Result<NamedFile, Err> {
  log::info!("download by url: download/{}/{}", p.as_ref().0, p.as_ref().1);
  let param: (String, String) = p.into_inner();
  let username = param.0;
  let code = param.1;
  let filename = match data.file_handler.from_download_code(&code) {
    Some(p) => {
      if username != p.0 {
        return Err(Box::from("unexpected"));
      }
      p.1
    },
    None => {
      return Err(Box::from("unexpected"));
    }
  };
  let storage = std::path::PathBuf::from("inner/storage");
  let userfile_path = storage.join(&username).join(&filename);
  Ok(NamedFile::open(userfile_path)?)
}




#[post("/files")]
pub async fn get_file_list(param: web::Json<FileListRequest>, data: web::Data<Arc<Server>>) -> HttpResponse {
  log::info!("user try get file list: {}", serde_json::to_string(&param).unwrap());
  let resp: HttpResponse;
  match do_get_file_list(param, data) {
    Ok(response) => resp = response,
    Err(e) => {
      resp = HttpResponse::BadRequest().body(e.to_string());
    }
  }
  log::debug!("Server get file resp with {:?}", resp);
  resp 
}

#[post("/file")]
pub async fn get_file_elem(param: web::Json<FileElemRequest>, data: web::Data<Arc<Server>>) 
  -> Result<HttpResponse, Err> {
  log::info!("user try get file elem: {}", serde_json::to_string(&param).unwrap());
  let sqlhandler = SqlHandler::new(data.dbpool.clone());
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
  let userfile = storage.join(&user.username).join(&param.name);
  let file = std::fs::File::open(userfile)?;
  Ok(HttpResponse::Ok().body(serde_json::to_string(
    &FileListElem::from_name_and_metadata(param.into_inner().name, file.metadata()?)?
  )?))
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
  let res = match std::fs::read_to_string(&path) {
    Ok(content) => content,
    Err(e) => {
        log::error!("Error reading {}: {}", path, e);
        return HttpResponse::InternalServerError().body("Internal Server Error");
    }
  };
  let mut builder = HttpResponse::Ok();
  builder
    .append_header(("Cache-Control", "no-cache, no-store, must-revalidate")) // Prevent caching
    .append_header(("Pragma", "no-cache")) // For older HTTP/1.0 clients
    .append_header(("Expires", "0")); // Proxies
  if path.ends_with(".js") {
    builder.append_header(("Content-Type", "application/javascript"));
  } else if path.ends_with(".css") {
    builder.append_header(("Content-Type", "text/css"));
  }
  builder.body(res)
}

