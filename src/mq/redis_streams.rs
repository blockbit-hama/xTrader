//! Redis Streams Producer 및 Consumer 구현 (간단 버전)
//!
//! 이 모듈은 체결 내역을 Redis Streams에 발행하는 기본 기능을 제공합니다.

use redis::{Client, RedisResult, AsyncCommands};
use redis::aio::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};
use crate::matching_engine::model::ExecutionReport;

/// Redis Streams Producer
pub struct RedisStreamsProducer {
    client: Arc<Client>,
    connection: Arc<Mutex<Connection>>,
    stream_name: String,
}

/// 체결 내역 메시지 구조
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExecutionMessage {
    pub execution_id: String,
    pub symbol: String,
    pub side: String,
    pub price: u64,
    pub quantity: u64,
    pub timestamp: u64,
    pub order_id: String,
    pub user_id: String,
}

impl From<&ExecutionReport> for ExecutionMessage {
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
        }
    }
}

impl RedisStreamsProducer {
    /// 새 Producer 생성
    pub async fn new(redis_url: &str, stream_name: &str) -> RedisResult<Self> {
        let client = Client::open(redis_url)?;
        let connection = client.get_async_connection().await?;
        
        info!("Redis Streams Producer 초기화 완료: {}", stream_name);
        
        Ok(Self {
            client: Arc::new(client),
            connection: Arc::new(Mutex::new(connection)),
            stream_name: stream_name.to_string(),
        })
    }

    /// 체결 내역을 Redis Stream에 발행
    pub async fn publish_execution(&self, execution: &ExecutionReport) -> RedisResult<String> {
        let message = ExecutionMessage::from(execution);
        let message_json = serde_json::to_string(&message)
            .map_err(|e| redis::RedisError::from((redis::ErrorKind::TypeError, "JSON serialization error")))?;
        
        let mut conn = self.connection.lock().await;
        
        // XADD 명령으로 스트림에 메시지 추가
        let message_id: String = conn.xadd(
            &self.stream_name,
            "*", // 자동 생성 ID
            &[("execution", &message_json)]
        ).await?;
        
        info!("체결 내역 발행 완료: {} -> {}", execution.execution_id, message_id);
        Ok(message_id)
    }

    /// 배치로 체결 내역 발행
    pub async fn publish_executions_batch(&self, executions: &[ExecutionReport]) -> RedisResult<Vec<String>> {
        let mut conn = self.connection.lock().await;
        let mut message_ids = Vec::new();
        
        for execution in executions {
            let message = ExecutionMessage::from(execution);
            let message_json = serde_json::to_string(&message)
                .map_err(|e| redis::RedisError::from((redis::ErrorKind::TypeError, "JSON serialization error")))?;
            
            let message_id: String = conn.xadd(
                &self.stream_name,
                "*",
                &[("execution", &message_json)]
            ).await?;
            
            message_ids.push(message_id);
        }
        
        info!("배치 체결 내역 발행 완료: {}개", executions.len());
        Ok(message_ids)
    }
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
    async fn test_execution_message_conversion() {
        let execution = create_test_execution();
        let message = ExecutionMessage::from(&execution);
        
        assert_eq!(message.execution_id, "exec_001");
        assert_eq!(message.symbol, "BTC-KRW");
        assert_eq!(message.side, "Buy");
        assert_eq!(message.price, 50000);
        assert_eq!(message.quantity, 100);
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let execution = create_test_execution();
        let message = ExecutionMessage::from(&execution);
        
        let json = serde_json::to_string(&message).unwrap();
        let deserialized: ExecutionMessage = serde_json::from_str(&json).unwrap();
        
        assert_eq!(message.execution_id, deserialized.execution_id);
        assert_eq!(message.symbol, deserialized.symbol);
        assert_eq!(message.price, deserialized.price);
    }
}