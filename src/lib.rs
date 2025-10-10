extern crate serde;
pub mod types;
use actix_web::{
    Responder, post,
    web::{self, Data, Json},
};

use log::{error, info};
use near_api::*;
use near_sdk::AccountId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use types::{AccountConfig, TokenTransferRequest};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(ft_transfer), components(schemas(TokenTransferRequest)))]
pub struct ApiDoc;

#[utoipa::path(
    post,
    path = "/transfer",
    request_body = TokenTransferRequest,
    responses(
        (status = 200, description = "Fungible token transfer successful", body = String)
    )
)]
#[post("/transfer")]
pub async fn ft_transfer(
    payload: Json<TokenTransferRequest>,
    config: Data<AccountConfig>,
) -> impl Responder {
    let network: NetworkConfig = NetworkConfig::testnet();

    // Parse amount to FT balance (yoctoNEAR)
    let amount_raw = payload.amount.parse::<u128>().unwrap();
    let amount: FTBalance = FTBalance::with_decimals(24).with_amount(amount_raw);

    // Create transfer transaction
    let txn = Tokens::account(config.account_id.clone())
        .send_to(payload.reciever_id.clone())
        .ft(config.ft_contract_id.clone(), amount)
        .expect("Error Occured in getting FT")
        .with_signer(Arc::clone(&config.signer))
        .send_to(&network);

    match txn.await {
        Ok(result) => {
            // Handle success
            info!("Transfer successful: {:?}", result);
            "Transfer successful".to_string()
        }
        Err(e) => {
            // Handle error
            error!("Transfer failed: {:?}", e);
            format!("Transfer failed: {:?}", e)
        }
    }
}
