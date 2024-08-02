use actix_web::{App, web, HttpServer};
use clap::Parser;
use pulsear::api;

#[derive(Parser)]
struct Args {

}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
	HttpServer::new(|| {
      App::new()
      .wrap(actix_web::middleware::Logger::default())
      .route("/", web::get().to(api::index))
      .route("/index.html", web::get().to(api::index))
      .service(api::resources)
      .service(api::login)
      .service(api::logout)
    })
		.bind(("0.0.0.0", 6543))?
    .workers(4)
		.run()
		.await
}

