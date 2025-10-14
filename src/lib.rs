pub mod config;
pub mod types;
pub mod worker;

use actix_web::{get, post, web::{Data, Json}, HttpResponse, Responder};
use actix_web::web::{Path, Query};
use deadpool_redis::Pool;
use log::error;
use redis::AsyncCommands;
use tokio::sync::mpsc::Sender;
use types::*;
use utoipa::OpenApi;
use crate::config::Settings;

#[derive(OpenApi)]
#[openapi(
    paths(
        ft_transfer,
        get_transaction_by_id,
        get_transactions_by_receiver,
        get_all_transactions,
        get_transactions_by_status
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
        (name = "NEAR FT Transfer Service", description = "Endpoints for a high-throughput FT transfer service")
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
    sender: Data<Sender<TokenTransferRequest>>,
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
            .lpush(format!("user_txns:{}", record.request.reciever_id), &record.id)
            .await
            .unwrap_or_else(|e| error!("Redis LPUSH error: {}", e));
    });


    match sender.send(request).await {
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
    params(("id" = String, Path, description = "Unique ID of the transaction")),
    responses(
        (status = 200, body = TransactionRecord),
        (status = 404)
    )
)]
#[get("/transaction/{id}")]
pub async fn get_transaction_by_id(
    path: Path<String>,
    redis_pool: Data<Pool>,
) -> impl Responder {
    let tx_id = path.into_inner();
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Could not get Redis connection: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };
    match conn.get::<_, String>(format!("txn:{}", tx_id)).await {
        Ok(record_json) => match serde_json::from_str::<TransactionRecord>(&record_json) {
            Ok(record) => HttpResponse::Ok().json(record),
            Err(_) => HttpResponse::InternalServerError().finish(),
        },
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

#[utoipa::path(
    get,
    path = "/transactions/{receiver_id}",
    params(
        ("receiver_id" = String, Path, description = "The NEAR account ID of the receiver"),
        ("offset" = Option<u64>, Query, description = "Pagination offset, default 0"),
        ("limit" = Option<u64>, Query, description = "Pagination limit, default 10")
    ),
    responses((status = 200, body = [TransactionRecord]))
)]
#[get("/transactions/{receiver_id}")]
pub async fn get_transactions_by_receiver(
    path: Path<String>,
    query: Query<Pagination>,
    redis_pool: Data<Pool>,
) -> impl Responder {
    let receiver_id = path.into_inner();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(10);
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Could not get Redis connection: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let user_txn_key = format!("user_txns:{}", receiver_id);
    let txn_ids: Vec<String> = match conn.lrange(&user_txn_key, offset, offset + limit - 1).await {
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
        .filter_map(|opt_json_str| {
            opt_json_str.and_then(|json_str| serde_json::from_str(&json_str).ok())
        })
        .collect();
    HttpResponse::Ok().json(records)
}

#[utoipa::path(
    get,
    path = "/transactions",
    params(
        ("receiver_id" = String, Path, description = "The NEAR account ID of the receiver"),
        ("offset" = Option<u64>, Query, description = "Pagination offset, default 0"),
        ("limit" = Option<u64>, Query, description = "Pagination limit, default 10")
    ),
    responses((status = 200, body = PaginatedTransactionResponse))
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
    if keys.is_empty() {
        return HttpResponse::Ok().json(PaginatedTransactionResponse {
            next_cursor,
            records: Vec::new(),
        });
    }
    let records_json: Vec<Option<String>> = match conn.mget(keys).await {
        Ok(records) => records,
        Err(_) => return HttpResponse::InternalServerError().finish(),
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



#[utoipa::path(
    get,
    path = "/transactions/status/{status}",
    params(
        ("status" = String, Path, description = "The status to filter by (Queued, Success, Failure)")
    ),
    responses(
        (status = 200, description = "A list of transaction records matching the status", body = [TransactionRecord])
    )
)]
#[get("/transactions/status/{status}")]
pub async fn get_transactions_by_status(
    path: Path<String>,
    redis_pool: Data<Pool>,
) -> impl Responder {
    let status_str = path.into_inner();
    let status_to_filter = match status_str.to_lowercase().as_str() {
        "queued" => TransactionStatus::Queued,
        "success" => TransactionStatus::Success,
        "failure" => TransactionStatus::Failure,
        _ => return HttpResponse::BadRequest().body("Invalid status provided. Use one of: Queued, Success, Failure."),
    };

    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Could not get Redis connection: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut all_keys = Vec::new();
    let mut cursor = 0;

    // THE FIX: Manually loop through all keys using SCAN
    loop {
        let (next_cursor, keys): (u64, Vec<String>) = match redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("txn:*")
            .arg("COUNT")
            .arg(100) // Fetch 100 keys at a time
            .query_async(&mut conn)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                error!("Redis SCAN error: {}", e);
                return HttpResponse::InternalServerError().finish();
            }
        };

        all_keys.extend(keys);

        if next_cursor == 0 {
            break; // The iteration is complete
        }
        cursor = next_cursor;
    }

    if all_keys.is_empty() {
        return HttpResponse::Ok().json(Vec::<TransactionRecord>::new());
    }

    let records_json: Vec<Option<String>> = match conn.mget(all_keys).await {
        Ok(records) => records,
        Err(e) => {
            error!("Redis MGET error: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let records: Vec<TransactionRecord> = records_json
        .into_iter()
        .filter_map(|opt_json_str| {
            opt_json_str.and_then(|json_str| serde_json::from_str(&json_str).ok())
        })
        .filter(|record: &TransactionRecord| std::mem::discriminant(&record.status) == std::mem::discriminant(&status_to_filter))
        .collect();

    HttpResponse::Ok().json(records)
}