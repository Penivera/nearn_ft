use actix_web::{post,HttpResponse,Responder};


#[post("/ft_transfer")]
async fn ft_transfer() -> impl Responder {
    HttpResponse::Ok().body("coming soon")
}
