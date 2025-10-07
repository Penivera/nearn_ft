use actix_web::{App,HttpServer};
use nearn_actix::ft_transfer;

#[actix_web::main]
async fn main() -> std::io::Result<()>{
    HttpServer::new(||{
        App::new().service(ft_transfer)
    }).bind(("0.0.0.0",8000))?.run().await
}
