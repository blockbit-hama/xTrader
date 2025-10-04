use serde::{Deserialize, Serialize};
use crate::matching_engine::model::{Order, OrderType, Side, ExecutionReport, OrderBookSnapshot as EngineOrderBookSnapshot};

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
    pub orderbook: EngineOrderBookSnapshot,
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
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// 호가창 변경 타입
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum OrderBookChangeType {
    /// 추가
    Add,
    /// 업데이트
    Update,
    /// 제거
    Remove,
}

/// 호가창 변경 사항
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderBookChange {
    /// 변경 타입
    pub change_type: OrderBookChangeType,
    /// 가격
    pub price: u64,
    /// 수량
    pub quantity: u64,
}

/// 호가창 Delta 업데이트
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderBookDelta {
    /// 심볼
    pub symbol: String,
    /// 매수 변경사항
    pub bid_changes: Vec<OrderBookChange>,
    /// 매도 변경사항
    pub ask_changes: Vec<OrderBookChange>,
    /// 타임스탬프
    pub timestamp: u64,
    /// 시퀀스 번호 (동기화용)
    pub sequence: u64,
}

/// 호가창 Snapshot 업데이트
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderBookSnapshot {
    /// 심볼
    pub symbol: String,
    /// 매수 호가
    pub bids: Vec<(u64, u64)>,
    /// 매도 호가
    pub asks: Vec<(u64, u64)>,
    /// 타임스탬프
    pub timestamp: u64,
    /// 시퀀스 번호 (동기화용)
    pub sequence: u64,
}

/// WebSocket 메시지 타입
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// 체결 결과
    Execution(ExecutionReport),
    /// 호가창 Delta 업데이트 (변경된 부분만)
    OrderBookDelta(OrderBookDelta),
    /// 호가창 Snapshot 업데이트 (전체 호가창)
    OrderBookSnapshot(OrderBookSnapshot),
    /// 호가창 업데이트 (기존 호환성 유지)
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
    /// 동기화 요청 응답
    SyncResponse {
        symbol: String,
        snapshot: OrderBookSnapshot,
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