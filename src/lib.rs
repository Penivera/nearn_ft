pub mod config;
pub mod types;
pub mod worker;

use actix_web::{
    post,
    web::{Data, Json},
    HttpResponse, Responder,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use types::TokenTransferRequest;
use utoipa::OpenApi;

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct TransferResponse {
    pub success: bool,
    pub message: String,
}

#[derive(OpenApi)]
#[openapi(paths(ft_transfer), components(schemas(TokenTransferRequest, TransferResponse)))]
pub struct ApiDoc;

#[utoipa::path(
    post,
    path = "/transfer",
    request_body = TokenTransferRequest,
    responses(
        (status = 202, description = "Transfer request accepted for processing", body = TransferResponse),
        (status = 400, description = "Invalid input", body = TransferResponse),
        (status = 500, description = "Internal server error", body = TransferResponse)
    )
)]
#[post("/transfer")]
pub async fn ft_transfer(
    payload: Json<TokenTransferRequest>,
    sender: Data<Sender<TokenTransferRequest>>,
) -> impl Responder {
    // Basic validation
    if payload.reciever_id.to_string().is_empty() || payload.amount.parse::<u128>().is_err() {
        return HttpResponse::BadRequest().json(TransferResponse {
            success: false,
            message: "Invalid receiver_id or amount".to_string(),
        });
    }

    match sender.send(payload.into_inner()).await {
        Ok(_) => HttpResponse::Accepted().json(TransferResponse {
            success: true,
            message: "Transfer request accepted and queued for processing.".to_string(),
        }),
        Err(_) => HttpResponse::InternalServerError().json(TransferResponse {
            success: false,
            message: "Failed to queue transfer request.".to_string(),
        }),
    }
}