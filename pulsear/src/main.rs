use actix_web::{App, web, HttpServer};
use clap::Parser;
use pulsear::api;

#[derive(Parser)]
struct Args {

}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	HttpServer::new(|| {
      App::new()
      .route("/", web::get().to(api::index))
      .route("/index.html", web::get().to(api::index))
    })
		.bind(("127.0.0.1", 6543))?
		.run()
		.await
}

