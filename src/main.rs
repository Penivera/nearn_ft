use std::fs::File;
use std::io::BufReader;
use actix_web::{App, HttpServer, middleware::Logger, web};
use futures::future::join_all;
use log::{error, info};
use near_api::near_primitives::account::{AccessKeyPermission, FunctionCallPermission};
use near_api::near_primitives::views::FinalExecutionStatus;
use near_api::{signer::generate_secret_key, *};
use nearn_ft::{
    ApiDoc, config::Settings, ft_transfer, types::TokenTransferRequest, worker::run_worker,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod types;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let settings: Settings = Settings::new().expect("Failed to load settings from Settings.toml");
    let network_config: NetworkConfig = match settings.network.as_str() {
        "mainnet" => NetworkConfig::mainnet(),
        "testnet" => NetworkConfig::testnet(),
        other => {
            info!("Unknown network `{}`, defaulting to testnet", other);
            NetworkConfig::testnet()
        }
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
                .add_key(
                    AccessKeyPermission::FullAccess,
                    new_public_key.clone(),
                )
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

    let (tx, rx) = mpsc::channel::<TokenTransferRequest>(1000); // Channel with a buffer of 1000

    let worker_settings = settings.clone();
    /*let account_id = AccountId::from_str(&settings.account_id).unwrap();
    let ft_contract_id = AccountId::from_str(&settings.ft_contract_id).unwrap();*/
    let worker_signer = Arc::clone(&master_signer);

    tokio::spawn(async move {
        run_worker(rx, worker_signer, worker_settings, network_config).await;
    });

    info!("🚀 Server starting at http://127.0.0.1:8000");
    info!("📚 Swagger UI available at http://127.0.0.1:8000/docs/");

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    let mut certs_file = BufReader::new(File::open("cert.pem").unwrap());
    let mut key_file = BufReader::new(File::open("key.pem").unwrap());

    // load TLS certs and key
    // to create a self-signed temporary cert for testing:
    // `openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'`
    let tls_certs = rustls_pemfile::certs(&mut certs_file)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let tls_key = rustls_pemfile::pkcs8_private_keys(&mut key_file)
        .next()
        .unwrap()
        .unwrap();

    // set up TLS config options
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(tls_certs, rustls::pki_types::PrivateKeyDer::Pkcs8(tls_key))
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(tx.clone()))
            .wrap(Logger::new("%r %T"))
            .service(ft_transfer)
            .service(
                SwaggerUi::new("/docs/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .bind_rustls_0_23(("127.0.0.1", 8000), tls_config)?
    .run()
    .await
}
