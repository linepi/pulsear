use pulsear::*;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let server_config = read_server_config().unwrap();
  let server = Arc::new(Server::from(server_config));
  start(server, true).await
}
