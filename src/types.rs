extern crate serde;
use near_sdk::AccountId;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct TokenTransferRequest {
    #[schema(value_type = String)]
    pub reciever_id: AccountId,
    pub amount: String,
    pub memo: Option<String>,
}
