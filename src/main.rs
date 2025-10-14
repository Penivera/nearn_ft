use actix_web::{App, HttpServer, middleware::Logger, web};
use deadpool_redis::{Config, Runtime};
use futures::future::join_all;
use log::{error, info};
use near_api::near_primitives::account::AccessKeyPermission;
use near_api::near_primitives::views::FinalExecutionStatus;
use near_api::{signer::generate_secret_key, *};
use nearn_ft::{
    ApiDoc, config::Settings, ft_transfer, get_all_transactions, get_transaction_by_id,
    get_transactions_by_receiver, types::TokenTransferRequest, worker::run_worker,get_transactions_by_status
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use url::Url;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
pub mod types;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let settings: Settings = Settings::new().expect("Failed to load settings from Settings.toml");

    // --- Create Redis Connection Pool ---
    let redis_cfg = Config::from_url(&settings.redis_url);
    let redis_pool = redis_cfg
        .create_pool(Some(Runtime::Tokio1))
        .expect("Failed to create Redis pool");
    info!("Redis connection pool created.");

    let network_config = NetworkConfig {
        network_name: settings.network.clone(),
        // Iterate over the URLs, parse them, create an RPCEndpoint for each,
        // and collect them into the vector.
        rpc_endpoints: settings
            .rpc_urls
            .iter()
            .map(|url_str| {
                RPCEndpoint::new(
                    Url::parse(url_str).expect("Failed to parse RPC URL from settings"),
                )
            })
            .collect(),
        linkdrop_account_id: None,
        near_social_db_contract_account_id: None,
        faucet_url: None,
        meta_transaction_relayer_url: None,
        fastnear_url: None,
        staking_pools_factory_account_id: None,
    };

    let master_signer: signer::secret_key::SecretKeySigner =
        Signer::from_seed_phrase(&settings.master_key, None)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            .expect("Failed to create master signer from seed phrase");

    let master_signer: Arc<Signer> = Signer::new(master_signer)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        .expect("Failed to create master signer")
        .into();

    info!(
        "Generating and adding {} keys to the pool...",
        settings.num_pool_keys
    );

    let key_futures = (0..settings.num_pool_keys).map(|_| {
        let settings = settings.clone();
        let network_config = network_config.clone();
        let master_signer = Arc::clone(&master_signer);

        tokio::spawn(async move {
            let new_secret_key = generate_secret_key().expect("Failed to generate secret key");
            let new_public_key = new_secret_key.public_key();

            /*let allowance = (settings.key_allowance_near * 1_000_000_000_000_000_000_000_000.0) as u128;

            let ft_contract_id = AccountId::from_str(&settings.ft_contract_id).unwrap();*/

            let result = Account(near_sdk::AccountId::from_str(&settings.account_id).unwrap())
                .add_key(AccessKeyPermission::FullAccess, new_public_key.clone())
                .with_signer(Arc::clone(&master_signer))
                .send_to(&network_config)
                .await;

            match result {
                Ok(res) => {
                    if matches!(res.status, FinalExecutionStatus::SuccessValue(_)) {
                        info!("Successfully added key: {}", new_public_key);
                        let new_signer = Signer::from_secret_key(new_secret_key);
                        master_signer
                            .add_signer_to_pool(new_signer)
                            .await
                            .expect("Failed to add signer to pool");
                    } else {
                        error!("Failed to add key {}: {:?}", &new_public_key, res.status);
                    }
                }
                Err(e) => {
                    error!("Error adding key {}: {}", &new_public_key, e);
                }
            }
        })
    });

    join_all(key_futures).await;

    info!("Key pool successfully populated.");

    let (tx, rx) = mpsc::channel::<(String, TokenTransferRequest)>(1000);

    let worker_settings = settings.clone();
    /*let account_id = AccountId::from_str(&settings.account_id).unwrap();
    let ft_contract_id = AccountId::from_str(&settings.ft_contract_id).unwrap();*/
    let worker_signer = Arc::clone(&master_signer);
    let worker_redis_pool = redis_pool.clone();

    tokio::spawn(async move {
        run_worker(
            rx,
            worker_signer,
            worker_settings,
            network_config,
            worker_redis_pool,
        )
        .await;
    });

    info!("🚀 Server starting at port 8080");
    info!("📚 Swagger UI available at /");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(tx.clone()))
            .app_data(web::Data::new(settings.clone()))
            .app_data(web::Data::new(redis_pool.clone()))
            .wrap(Logger::new("%r %T"))
            .service(ft_transfer)
            .service(get_transaction_by_id)
            .service(get_transactions_by_receiver)
            .service(get_all_transactions)
            .service(get_transactions_by_status)
            .service(SwaggerUi::new("/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
