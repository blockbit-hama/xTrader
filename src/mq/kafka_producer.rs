//! Apache Kafka Producer 구현 (Mock 버전)
//!
//! 이 모듈은 체결 내역을 Kafka Topic에 발행하는 기능을 Mock으로 구현합니다.
//! 실제 Kafka 라이브러리 대신 로깅과 메모리 저장을 사용합니다.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};
use crate::matching_engine::model::ExecutionReport;

/// Kafka Producer (Mock 구현)
pub struct KafkaProducer {
    topic_name: String,
    messages_sent: Arc<Mutex<u64>>,
}

/// 시장 데이터 메시지 구조
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketDataMessage {
    pub execution_id: String,
    pub symbol: String,
    pub side: String,
    pub price: u64,
    pub quantity: u64,
    pub timestamp: u64,
    pub order_id: String,
    pub user_id: String,
    pub message_type: String, // "execution", "orderbook_update", "market_statistics"
}

impl From<&ExecutionReport> for MarketDataMessage {
    fn from(report: &ExecutionReport) -> Self {
        Self {
            execution_id: report.execution_id.clone(),
            symbol: report.symbol.clone(),
            side: format!("{:?}", report.side),
            price: report.price,
            quantity: report.quantity,
            timestamp: report.timestamp,
            order_id: report.order_id.clone(),
            user_id: "user_placeholder".to_string(), // TODO: ExecutionReport에 user_id 필드 추가 필요
            message_type: "execution".to_string(),
        }
    }
}

/// Kafka 오류 타입 (Mock)
#[derive(Debug)]
pub enum KafkaError {
    ConnectionError(String),
    SerializationError(String),
    SendError(String),
}

impl std::fmt::Display for KafkaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KafkaError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            KafkaError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            KafkaError::SendError(msg) => write!(f, "Send error: {}", msg),
        }
    }
}

impl std::error::Error for KafkaError {}

impl KafkaProducer {
    /// 새 Kafka Producer 생성 (Mock)
    pub async fn new(kafka_brokers: &[String], topic_name: &str) -> Result<Self, KafkaError> {
        info!("Kafka Producer 초기화 완료 (Mock): {} -> {}", kafka_brokers.join(","), topic_name);
        
        Ok(Self {
            topic_name: topic_name.to_string(),
            messages_sent: Arc::new(Mutex::new(0)),
        })
    }

    /// 체결 내역을 Kafka Topic에 발행 (Mock)
    pub async fn publish_execution(&self, execution: &ExecutionReport) -> Result<(), KafkaError> {
        let message = MarketDataMessage::from(execution);
        let message_json = serde_json::to_string(&message)
            .map_err(|e| KafkaError::SerializationError(e.to_string()))?;
        
        // Mock: 메시지 카운터 증가
        let mut count = self.messages_sent.lock().await;
        *count += 1;
        
        info!("체결 내역 Kafka 발행 완료 (Mock): {} -> {} (메시지 #{}: {})", 
              execution.execution_id, execution.symbol, *count, message_json);
        
        Ok(())
    }

    /// 배치로 체결 내역 발행 (Mock)
    pub async fn publish_executions_batch(&self, executions: &[ExecutionReport]) -> Result<usize, KafkaError> {
        if executions.is_empty() {
            return Ok(0);
        }
        
        let mut count = self.messages_sent.lock().await;
        *count += executions.len() as u64;
        
        info!("배치 체결 내역 Kafka 발행 완료 (Mock): {}개 (총 메시지: {})", executions.len(), *count);
        
        Ok(executions.len())
    }

    /// 시장 통계 발행 (Mock)
    pub async fn publish_market_statistics(&self, symbol: &str, stats: &MarketStatisticsMessage) -> Result<(), KafkaError> {
        let message_json = serde_json::to_string(stats)
            .map_err(|e| KafkaError::SerializationError(e.to_string()))?;
        
        let mut count = self.messages_sent.lock().await;
        *count += 1;
        
        info!("시장 통계 Kafka 발행 완료 (Mock): {} (메시지 #{}: {})", symbol, *count, message_json);
        
        Ok(())
    }

    /// 호가창 업데이트 발행 (Mock)
    pub async fn publish_orderbook_update(&self, symbol: &str, orderbook: &OrderBookUpdateMessage) -> Result<(), KafkaError> {
        let message_json = serde_json::to_string(orderbook)
            .map_err(|e| KafkaError::SerializationError(e.to_string()))?;
        
        let mut count = self.messages_sent.lock().await;
        *count += 1;
        
        info!("호가창 업데이트 Kafka 발행 완료 (Mock): {} (메시지 #{}: {})", symbol, *count, message_json);
        
        Ok(())
    }

    /// Producer 상태 조회
    pub async fn get_producer_stats(&self) -> Result<ProducerStats, KafkaError> {
        let count = self.messages_sent.lock().await;
        
        Ok(ProducerStats {
            topic_name: self.topic_name.clone(),
            messages_sent: *count,
            last_send_time: std::time::SystemTime::now(),
        })
    }
}

/// 시장 통계 메시지
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketStatisticsMessage {
    pub symbol: String,
    pub timestamp: u64,
    pub message_type: String,
    pub volume_24h: u64,
    pub price_change_24h: f64,
    pub high_24h: u64,
    pub low_24h: u64,
    pub trades_count_24h: u64,
}

/// 호가창 업데이트 메시지
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderBookUpdateMessage {
    pub symbol: String,
    pub timestamp: u64,
    pub message_type: String,
    pub bids: Vec<(u64, u64)>,
    pub asks: Vec<(u64, u64)>,
    pub sequence: u64,
}

/// Producer 통계 정보
#[derive(Debug, Clone)]
pub struct ProducerStats {
    pub topic_name: String,
    pub messages_sent: u64,
    pub last_send_time: std::time::SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_market_data_message_conversion() {
        let execution = create_test_execution();
        let message = MarketDataMessage::from(&execution);
        
        assert_eq!(message.execution_id, "exec_001");
        assert_eq!(message.symbol, "BTC-KRW");
        assert_eq!(message.side, "Buy");
        assert_eq!(message.price, 50000);
        assert_eq!(message.quantity, 100);
        assert_eq!(message.message_type, "execution");
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let execution = create_test_execution();
        let message = MarketDataMessage::from(&execution);
        
        let json = serde_json::to_string(&message).unwrap();
        let deserialized: MarketDataMessage = serde_json::from_str(&json).unwrap();
        
        assert_eq!(message.execution_id, deserialized.execution_id);
        assert_eq!(message.symbol, deserialized.symbol);
        assert_eq!(message.price, deserialized.price);
    }

    #[tokio::test]
    async fn test_kafka_producer_mock() {
        let producer = KafkaProducer::new(&["localhost:9092".to_string()], "test-topic").await.unwrap();
        
        let execution = create_test_execution();
        let result = producer.publish_execution(&execution).await;
        
        assert!(result.is_ok());
        
        let stats = producer.get_producer_stats().await.unwrap();
        assert_eq!(stats.messages_sent, 1);
        assert_eq!(stats.topic_name, "test-topic");
    }

    #[tokio::test]
    async fn test_market_statistics_message() {
        let stats = MarketStatisticsMessage {
            symbol: "BTC-KRW".to_string(),
            timestamp: 1234567890,
            message_type: "market_statistics".to_string(),
            volume_24h: 1000000,
            price_change_24h: 5.5,
            high_24h: 52000,
            low_24h: 48000,
            trades_count_24h: 1500,
        };
        
        assert_eq!(stats.symbol, "BTC-KRW");
        assert_eq!(stats.message_type, "market_statistics");
        assert_eq!(stats.volume_24h, 1000000);
    }
}