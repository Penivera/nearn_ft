extern crate serde;
pub mod types;

use std::fs::File;
use std::io::Write;
use actix_web::{
    Responder, post,
    web::{Data, Json},
};

use log::{error, info};
use near_api::*;
use std::sync::Arc;
use types::{AccountConfig, TokenTransferRequest};
use utoipa::OpenApi;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct TransferResponse {
    pub success: bool,
    pub txn_hash: String,
    pub sender: String,
    pub receiver: String,
    pub token_id: Option<String>,
    pub amount: String,
    pub status: String,
    pub gas_burnt: u64,
    pub tokens_burnt: String,
    pub logs: Vec<String>,
    pub receipt_ids: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(OpenApi)]
#[openapi(paths(ft_transfer), components(schemas(TokenTransferRequest, TransferResponse)))]
pub struct ApiDoc;

#[utoipa::path(
    post,
    path = "/transfer",
    request_body = TokenTransferRequest,
    responses(
        (status = 200, description = "Fungible token transfer result", body = TransferResponse)
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
    let amount: FTBalance = FTBalance::with_decimals(config.ft_decimals).with_amount(amount_raw);

    // Create transfer transaction
    let txn = Tokens::account(config.account_id.clone())
        .send_to(payload.reciever_id.clone())
        .ft(config.ft_contract_id.clone(), amount)
        .expect("Error Occured in getting FT")
        .with_signer(Arc::clone(&config.signer))
        .send_to(&network);

    match txn.await {
        Ok(result) => {
            // Extract relevant transaction information
            let txn_hash = result.transaction.hash.to_string();
            let status = format!("{:?}", result.status);
            let gas_burnt = result.transaction_outcome.outcome.gas_burnt;
            let tokens_burnt = result.transaction_outcome.outcome.tokens_burnt.to_string();
            let logs = result.transaction_outcome.outcome.logs.clone();
            let receipt_ids = result.transaction_outcome.outcome.receipt_ids.iter().map(|id| id.to_string()).collect();

            // Save full result to file for debugging
            let data = to_string_pretty(&result);
            let mut file = File::create("result.json").unwrap();
            file.write_all(data.unwrap().as_bytes()).unwrap();

            info!("Transfer successful: {:?}", result);

            // Return structured response
            Json(TransferResponse {
                success: true,
                txn_hash,
                sender: config.account_id.to_string(),
                receiver: payload.reciever_id.to_string(),
                token_id: Some(config.ft_contract_id.to_string()),
                amount: payload.amount.clone(),
                status,
                gas_burnt,
                tokens_burnt,
                logs,
                receipt_ids,
                error_message: None,
            })
        }
        Err(e) => {
            error!("Transfer failed: {:?}", e);
            // Return error response
            Json(TransferResponse {
                success: false,
                txn_hash: "".to_string(),
                sender: config.account_id.to_string(),
                receiver: payload.reciever_id.to_string(),
                token_id: Some(config.ft_contract_id.to_string()),
                amount: payload.amount.clone(),
                status: "Failed".to_string(),
                gas_burnt: 0,
                tokens_burnt: "0".to_string(),
                logs: vec![],
                receipt_ids: vec![],
                error_message: Some(format!("{:?}", e)),
            })
        }
    }
}
