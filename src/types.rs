use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub enum TransactionStatus {
    Queued,
    Success,
    Failure,
}

#[derive(Deserialize, Serialize, Debug, Clone, ToSchema)]
pub struct TokenTransferRequest {
    #[schema(value_type = String)]
    pub reciever_id: String,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct TransactionRecord {
    pub id: String,
    pub sender_id: String,
    pub status: TransactionStatus,
    pub request: TokenTransferRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

impl TransactionRecord {
    pub fn new(sender_id: String, request: TokenTransferRequest) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender_id,
            status: TransactionStatus::Queued,
            request,
            txn_hash: None,
            error_message: None,
            created_at: Utc::now(),
        }
    }
}

// --- PAGINATION AND RESPONSE STRUCTS ---

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct Pagination {
    pub offset: Option<isize>,
    pub limit: Option<isize>,
}

#[derive(Deserialize, ToSchema, IntoParams)]
pub struct ScanPagination {
    pub cursor: Option<u64>,
    pub count: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct TransferResponse {
    pub success: bool,
    pub message: String,
    pub transaction_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct PaginatedTransactionResponse {
    pub next_cursor: u64,
    pub records: Vec<TransactionRecord>,
}
