use crate::config::Settings;
use crate::types::{TokenTransferRequest, TransactionRecord, TransactionStatus};
use deadpool_redis::Pool;
use log::{error, info};
use near_api::near_primitives::action::{Action, FunctionCallAction};
use near_api::near_primitives::views::FinalExecutionStatus;
use near_api::*;
use near_sdk::AccountId;
use near_sdk::json_types::U128;
use redis::AsyncCommands;
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Semaphore, mpsc::Receiver};
use tokio::time::timeout;

pub async fn run_worker(
    // THE FIX: The receiver now gets a tuple of (ID, Request)
    mut receiver: Receiver<(String, TokenTransferRequest)>,
    signer: Arc<Signer>,
    settings: Settings,
    network_config: NetworkConfig,
    // THE FIX: Accept the Redis pool
    redis_pool: Pool,
) {
    let semaphore = Arc::new(Semaphore::new(settings.concurrency));

    loop {
        // The batch now stores tuples of (ID, Request)
        let mut batch: Vec<(String, TokenTransferRequest)> =
            Vec::with_capacity(settings.batch_size);

        match receiver.recv().await {
            Some(item) => batch.push(item),
            None => break,
        }

        let batch_timeout = Duration::from_secs(settings.batch_timeout_secs);
        while batch.len() < settings.batch_size {
            match timeout(batch_timeout, receiver.recv()).await {
                Ok(Some(item)) => batch.push(item),
                _ => break,
            }
        }

        if !batch.is_empty() {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let signer = Arc::clone(&signer);
            let settings = settings.clone();
            let network_config = network_config.clone();
            let redis_pool = redis_pool.clone(); // Clone pool for the task

            tokio::spawn(async move {
                let _permit = permit;
                let transfers_count = batch.len();

                let sender_id = AccountId::from_str(&settings.account_id).unwrap();
                let ft_contract_id = AccountId::from_str(&settings.ft_contract_id).unwrap();

                let mut transaction =
                    Transaction::construct(sender_id.clone(), ft_contract_id.clone());

                let deposit = 1;
                let gas = 30_000_000_000_000;

                // The loop now destructures the tuple
                for (_id, transfer) in &batch {
                    let amount_raw = transfer.amount.parse::<u128>().unwrap_or(0);
                    if amount_raw > 0 {
                        let ft_transfer_action =
                            Action::FunctionCall(Box::new(FunctionCallAction {
                                method_name: "ft_transfer".to_string(),
                                args: json!({
                                    "receiver_id": transfer.reciever_id,
                                    "amount": U128(amount_raw),
                                    "memo": transfer.memo,
                                })
                                .to_string()
                                .into_bytes(),
                                gas,
                                deposit,
                            }));
                        transaction = transaction.add_action(ft_transfer_action);
                    }
                }

                info!("Sending batch of {} transfers...", transfers_count);

                let transaction_result = transaction
                    .with_signer(signer)
                    .send_to(&network_config)
                    .await;

                // --- THE FIX: Spawn a new task to update Redis ---
                tokio::spawn(async move {
                    let mut conn = match redis_pool.get().await {
                        Ok(conn) => conn,
                        Err(e) => {
                            error!("Worker failed to get Redis connection: {}", e);
                            return;
                        }
                    };

                    match transaction_result {
                        Ok(result)
                            if matches!(result.status, FinalExecutionStatus::SuccessValue(_)) =>
                        {
                            info!("Batch successful. Hash: {}", result.transaction.hash);
                            for (id, _) in batch {
                                let key = format!("txn:{}", id);
                                if let Ok(mut record) =
                                    conn.get::<_, String>(&key).await.and_then(|json| {
                                        Ok(serde_json::from_str::<TransactionRecord>(&json)
                                            .unwrap())
                                    })
                                {
                                    record.status = TransactionStatus::Success;
                                    record.txn_hash = Some(result.transaction.hash.to_string());
                                    let _: () = conn
                                        .set(&key, serde_json::to_string(&record).unwrap())
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                        Ok(result) => {
                            error!("Batch failed. Status: {:?}", result.status);
                            for (id, _) in batch {
                                let key = format!("txn:{}", id);
                                if let Ok(mut record) =
                                    conn.get::<_, String>(&key).await.and_then(|json| {
                                        Ok(serde_json::from_str::<TransactionRecord>(&json)
                                            .unwrap())
                                    })
                                {
                                    record.status = TransactionStatus::Failure;
                                    record.error_message = Some(format!("{:?}", result.status));
                                    let _: () = conn
                                        .set(&key, serde_json::to_string(&record).unwrap())
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error sending batch: {}", e);
                            for (id, _) in batch {
                                let key = format!("txn:{}", id);
                                if let Ok(mut record) =
                                    conn.get::<_, String>(&key).await.and_then(|json| {
                                        Ok(serde_json::from_str::<TransactionRecord>(&json)
                                            .unwrap())
                                    })
                                {
                                    record.status = TransactionStatus::Failure;
                                    record.error_message = Some(e.to_string());
                                    let _: () = conn
                                        .set(&key, serde_json::to_string(&record).unwrap())
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                    }
                });
            });
        }
    }
}
