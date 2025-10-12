pub mod config;
pub mod types;
pub mod worker;

use crate::config::Settings;
use actix_web::web::{Path, Query};
use actix_web::{
    HttpResponse, Responder, get, post,
    web::{Data, Json},
};
use deadpool_redis::Pool;
use log::{error, info};
use redis::AsyncCommands;
use tokio::sync::mpsc::Sender;
use types::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        ft_transfer,
        get_transaction_by_id,
        get_transactions_by_receiver,
        get_all_transactions
    ),
    components(schemas(
        TokenTransferRequest,
        TransactionRecord,
        TransactionStatus,
        TransferResponse,
        PaginatedTransactionResponse,
        Pagination,
        ScanPagination
    )),
    tags(
        (name = "NEAR FT Transfer Service", description = "Endpoints for a high-throughput FT transfer service it assumes reciever_id is always a valid account on the NEAR blockchain.")
    )
)]
pub struct ApiDoc;

#[utoipa::path(
    post,
    path = "/transfer",
    request_body = TokenTransferRequest,
    responses(
        (status = 202, description = "Transfer request accepted for processing", body = TransferResponse),
        (status = 400, description = "Invalid input", body = TransferResponse),
        (status = 500, description = "Internal server error", body = TransferResponse)
    )
)]
#[post("/transfer")]
pub async fn ft_transfer(
    payload: Json<TokenTransferRequest>,
    sender: Data<Sender<(String, TokenTransferRequest)>>,
    settings: Data<Settings>,
    redis_pool: Data<Pool>,
) -> impl Responder {
    let request = payload.into_inner();
    let record = TransactionRecord::new(settings.account_id.clone(), request.clone());
    let record_id = record.id.clone();
    // Basic validation
    if request.reciever_id.to_string().is_empty() || request.amount.parse::<u128>().is_err() {
        return HttpResponse::BadRequest().json(TransferResponse {
            success: false,
            message: "Invalid receiver_id or amount".to_string(),
            transaction_id: record_id,
        });
    }

    // --- Spawn Green Thread for Redis Write ---
    tokio::spawn(async move {
        let record_json = serde_json::to_string(&record).unwrap_or_default();
        let mut conn = redis_pool.get().await.expect("Failed to get redis conn");

        // Save the full record
        let _: () = conn
            .set(format!("txn:{}", record.id), record_json)
            .await
            .unwrap_or_else(|e| error!("Redis SET error: {}", e));

        // Add to the user's transaction list
        let _: () = conn
            .lpush(
                format!("user_txns:{}", record.request.reciever_id),
                &record.id,
            )
            .await
            .unwrap_or_else(|e| error!("Redis LPUSH error: {}", e));
    });

    match sender.send((record_id.clone(), request)).await {
        Ok(_) => HttpResponse::Accepted().json(TransferResponse {
            success: true,
            message: "Transfer request accepted and queued for processing.".to_string(),
            transaction_id: record_id,
        }),
        Err(_) => HttpResponse::InternalServerError().json(TransferResponse {
            success: false,
            message: "Failed to queue transfer request.".to_string(),
            transaction_id: record_id,
        }),
    }
}
#[utoipa::path(
    get,
    path = "/transaction/{id}",
    params(
        ("id" = String, Path, description = "Unique ID of the transaction to fetch")
    ),
    responses(
        (status = 200, description = "Transaction record found", body = TransactionRecord),
        (status = 404, description = "Transaction not found")
    )
)]
#[get("/transaction/{id}")]
pub async fn get_transaction_by_id(path: Path<String>, redis_pool: Data<Pool>) -> impl Responder {
    let tx_id = path.into_inner();
    let key = format!("txn:{}", tx_id);
    info!("Attempting to fetch transaction with key: {}", key); // Added logging

    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Could not get Redis connection: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    match conn.get::<_, String>(&key).await {
        Ok(record_json) => {
            info!("Found record for key: {}", key); // Added logging
            match serde_json::from_str::<TransactionRecord>(&record_json) {
                Ok(record) => HttpResponse::Ok().json(record),
                Err(e) => {
                    error!("Failed to parse JSON for key {}: {}", key, e);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Err(e) => {
            // This error is expected if the key doesn't exist
            error!("Record not found for key {}: {}", key, e); // Added logging
            HttpResponse::NotFound().finish()
        }
    }
}

// In src/lib.rs

#[utoipa::path(
    get,
    path = "/transactions/{receiver_id}",
    params(
        ("receiver_id" = String, Path, description = "The NEAR account ID of the receiver")
    ),
    // Removed the pagination params
    responses(
        (status = 200, description = "A list of all transaction records for the receiver", body = [TransactionRecord])
    )
)]
#[get("/transactions/{receiver_id}")]
pub async fn get_transactions_by_receiver(
    path: Path<String>,
    redis_pool: Data<Pool>,
) -> impl Responder {
    let receiver_id = path.into_inner();

    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Could not get Redis connection: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let user_txn_key = format!("user_txns:{}", receiver_id);
    // Fetch all IDs from the list by using 0 (start) and -1 (end)
    let txn_ids: Vec<String> = match conn.lrange(&user_txn_key, 0, -1).await {
        Ok(ids) => ids,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    if txn_ids.is_empty() {
        return HttpResponse::Ok().json(Vec::<TransactionRecord>::new());
    }

    let keys: Vec<String> = txn_ids.into_iter().map(|id| format!("txn:{}", id)).collect();

    let records_json: Vec<Option<String>> = match conn.mget(keys).await {
        Ok(records) => records,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let records: Vec<TransactionRecord> = records_json
        .into_iter()
        .filter_map(|opt_json_str| opt_json_str.and_then(|json_str| serde_json::from_str(&json_str).ok()))
        .collect();

    HttpResponse::Ok().json(records)
}


#[utoipa::path(
    get,
    path = "/transactions",
    // THE FIX: Define the optional query parameters explicitly.
    params(
        ("cursor" = Option<u64>, Query, description = "Scan cursor (use 0 to start)"),
        ("count" = Option<u64>, Query, description = "Maximum number of items to return")
    ),
    responses(
        (status = 200, body = PaginatedTransactionResponse)
    )
)]
#[get("/transactions")]
pub async fn get_all_transactions(
    query: Query<ScanPagination>,
    redis_pool: Data<Pool>,
) -> impl Responder {
    let cursor = query.cursor.unwrap_or(0);
    let count = query.count.unwrap_or(10);

    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Could not get Redis connection: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    info!("Scanning for transactions with cursor {}...", cursor); // Added logging

    let (next_cursor, keys): (u64, Vec<String>) = match redis::cmd("SCAN")
        .arg(cursor)
        .arg("MATCH")
        .arg("txn:*")
        .arg("COUNT")
        .arg(count)
        .query_async(&mut conn)
        .await
    {
        Ok(res) => res,
        Err(e) => {
            error!("Redis SCAN error: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    info!(
        "SCAN found {} keys. Next cursor: {}",
        keys.len(),
        next_cursor
    ); // Added logging

    if keys.is_empty() {
        return HttpResponse::Ok().json(PaginatedTransactionResponse {
            next_cursor,
            records: Vec::new(),
        });
    }

    let records_json: Vec<Option<String>> = match conn.mget(&keys).await {
        Ok(records) => records,
        Err(e) => {
            error!("Redis MGET error for scanned keys: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let records: Vec<TransactionRecord> = records_json
        .into_iter()
        .filter_map(|opt_json_str| {
            opt_json_str.and_then(|json_str| serde_json::from_str(&json_str).ok())
        })
        .collect();

    HttpResponse::Ok().json(PaginatedTransactionResponse {
        next_cursor,
        records,
    })
}
