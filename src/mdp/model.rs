use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/**
* filename : model
* author : HAMA
* date: 2025. 5. 13.
* description: Market Data Publisher 모델 정의
**/

/// 시장 데이터 이벤트 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketDataEvent {
    OrderBookUpdate {
        symbol: String,
        bids: Vec<(u64, u64)>,
        asks: Vec<(u64, u64)>,
    },
    TradeUpdate {
        symbol: String,
        price: u64,
        quantity: u64,
        timestamp: u64,
    },
    CandlestickUpdate {
        symbol: String,
        interval: String,
        candle: CandlestickData,
    },
    Execution {
        symbol: String,
        execution_id: String,
        order_id: String,
        side: String,
        price: u64,
        quantity: u64,
        timestamp: u64,
    },
}

/// 봉차트 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickData {
    pub open_time: u64,
    pub close_time: u64,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
    pub trade_count: u64,
}

/// 시장 통계 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStatistics {
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

/// 체결 내역 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub symbol: String,
    pub total_volume: u64,
    pub total_trades: u64,
    pub avg_price: f64,
    pub last_price: u64,
    pub timestamp: u64,
}

/// 주문서 레벨 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: u64,
    pub volume: u64,
    pub order_count: u64,
}

/// 주문서 스냅샷 (MDP 버전)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshotMDP {
    pub symbol: String,
    pub timestamp: u64,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}
