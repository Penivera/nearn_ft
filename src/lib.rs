extern crate serde;
use actix_web::{
    Responder, post,
    web::{self, Json},
};
use dotenv::dotenv;
use near_api::{NearToken, NetworkConfig, Tokens, Signer};
use near_sdk::AccountId;
use serde::{Deserialize, Serialize};
use std::{env, str::FromStr, sync::Arc};
use utoipa::OpenApi;
use log::{error,info};

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
struct TokenTransferRequest {
    #[schema(value_type = String)]
    reciever_id: AccountId,
    amount: String,
    memo: Option<String>,
}

#[derive(Clone)]
pub struct AccountConfig {
    account_id: AccountId,
    signer: Arc<Signer>,
}



impl AccountConfig {
    pub fn new() -> Self {
        dotenv().ok();
        Self {
            account_id: AccountId::from_str(
                &env::var("NEAR_ACCOUNT_ID").expect("NEAR_ACCOUNT_ID NOT FOUND"),
            )
            .expect("Failed to parse NEAR_ACCOUNT_ID"),
            signer: Signer::new(Signer::from_seed_phrase(
                &env::var("NEAR_PRIVATE_KEY").expect("NEAR_PRIVATE_KEY NOT FOUND"),
                None,
            )
            .expect("Failed to parse Secret key")).unwrap(),
        }
    }
}

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
    state: web::Data<AccountConfig>,
) -> impl Responder {
    let network: NetworkConfig = NetworkConfig::testnet();

    // Parse amount to NearToken
    let amount = NearToken::from_yoctonear(payload.amount.parse::<u128>().unwrap());

    // Create transfer transaction
    let txn = Tokens::account(state.account_id.clone())
        .send_to(payload.reciever_id.clone())
        .near(amount)
        .with_signer(Arc::clone(&state.signer))
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
