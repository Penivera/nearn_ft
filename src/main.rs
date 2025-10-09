use actix_web::{App, HttpServer, middleware::Logger,web};
use nearn_ft::{ApiDoc, ft_transfer,AccountConfig};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use log::info;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize colored logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("🚀 Server starting at http://127.0.0.1:8000");
    info!("📚 Swagger UI available at http://127.0.0.1:8000/docs/");

    let config = AccountConfig::new();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config.clone()))
            .wrap(Logger::new(
                "%r %T",
            ))
            .service(ft_transfer)
            .service(
                SwaggerUi::new("/docs/{_:.*}")
                .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}
