use crate::config::Settings;
use crate::types::TokenTransferRequest;
use log::{error, info};
use near_api::*;
use near_sdk::AccountId;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use near_api::near_primitives::action::{Action, FunctionCallAction};
use near_api::near_primitives::views::FinalExecutionStatus;
use near_sdk::json_types::U128;
use serde_json::json;
use tokio::sync::{mpsc::Receiver, Semaphore};
use tokio::time::timeout;

pub async fn run_worker(
    mut receiver: Receiver<TokenTransferRequest>,
    signer: Arc<Signer>,
    settings: Settings,
    network_config: NetworkConfig,
) {
    let semaphore = Arc::new(Semaphore::new(settings.concurrency));

    loop {
        let mut batch = Vec::with_capacity(settings.batch_size);

        // Wait for the first transfer to start a new batch
        match receiver.recv().await {
            Some(transfer) => batch.push(transfer),
            None => break, // Channel closed
        }

        // Continue filling the batch until it's full or the timeout expires
        let batch_timeout = Duration::from_secs(settings.batch_timeout_secs);
        while batch.len() < settings.batch_size {
            match timeout(batch_timeout, receiver.recv()).await {
                Ok(Some(transfer)) => batch.push(transfer),
                _ => break, // Timeout or channel closed
            }
        }

        if !batch.is_empty() {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let signer = Arc::clone(&signer);
            let settings = settings.clone();
            let network_config = network_config.clone();

            tokio::spawn(async move {
                let _permit = permit; // Permit is dropped when this task finishes
                let transfers_count = batch.len();

                let sender_id = AccountId::from_str(&settings.account_id).unwrap();
                let ft_contract_id = AccountId::from_str(&settings.ft_contract_id).unwrap();

                // 1. Create a transaction builder. The signer and receiver are the same.
                let mut transaction = Transaction::construct(sender_id.clone(), ft_contract_id.clone());

                let deposit = 1;

                // A standard gas amount for a simple transfer. This can be tuned.
                let gas = 30_000_000_000_000;
                // 2. Loop through the batch and add an `ft_transfer` action for each item.
                for transfer in batch {
                    let amount_raw = transfer.amount.parse::<u128>().unwrap_or(0);
                    if amount_raw > 0 {
                        let _amount =
                            FTBalance::with_decimals(settings.ft_decimals).with_amount(amount_raw);

                        // Create the fungible token transfer action
                        let ft_transfer_action = Action::FunctionCall(Box::new(FunctionCallAction {
                            method_name: "ft_transfer".to_string(),
                            // Serialize the arguments to a JSON byte vector.
                            args: json!({
                                "receiver_id": transfer.reciever_id,
                                "amount": U128(amount_raw),
                                "memo": transfer.memo,
                            })
                                .to_string()
                                .into_bytes(),
                            gas,
                            deposit,
                        }));// .call_function can fail if args aren't serializable


                        // Add the created action to our main transaction builder
                        transaction = transaction.add_action(ft_transfer_action);
                    }
                }

                info!("Sending batch of {} transfers...", transfers_count);

                // 3. Attach the signer (with its key pool) and send the transaction.
                let transaction_result = transaction
                    .with_signer(signer)
                    .send_to(&network_config)
                    .await;

                match transaction_result {
                    Ok(result) if matches!(result.status, FinalExecutionStatus::SuccessValue(_)) => {
                        info!(
                            "Batch transaction successful. Hash: {}",
                            result.transaction.hash
                        );
                    }
                    Ok(result) => {
                        error!(
                            "Batch transaction failed. Hash: {}. Status: {:?}",
                            result.transaction.hash, result.status
                        );
                    }
                    Err(e) => {
                        error!("Error sending batch transaction: {}", e);
                    }
                }
            });
        }
    }
}