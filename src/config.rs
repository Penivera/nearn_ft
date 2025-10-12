use dotenv::dotenv;
use serde::Deserialize;
use std::{env, fs};

// This struct maps directly to the Settings.toml file
#[derive(Deserialize)]
struct FileSettings {
    pub rpc_urls: Vec<String>,
    pub ft_contract_id: String,
    pub account_id: String,
    pub ft_decimals: u8,
    pub batch_size: usize,
    pub batch_timeout_secs: u64,
    pub concurrency: usize,
    pub num_pool_keys: usize,
    pub key_allowance_near: f64,
    pub network: String,
}

// This is the final, complete Settings struct for the application
#[derive(Clone)]
pub struct Settings {
    pub rpc_urls: Vec<String>,
    pub ft_contract_id: String,
    pub account_id: String,
    pub master_key: String, // Loaded from .env
    pub ft_decimals: u8,
    pub batch_size: usize,
    pub batch_timeout_secs: u64,
    pub concurrency: usize,
    pub num_pool_keys: usize,
    pub key_allowance_near: f64,
    pub network: String,
    pub redis_url: String,
}

impl Settings {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load settings from Settings.toml
        let config_str = fs::read_to_string("Settings.toml")?;
        let file_settings: FileSettings = toml::from_str(&config_str)?;

        // Load .env file for the master key
        dotenv().ok();
        let master_key = env::var("NEAR_MASTER_KEY")
            .map_err(|_| "MASTER_KEY not found in environment or .env file")?;

        let redis_url = env::var("REDIS_URL")
            .map_err(|_| "REDIS_URL not found in environment or .env file")?;
        // Combine into the final Settings struct
        Ok(Settings {
            rpc_urls: file_settings.rpc_urls,
            ft_contract_id: file_settings.ft_contract_id,
            account_id: file_settings.account_id,
            master_key,
            ft_decimals: file_settings.ft_decimals,
            batch_size: file_settings.batch_size,
            batch_timeout_secs: file_settings.batch_timeout_secs,
            concurrency: file_settings.concurrency,
            num_pool_keys: file_settings.num_pool_keys,
            key_allowance_near: file_settings.key_allowance_near,
            network: file_settings.network,
            redis_url,
        })
    }
}
