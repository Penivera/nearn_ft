extern crate serde;
use dotenv::dotenv;
use near_api::Signer;
use near_sdk::AccountId;
use serde::{Deserialize, Serialize};
use std::{env, str::FromStr, sync::Arc};

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct TokenTransferRequest {
    #[schema(value_type = String)]
    pub reciever_id: AccountId,
    pub amount: String,
    pub memo: Option<String>,
}

#[derive(Clone)]
pub struct AccountConfig {
    pub account_id: AccountId,
    pub ft_contract_id: AccountId,
    pub signer: Arc<Signer>,
}

pub struct Settings {
    pub ft_contract_id: String,
    pub account_id: String,
    pub private_key: String,
}

impl Settings {
    pub fn new() -> Result<Self, env::VarError> {
        dotenv().ok();
        let ft_contract_id = env::var("FT_CONTRACT_ID")?;
        let account_id = env::var("NEAR_ACCOUNT_ID")?;
        let private_key = env::var("NEAR_PRIVATE_KEY")?;
        Ok(Self {
            ft_contract_id,
            account_id,
            private_key,
        })
    }

    pub fn into_account_config(self) -> Result<AccountConfig, Box<dyn std::error::Error>> {
        let account_id = AccountId::from_str(&self.account_id)
            .map_err(|e| format!("Failed to parse NEAR_ACCOUNT_ID: {}", e))?;

        let ft_contract_id = AccountId::from_str(&self.ft_contract_id)
            .map_err(|e| format!("Failed to parse FT_CONTRACT_ID: {}", e))?;

        let signer_from_seed = Signer::from_seed_phrase(&self.private_key, None)
            .map_err(|e| format!("Failed to parse NEAR_PRIVATE_KEY: {}", e))?;

        let signer: Arc<Signer> =
            Signer::new(signer_from_seed).map_err(|e| format!("Failed to create signer: {}", e))?;

        Ok(AccountConfig {
            account_id,
            ft_contract_id,
            signer,
        })
    }
}
