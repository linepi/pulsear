use pulsear::*;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let server_config = read_server_config().unwrap();
  let server = Arc::new(Server {
    file_handler: FileHandler::new(server_config.file_worker_num),
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
  start(server, true).await
}
