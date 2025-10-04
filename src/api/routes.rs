use axum::{
    routing::{get, post},
    Router,
};

use crate::api::handlers::*;
use crate::server::ServerState;

/// API 라우터 생성
pub fn create_api_router() -> Router<ServerState> {
    Router::new()
        // 주문 관련 API
        .route("/v1/order", post(submit_order))
        .route("/v1/order/cancel", post(cancel_order))
        .route("/v1/order/:order_id", get(get_order_status))
        
        // 시장 데이터 API
        .route("/api/v1/orderbook/:symbol", get(get_orderbook))
        .route("/api/v1/executions/:symbol", get(get_executions))
        .route("/api/v1/statistics/:symbol", get(get_statistics))
        .route("/api/v1/klines/:symbol/:interval", get(get_candles))
        
        // 하이브리드 호가창 동기화 API
        .route("/api/v1/sync/:symbol", get(sync_orderbook))
}