use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::models::*;
use crate::matching_engine::engine::MatchingEngine;
use crate::matching_engine::model::{Order, OrderType, Side};
use crate::server::ServerState;

/// 주문 제출 핸들러
pub async fn submit_order(
    State(state): State<ServerState>,
    Json(payload): Json<OrderRequest>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 입력 검증
    if payload.quantity == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "INVALID_QUANTITY".to_string(),
                message: "수량은 0보다 커야 합니다".to_string(),
            }),
        ));
    }

    if payload.order_type == OrderType::Limit && payload.price.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "MISSING_PRICE".to_string(),
                message: "지정가 주문에는 가격이 필요합니다".to_string(),
            }),
        ));
    }

    if payload.order_type == OrderType::Limit && payload.price.unwrap_or(0) == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "INVALID_PRICE".to_string(),
                message: "가격은 0보다 커야 합니다".to_string(),
            }),
        ));
    }

    // 주문 생성
    let order_id = Uuid::new_v4().to_string();
    let order = Order::new(
        order_id.clone(),
        payload.symbol.clone(),
        payload.side.clone(),
        payload.order_type.clone(),
        payload.price.unwrap_or(0),
        payload.quantity,
        payload.client_id.clone(),
    );

    // 주문을 채널로 전송 (빠른 응답을 위해 clone 사용)
    if let Err(e) = state.order_tx.send(order.clone()) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "ORDER_SEND_FAILED".to_string(),
                message: format!("주문 전송 실패: {}", e),
            }),
        ));
    }

    // 즉시 접수 확인 응답 (매칭 결과는 WebSocket으로 전달)
    Ok(Json(OrderResponse {
        order_id,
        status: "ACCEPTED".to_string(),
        message: "주문이 접수되었습니다. 체결 결과는 WebSocket으로 전달됩니다".to_string(),
    }))
}

/// 주문 취소 핸들러
pub async fn cancel_order(
    State(state): State<ServerState>,
    Json(payload): Json<CancelOrderRequest>,
) -> Result<Json<CancelOrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut engine_guard = state.engine.lock().await;
    
    // 주문 존재 확인
    if engine_guard.get_order(&payload.order_id).is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "ORDER_NOT_FOUND".to_string(),
                message: "주문을 찾을 수 없습니다".to_string(),
            }),
        ));
    }

    // 취소 주문 생성
    let cancel_order = Order::new_cancel(payload.order_id.clone());
    engine_guard.handle_cancel_order(&cancel_order);

    Ok(Json(CancelOrderResponse {
        order_id: payload.order_id,
        status: "CANCELLED".to_string(),
        message: "주문이 취소되었습니다".to_string(),
    }))
}

/// 주문서 조회 핸들러
pub async fn get_orderbook(
    State(state): State<ServerState>,
    Path(symbol): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<OrderBookResponse>, (StatusCode, Json<ErrorResponse>)> {
    let engine_guard = state.engine.lock().await;
    
    let depth = params
        .get("depth")
        .and_then(|d| d.parse::<usize>().ok())
        .unwrap_or(10);

    match engine_guard.get_order_book_snapshot(&symbol, depth) {
        Some(orderbook) => Ok(Json(OrderBookResponse { orderbook })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "SYMBOL_NOT_FOUND".to_string(),
                message: "지원하지 않는 심볼입니다".to_string(),
            }),
        )),
    }
}

/// 체결 내역 조회 핸들러
pub async fn get_executions(
    State(state): State<ServerState>,
    Path(symbol): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(100);

    let mdp_guard = state.mdp.lock().await;
    let executions = mdp_guard.get_executions(&symbol, limit).await;

    Ok(Json(ExecutionResponse { executions }))
}

/// 시장 통계 조회 핸들러
pub async fn get_statistics(
    State(state): State<ServerState>,
    Path(symbol): Path<String>,
) -> Result<Json<MarketStatisticsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mdp_guard = state.mdp.lock().await;
    
    if let Some(stats) = mdp_guard.get_statistics(&symbol).await {
        Ok(Json(MarketStatisticsResponse {
            symbol: stats.symbol,
            timestamp: stats.timestamp,
            open_price_24h: stats.open_price_24h,
            high_price_24h: stats.high_price_24h,
            low_price_24h: stats.low_price_24h,
            last_price: stats.last_price,
            volume_24h: stats.volume_24h,
            price_change_24h: stats.price_change_24h,
            bid_price: stats.bid_price,
            ask_price: stats.ask_price,
        }))
    } else {
        // 기본값 반환
        Ok(Json(MarketStatisticsResponse {
            symbol,
            timestamp: chrono::Utc::now().timestamp() as u64,
            open_price_24h: None,
            high_price_24h: None,
            low_price_24h: None,
            last_price: None,
            volume_24h: 0,
            price_change_24h: None,
            bid_price: None,
            ask_price: None,
        }))
    }
}

/// 봉차트 조회 핸들러
pub async fn get_candles(
    State(state): State<ServerState>,
    Path((symbol, interval)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<CandleResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(100);

    let mdp_guard = state.mdp.lock().await;
    let candlesticks = mdp_guard.get_candles(&symbol, &interval, limit).await;
    
    // CandlestickData를 CandleData로 변환
    let candles: Vec<CandleData> = candlesticks.into_iter().map(|c| CandleData {
        open_time: c.open_time,
        close_time: c.close_time,
        open: c.open,
        high: c.high,
        low: c.low,
        close: c.close,
        volume: c.volume,
        trade_count: c.trade_count,
    }).collect();

    Ok(Json(CandleResponse {
        symbol,
        interval,
        candles,
    }))
}