use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 체결 내역 DB 모델
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionRecord {
    pub exec_id: String,
    pub taker_order_id: String,
    pub maker_order_id: String,
    pub symbol: String,
    pub side: String,
    pub price: i64,
    pub quantity: i64,
    pub taker_fee: i64,
    pub maker_fee: i64,
    pub transaction_time: i64,
}

/// 주문 DB 모델
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OrderRecord {
    pub order_id: String,
    pub client_id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: Option<i64>,
    pub quantity: i64,
    pub filled_quantity: i64,
    pub status: String,
}

/// 잔고 DB 모델
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BalanceRecord {
    pub client_id: String,
    pub asset: String,
    pub available: i64,
    pub locked: i64,
}

/// 감사 로그 DB 모델
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: Option<i64>,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: String,
    pub details: Option<String>,
}
