use serde::{Deserialize, Serialize};
use crate::matching_engine::model::{Order, OrderType, Side, ExecutionReport, OrderBookSnapshot};

/// 주문 제출 요청
#[derive(Debug, Deserialize, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<u64>,
    pub quantity: u64,
    pub client_id: String,
}

/// 주문 제출 응답
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: String,
    pub message: String,
}

/// 주문 취소 요청
#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub order_id: String,
}

/// 주문 취소 응답
#[derive(Debug, Serialize)]
pub struct CancelOrderResponse {
    pub order_id: String,
    pub status: String,
    pub message: String,
}

/// 체결 내역 조회 응답
#[derive(Debug, Serialize)]
pub struct ExecutionResponse {
    pub executions: Vec<ExecutionReport>,
}

/// 주문서 조회 응답
#[derive(Debug, Serialize)]
pub struct OrderBookResponse {
    pub orderbook: OrderBookSnapshot,
}

/// 시장 통계 응답
#[derive(Debug, Serialize)]
pub struct MarketStatisticsResponse {
    pub symbol: String,
    pub timestamp: u64,
    pub open_price_24h: Option<u64>,
    pub high_price_24h: Option<u64>,
    pub low_price_24h: Option<u64>,
    pub last_price: Option<u64>,
    pub volume_24h: u64,
    pub price_change_24h: Option<f64>,
    pub bid_price: Option<u64>,
    pub ask_price: Option<u64>,
}

/// 봉차트 데이터 응답
#[derive(Debug, Serialize)]
pub struct CandleResponse {
    pub symbol: String,
    pub interval: String,
    pub candles: Vec<CandleData>,
}

/// 봉차트 데이터
#[derive(Debug, Serialize)]
pub struct CandleData {
    pub open_time: u64,
    pub close_time: u64,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
    pub trade_count: u64,
}

/// WebSocket 메시지 타입
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// 체결 결과
    Execution(ExecutionReport),
    /// 호가창 업데이트
    OrderBookUpdate {
        symbol: String,
        bids: Vec<(u64, u64)>,
        asks: Vec<(u64, u64)>,
        timestamp: u64,
    },
    /// 시장 통계 업데이트
    MarketStatistics {
        symbol: String,
        timestamp: u64,
        last_price: Option<u64>,
        price_change_24h: Option<f64>,
        volume_24h: u64,
        high_price_24h: Option<u64>,
        low_price_24h: Option<u64>,
    },
    /// 봉차트 업데이트
    CandlestickUpdate {
        symbol: String,
        interval: String,
        candle: CandleData,
    },
    /// 에러 메시지
    Error {
        message: String,
    },
}

/// API 오류 응답
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}