//! RabbitMQ Producer 구현 (Mock 버전)
//!
//! 이 모듈은 WebSocket 알림을 RabbitMQ Topic Exchange에 발행하여
//! 다중 WebSocket 서버로 분배하는 기능을 제공합니다.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};
use crate::api::models::WebSocketMessage;

/// RabbitMQ Producer (Mock 구현)
pub struct RabbitMQProducer {
    exchange_name: String,
    messages_sent: Arc<Mutex<u64>>,
}

/// WebSocket 알림 메시지 구조
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebSocketNotificationMessage {
    pub message_id: String,
    pub routing_key: String,
    pub message_type: String,
    pub symbol: Option<String>,
    pub user_id: Option<String>,
    pub data: serde_json::Value,
    pub timestamp: u64,
    pub priority: u8, // 0-9 (0: 높음, 9: 낮음)
}

impl From<&WebSocketMessage> for WebSocketNotificationMessage {
    fn from(ws_message: &WebSocketMessage) -> Self {
        let (routing_key, symbol, user_id, data) = match ws_message {
            WebSocketMessage::Execution(execution) => {
                (
                    format!("execution.{}", execution.symbol),
                    Some(execution.symbol.clone()),
                    None, // TODO: ExecutionReport에 user_id 추가 필요
                    serde_json::to_value(execution).unwrap_or_default(),
                )
            }
            WebSocketMessage::OrderBookDelta(delta) => {
                (
                    format!("orderbook.delta.{}", delta.symbol),
                    Some(delta.symbol.clone()),
                    None,
                    serde_json::to_value(delta).unwrap_or_default(),
                )
            }
            WebSocketMessage::OrderBookSnapshot(snapshot) => {
                (
                    format!("orderbook.snapshot.{}", snapshot.symbol),
                    Some(snapshot.symbol.clone()),
                    None,
                    serde_json::to_value(snapshot).unwrap_or_default(),
                )
            }
            WebSocketMessage::MarketStatistics { symbol, .. } => {
                (
                    format!("market.stats.{}", symbol),
                    Some(symbol.clone()),
                    None,
                    serde_json::to_value(ws_message).unwrap_or_default(),
                )
            }
            WebSocketMessage::CandlestickUpdate { symbol, .. } => {
                (
                    format!("candlestick.{}", symbol),
                    Some(symbol.clone()),
                    None,
                    serde_json::to_value(ws_message).unwrap_or_default(),
                )
            }
            WebSocketMessage::SyncResponse { symbol, snapshot } => {
                (
                    format!("sync.response.{}", symbol),
                    Some(symbol.clone()),
                    None,
                    serde_json::to_value(snapshot).unwrap_or_default(),
                )
            }
            WebSocketMessage::Error { message } => {
                (
                    "error.general".to_string(),
                    None,
                    None,
                    serde_json::json!({"message": message}),
                )
            }
            _ => {
                (
                    "general.notification".to_string(),
                    None,
                    None,
                    serde_json::to_value(ws_message).unwrap_or_default(),
                )
            }
        };

        Self {
            message_id: uuid::Uuid::new_v4().to_string(),
            routing_key,
            message_type: Self::get_message_type(ws_message),
            symbol,
            user_id,
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            priority: Self::get_priority_for_message_type(&Self::get_message_type(ws_message)),
        }
    }
}

impl WebSocketNotificationMessage {
    /// WebSocket 메시지 타입 추출
    fn get_message_type(ws_message: &WebSocketMessage) -> String {
        match ws_message {
            WebSocketMessage::Execution(_) => "execution".to_string(),
            WebSocketMessage::OrderBookDelta(_) => "orderbook_delta".to_string(),
            WebSocketMessage::OrderBookSnapshot(_) => "orderbook_snapshot".to_string(),
            WebSocketMessage::OrderBookUpdate { .. } => "orderbook_update".to_string(),
            WebSocketMessage::MarketStatistics { .. } => "market_statistics".to_string(),
            WebSocketMessage::CandlestickUpdate { .. } => "candlestick_update".to_string(),
            WebSocketMessage::SyncResponse { .. } => "sync_response".to_string(),
            WebSocketMessage::Error { .. } => "error".to_string(),
        }
    }

    /// 메시지 타입별 우선순위 설정
    fn get_priority_for_message_type(message_type: &str) -> u8 {
        match message_type {
            "execution" => 1,        // 체결: 높은 우선순위
            "orderbook_delta" => 2,  // 호가창 변경: 높은 우선순위
            "orderbook_snapshot" => 3, // 호가창 스냅샷: 중간 우선순위
            "market_statistics" => 4, // 시장 통계: 중간 우선순위
            "candlestick_update" => 5, // 봉차트: 낮은 우선순위
            "sync_response" => 1,    // 동기화 응답: 높은 우선순위
            "error" => 0,            // 에러: 최고 우선순위
            _ => 6,                  // 기타: 낮은 우선순위
        }
    }

    /// 사용자별 라우팅 키 생성
    pub fn with_user_routing(&self, user_id: &str) -> String {
        if let Some(symbol) = &self.symbol {
            format!("user.{}.{}.{}", user_id, self.message_type, symbol)
        } else {
            format!("user.{}.{}", user_id, self.message_type)
        }
    }

    /// 심볼별 라우팅 키 생성
    pub fn with_symbol_routing(&self) -> String {
        if let Some(symbol) = &self.symbol {
            format!("symbol.{}.{}", symbol, self.message_type)
        } else {
            self.routing_key.clone()
        }
    }
}

/// RabbitMQ 오류 타입 (Mock)
#[derive(Debug)]
pub enum RabbitMQError {
    ConnectionError(String),
    SerializationError(String),
    PublishError(String),
    ExchangeError(String),
}

impl std::fmt::Display for RabbitMQError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RabbitMQError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            RabbitMQError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            RabbitMQError::PublishError(msg) => write!(f, "Publish error: {}", msg),
            RabbitMQError::ExchangeError(msg) => write!(f, "Exchange error: {}", msg),
        }
    }
}

impl std::error::Error for RabbitMQError {}

impl RabbitMQProducer {
    /// 새 RabbitMQ Producer 생성 (Mock)
    pub async fn new(rabbitmq_url: &str, exchange_name: &str) -> Result<Self, RabbitMQError> {
        info!("RabbitMQ Producer 초기화 완료 (Mock): {} -> {}", rabbitmq_url, exchange_name);
        
        Ok(Self {
            exchange_name: exchange_name.to_string(),
            messages_sent: Arc::new(Mutex::new(0)),
        })
    }

    /// WebSocket 메시지를 RabbitMQ에 발행 (Mock)
    pub async fn publish_websocket_message(&self, ws_message: &WebSocketMessage) -> Result<String, RabbitMQError> {
        let notification = WebSocketNotificationMessage::from(ws_message);
        let message_json = serde_json::to_string(&notification)
            .map_err(|e| RabbitMQError::SerializationError(e.to_string()))?;
        
        // Mock: 메시지 카운터 증가
        let mut count = self.messages_sent.lock().await;
        *count += 1;
        
        info!("WebSocket 메시지 RabbitMQ 발행 완료 (Mock): {} -> {} (메시지 #{}: {})", 
              notification.message_id, notification.routing_key, *count, message_json);
        
        Ok(notification.message_id)
    }

    /// 사용자별 WebSocket 메시지 발행 (Mock)
    pub async fn publish_user_message(&self, ws_message: &WebSocketMessage, user_id: &str) -> Result<String, RabbitMQError> {
        let mut notification = WebSocketNotificationMessage::from(ws_message);
        notification.routing_key = notification.with_user_routing(user_id);
        notification.user_id = Some(user_id.to_string());
        
        let message_json = serde_json::to_string(&notification)
            .map_err(|e| RabbitMQError::SerializationError(e.to_string()))?;
        
        let mut count = self.messages_sent.lock().await;
        *count += 1;
        
        info!("사용자별 WebSocket 메시지 RabbitMQ 발행 완료 (Mock): {} -> {} (메시지 #{}: {})", 
              notification.message_id, notification.routing_key, *count, message_json);
        
        Ok(notification.message_id)
    }

    /// 심볼별 WebSocket 메시지 발행 (Mock)
    pub async fn publish_symbol_message(&self, ws_message: &WebSocketMessage) -> Result<String, RabbitMQError> {
        let mut notification = WebSocketNotificationMessage::from(ws_message);
        notification.routing_key = notification.with_symbol_routing();
        
        let message_json = serde_json::to_string(&notification)
            .map_err(|e| RabbitMQError::SerializationError(e.to_string()))?;
        
        let mut count = self.messages_sent.lock().await;
        *count += 1;
        
        info!("심볼별 WebSocket 메시지 RabbitMQ 발행 완료 (Mock): {} -> {} (메시지 #{}: {})", 
              notification.message_id, notification.routing_key, *count, message_json);
        
        Ok(notification.message_id)
    }

    /// 배치로 WebSocket 메시지 발행 (Mock)
    pub async fn publish_batch_messages(&self, messages: &[(WebSocketMessage, Option<String>)]) -> Result<usize, RabbitMQError> {
        if messages.is_empty() {
            return Ok(0);
        }
        
        let mut count = self.messages_sent.lock().await;
        let mut published_count = 0;
        
        for (ws_message, user_id) in messages {
            let notification = WebSocketNotificationMessage::from(ws_message);
            let message_json = serde_json::to_string(&notification)
                .map_err(|e| RabbitMQError::SerializationError(e.to_string()))?;
            
            info!("배치 WebSocket 메시지 RabbitMQ 발행 (Mock): {} -> {} (메시지 #{}: {})", 
                  notification.message_id, notification.routing_key, *count + published_count + 1, message_json);
            
            published_count += 1;
        }
        
        *count += published_count;
        
        info!("배치 WebSocket 메시지 RabbitMQ 발행 완료 (Mock): {}개 (총 메시지: {})", published_count, *count);
        
        Ok(published_count as usize)
    }

    /// Producer 상태 조회
    pub async fn get_producer_stats(&self) -> Result<ProducerStats, RabbitMQError> {
        let count = self.messages_sent.lock().await;
        
        Ok(ProducerStats {
            exchange_name: self.exchange_name.clone(),
            messages_sent: *count,
            last_send_time: std::time::SystemTime::now(),
        })
    }

    /// Exchange 설정 (Mock)
    pub async fn setup_exchange(&self) -> Result<(), RabbitMQError> {
        info!("RabbitMQ Exchange 설정 완료 (Mock): {} (Topic Exchange)", self.exchange_name);
        Ok(())
    }

    /// Dead Letter Queue 설정 (Mock)
    pub async fn setup_dead_letter_queue(&self) -> Result<(), RabbitMQError> {
        info!("RabbitMQ Dead Letter Queue 설정 완료 (Mock): {}.dlq", self.exchange_name);
        Ok(())
    }
}

/// Producer 통계 정보
#[derive(Debug, Clone)]
pub struct ProducerStats {
    pub exchange_name: String,
    pub messages_sent: u64,
    pub last_send_time: std::time::SystemTime,
}

/// 라우팅 키 패턴 정의
pub struct RoutingPatterns;

impl RoutingPatterns {
    /// 모든 체결 메시지
    pub const EXECUTION_ALL: &'static str = "execution.*";
    
    /// 특정 심볼 체결 메시지
    pub fn execution_symbol(symbol: &str) -> String {
        format!("execution.{}", symbol)
    }
    
    /// 모든 호가창 메시지
    pub const ORDERBOOK_ALL: &'static str = "orderbook.*";
    
    /// 특정 심볼 호가창 메시지
    pub fn orderbook_symbol(symbol: &str) -> String {
        format!("orderbook.*.{}", symbol)
    }
    
    /// 모든 시장 통계 메시지
    pub const MARKET_STATS_ALL: &'static str = "market.stats.*";
    
    /// 특정 심볼 시장 통계 메시지
    pub fn market_stats_symbol(symbol: &str) -> String {
        format!("market.stats.{}", symbol)
    }
    
    /// 모든 봉차트 메시지
    pub const CANDLESTICK_ALL: &'static str = "candlestick.*";
    
    /// 특정 심볼 봉차트 메시지
    pub fn candlestick_symbol(symbol: &str) -> String {
        format!("candlestick.{}", symbol)
    }
    
    /// 사용자별 메시지 패턴
    pub fn user_pattern(user_id: &str) -> String {
        format!("user.{}.*", user_id)
    }
    
    /// 심볼별 메시지 패턴
    pub fn symbol_pattern(symbol: &str) -> String {
        format!("symbol.{}.*", symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::WebSocketMessage;
    use crate::matching_engine::model::{ExecutionReport, Side};

    fn create_test_execution() -> ExecutionReport {
        ExecutionReport {
            execution_id: "exec_001".to_string(),
            symbol: "BTC-KRW".to_string(),
            side: Side::Buy,
            price: 50000,
            quantity: 100,
            timestamp: 1234567890,
            order_id: "order_001".to_string(),
            user_id: "user_001".to_string(),
            is_maker: true,
        }
    }

    #[tokio::test]
    async fn test_websocket_notification_message_conversion() {
        let execution = create_test_execution();
        let ws_message = WebSocketMessage::Execution(execution);
        let notification = WebSocketNotificationMessage::from(&ws_message);
        
        assert_eq!(notification.message_type, "execution");
        assert_eq!(notification.routing_key, "execution.BTC-KRW");
        assert_eq!(notification.symbol, Some("BTC-KRW".to_string()));
        assert_eq!(notification.priority, 1); // 체결은 높은 우선순위
    }

    #[tokio::test]
    async fn test_user_routing_key() {
        let execution = create_test_execution();
        let ws_message = WebSocketMessage::Execution(execution);
        let notification = WebSocketNotificationMessage::from(&ws_message);
        
        let user_routing = notification.with_user_routing("user_123");
        assert_eq!(user_routing, "user.user_123.execution.BTC-KRW");
    }

    #[tokio::test]
    async fn test_symbol_routing_key() {
        let execution = create_test_execution();
        let ws_message = WebSocketMessage::Execution(execution);
        let notification = WebSocketNotificationMessage::from(&ws_message);
        
        let symbol_routing = notification.with_symbol_routing();
        assert_eq!(symbol_routing, "symbol.BTC-KRW.execution");
    }

    #[tokio::test]
    async fn test_rabbitmq_producer_mock() {
        let producer = RabbitMQProducer::new("amqp://localhost:5672", "websocket_notifications").await.unwrap();
        
        let execution = create_test_execution();
        let ws_message = WebSocketMessage::Execution(execution);
        let result = producer.publish_websocket_message(&ws_message).await;
        
        assert!(result.is_ok());
        
        let stats = producer.get_producer_stats().await.unwrap();
        assert_eq!(stats.messages_sent, 1);
        assert_eq!(stats.exchange_name, "websocket_notifications");
    }

    #[tokio::test]
    async fn test_routing_patterns() {
        assert_eq!(RoutingPatterns::execution_symbol("BTC-KRW"), "execution.BTC-KRW");
        assert_eq!(RoutingPatterns::orderbook_symbol("BTC-KRW"), "orderbook.*.BTC-KRW");
        assert_eq!(RoutingPatterns::market_stats_symbol("BTC-KRW"), "market.stats.BTC-KRW");
        assert_eq!(RoutingPatterns::candlestick_symbol("BTC-KRW"), "candlestick.BTC-KRW");
        assert_eq!(RoutingPatterns::user_pattern("user_123"), "user.user_123.*");
        assert_eq!(RoutingPatterns::symbol_pattern("BTC-KRW"), "symbol.BTC-KRW.*");
    }

    #[tokio::test]
    async fn test_message_priority() {
        let execution = create_test_execution();
        let ws_message = WebSocketMessage::Execution(execution);
        let notification = WebSocketNotificationMessage::from(&ws_message);
        
        assert_eq!(notification.priority, 1); // 체결은 높은 우선순위
        
        let error_message = WebSocketMessage::Error { message: "Test error".to_string() };
        let error_notification = WebSocketNotificationMessage::from(&error_message);
        
        assert_eq!(error_notification.priority, 0); // 에러는 최고 우선순위
    }
}