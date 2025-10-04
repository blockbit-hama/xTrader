//! RabbitMQ Consumer 구현 (Mock 버전)
//!
//! 이 모듈은 RabbitMQ Topic Exchange에서 WebSocket 알림을 소비하여
//! 다중 WebSocket 서버로 분배하는 기능을 제공합니다.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, warn};
use crate::mq::rabbitmq_producer::{WebSocketNotificationMessage, RoutingPatterns};

/// RabbitMQ Consumer Worker (Mock 구현)
pub struct RabbitMQConsumerWorker {
    exchange_name: String,
    queue_name: String,
    routing_patterns: Vec<String>,
    worker_id: String,
    batch_size: usize,
    processing_interval_ms: u64,
    messages_processed: Arc<Mutex<u64>>,
    connection_status: Arc<Mutex<bool>>,
}

/// Consumer Worker 설정
pub struct RabbitMQConsumerConfig {
    pub rabbitmq_url: String,
    pub exchange_name: String,
    pub queue_name: String,
    pub routing_patterns: Vec<String>,
    pub worker_id: String,
    pub batch_size: usize,
    pub processing_interval_ms: u64,
}

impl RabbitMQConsumerWorker {
    /// 새 RabbitMQ Consumer Worker 생성 (Mock)
    pub async fn new(config: RabbitMQConsumerConfig) -> Result<Self, String> {
        info!("RabbitMQ Consumer Worker 초기화 완료 (Mock): {} - {}", config.exchange_name, config.worker_id);
        
        Ok(Self {
            exchange_name: config.exchange_name,
            queue_name: config.queue_name,
            routing_patterns: config.routing_patterns,
            worker_id: config.worker_id,
            batch_size: config.batch_size,
            processing_interval_ms: config.processing_interval_ms,
            messages_processed: Arc::new(Mutex::new(0)),
            connection_status: Arc::new(Mutex::new(true)),
        })
    }

    /// Consumer Worker 실행 (메인 루프) (Mock)
    pub async fn run(&self) -> Result<(), String> {
        info!("RabbitMQ Consumer Worker 시작 (Mock): {}", self.worker_id);
        
        loop {
            match self.process_batch().await {
                Ok(processed_count) => {
                    if processed_count > 0 {
                        info!("Worker {} 처리 완료 (Mock): {}개 메시지", self.worker_id, processed_count);
                    }
                }
                Err(e) => {
                    error!("Worker {} 처리 오류 (Mock): {}", self.worker_id, e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
            }
            
            // 처리 간격 대기
            tokio::time::sleep(tokio::time::Duration::from_millis(self.processing_interval_ms)).await;
        }
    }

    /// 메시지 배치 처리 (Mock)
    async fn process_batch(&self) -> Result<usize, String> {
        // Mock: 실제로는 RabbitMQ에서 메시지를 폴링하지만, 여기서는 시뮬레이션
        let mut count = self.messages_processed.lock().await;
        let processed_count = (*count % 5) + 1; // 1-5개 사이의 랜덤한 메시지 처리
        *count += processed_count;
        
        // Mock 메시지 생성 및 처리
        for i in 0..processed_count {
            let mock_message = self.create_mock_message(i as usize);
            self.process_message(&mock_message).await?;
        }
        
        Ok(processed_count as usize)
    }

    /// Mock 메시지 생성
    fn create_mock_message(&self, index: usize) -> WebSocketNotificationMessage {
        let symbols = ["BTC-KRW", "ETH-KRW", "XRP-KRW"];
        let message_types = ["execution", "orderbook_delta", "market_statistics", "candlestick_update"];
        
        let symbol = symbols[index % symbols.len()];
        let message_type = message_types[index % message_types.len()];
        
        WebSocketNotificationMessage {
            message_id: format!("mock_msg_{}", index),
            routing_key: format!("{}.{}", message_type, symbol),
            message_type: message_type.to_string(),
            symbol: Some(symbol.to_string()),
            user_id: Some(format!("mock_user_{}", index % 10)),
            data: serde_json::json!({
                "price": 50000 + (index as u64 * 100),
                "quantity": 100 + (index as u64 * 10),
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
            }),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            priority: (index % 10) as u8,
        }
    }

    /// 개별 메시지 처리
    async fn process_message(&self, message: &WebSocketNotificationMessage) -> Result<(), String> {
        info!("WebSocket 메시지 처리 (Mock): {} - {} (우선순위: {})", 
              message.routing_key, message.message_id, message.priority);
        
        // 라우팅 패턴 매칭 확인
        if self.matches_routing_patterns(&message.routing_key) {
            self.distribute_to_websocket_servers(message).await?;
        } else {
            warn!("라우팅 패턴 불일치: {} (패턴: {:?})", 
                  message.routing_key, self.routing_patterns);
        }
        
        Ok(())
    }

    /// 라우팅 패턴 매칭 확인
    fn matches_routing_patterns(&self, routing_key: &str) -> bool {
        for pattern in &self.routing_patterns {
            if self.matches_pattern(pattern, routing_key) {
                return true;
            }
        }
        false
    }

    /// 와일드카드 패턴 매칭 (간단한 구현)
    fn matches_pattern(&self, pattern: &str, routing_key: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return routing_key.starts_with(prefix) && routing_key.ends_with(suffix);
            }
        }
        
        pattern == routing_key
    }

    /// WebSocket 서버로 분배
    async fn distribute_to_websocket_servers(&self, message: &WebSocketNotificationMessage) -> Result<(), String> {
        // Mock: 실제로는 여러 WebSocket 서버로 메시지를 전송
        let servers = ["ws-server-1", "ws-server-2", "ws-server-3"];
        
        for server in &servers {
            info!("WebSocket 서버로 메시지 전송 (Mock): {} -> {} (메시지: {})", 
                  server, message.routing_key, message.message_id);
            
            // TODO: 실제 WebSocket 서버로 메시지 전송
            // - WebSocket 연결 풀 관리
            // - 연결 실패 시 재시도
            // - 메시지 순서 보장
        }
        
        Ok(())
    }

    /// 연결 상태 확인
    pub async fn is_connected(&self) -> bool {
        let status = self.connection_status.lock().await;
        *status
    }

    /// 연결 상태 설정
    pub async fn set_connection_status(&self, connected: bool) {
        let mut status = self.connection_status.lock().await;
        *status = connected;
    }

    /// 처리된 메시지 수 조회
    pub async fn get_processed_count(&self) -> u64 {
        let count = self.messages_processed.lock().await;
        *count
    }
}

/// WebSocket 서버 Consumer
pub struct WebSocketServerConsumer {
    worker: RabbitMQConsumerWorker,
    server_id: String,
    max_connections: u32,
    current_connections: Arc<Mutex<u32>>,
}

impl WebSocketServerConsumer {
    /// 새 WebSocket 서버 Consumer 생성 (Mock)
    pub async fn new(config: RabbitMQConsumerConfig, server_id: String, max_connections: u32) -> Result<Self, String> {
        let worker = RabbitMQConsumerWorker::new(config).await?;
        
        info!("WebSocket 서버 Consumer 초기화 완료 (Mock): {}", server_id);
        
        Ok(Self {
            worker,
            server_id,
            max_connections,
            current_connections: Arc::new(Mutex::new(0)),
        })
    }

    /// WebSocket 서버 Consumer 실행 (Mock)
    pub async fn run(&self) -> Result<(), String> {
        info!("WebSocket 서버 Consumer 시작 (Mock): {}", self.server_id);
        
        // WebSocket 서버 전용 처리 로직
        self.worker.run().await
    }

    /// 연결 수 증가
    pub async fn increment_connections(&self) -> Result<(), String> {
        let mut connections = self.current_connections.lock().await;
        if *connections < self.max_connections {
            *connections += 1;
            info!("WebSocket 연결 증가 (Mock): {} ({}/{})", 
                  self.server_id, *connections, self.max_connections);
            Ok(())
        } else {
            Err(format!("최대 연결 수 초과: {} (최대: {})", *connections, self.max_connections))
        }
    }

    /// 연결 수 감소
    pub async fn decrement_connections(&self) {
        let mut connections = self.current_connections.lock().await;
        if *connections > 0 {
            *connections -= 1;
            info!("WebSocket 연결 감소 (Mock): {} ({}/{})", 
                  self.server_id, *connections, self.max_connections);
        }
    }

    /// 현재 연결 수 조회
    pub async fn get_current_connections(&self) -> u32 {
        let connections = self.current_connections.lock().await;
        *connections
    }

    /// 서버 상태 조회
    pub async fn get_server_status(&self) -> ServerStatus {
        let connections = self.current_connections.lock().await;
        let processed_count = self.worker.get_processed_count().await;
        let is_connected = self.worker.is_connected().await;
        
        ServerStatus {
            server_id: self.server_id.clone(),
            current_connections: *connections,
            max_connections: self.max_connections,
            processed_messages: processed_count,
            is_connected,
            last_activity: std::time::SystemTime::now(),
        }
    }
}

/// 로드밸런서 Consumer
pub struct LoadBalancerConsumer {
    workers: Vec<Arc<WebSocketServerConsumer>>,
    current_index: Arc<Mutex<usize>>,
}

impl LoadBalancerConsumer {
    /// 새 로드밸런서 Consumer 생성 (Mock)
    pub async fn new(server_configs: Vec<(RabbitMQConsumerConfig, String, u32)>) -> Result<Self, String> {
        let mut workers = Vec::new();
        
        for (config, server_id, max_connections) in server_configs {
            let worker = Arc::new(WebSocketServerConsumer::new(config, server_id, max_connections).await?);
            workers.push(worker);
        }
        
        info!("로드밸런서 Consumer 초기화 완료 (Mock): {}개 서버", workers.len());
        
        Ok(Self {
            workers,
            current_index: Arc::new(Mutex::new(0)),
        })
    }

    /// 로드밸런서 Consumer 실행 (Mock)
    pub async fn run(&self) -> Result<(), String> {
        info!("로드밸런서 Consumer 시작 (Mock)");
        
        // 모든 서버 Worker 시작
        let mut handles = Vec::new();
        
        for worker in &self.workers {
            let worker_clone = worker.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = worker_clone.run().await {
                    error!("WebSocket 서버 Worker 실행 오류: {}", e);
                }
            });
            handles.push(handle);
        }
        
        // 모든 Worker가 종료될 때까지 대기
        for handle in handles {
            let _ = handle.await;
        }
        
        Ok(())
    }

    /// 라운드 로빈으로 서버 선택
    pub async fn select_server(&self) -> Option<Arc<WebSocketServerConsumer>> {
        if self.workers.is_empty() {
            return None;
        }
        
        let mut index = self.current_index.lock().await;
        let selected = self.workers[*index].clone();
        *index = (*index + 1) % self.workers.len();
        
        Some(selected)
    }

    /// 최소 연결 수를 가진 서버 선택
    pub async fn select_least_loaded_server(&self) -> Option<Arc<WebSocketServerConsumer>> {
        if self.workers.is_empty() {
            return None;
        }
        
        let mut min_connections = u32::MAX;
        let mut selected_server = None;
        
        for worker in &self.workers {
            let connections = worker.get_current_connections().await;
            if connections < min_connections {
                min_connections = connections;
                selected_server = Some(worker.clone());
            }
        }
        
        selected_server
    }

    /// 모든 서버 상태 조회
    pub async fn get_all_server_status(&self) -> Vec<ServerStatus> {
        let mut statuses = Vec::new();
        
        for worker in &self.workers {
            let status = worker.get_server_status().await;
            statuses.push(status);
        }
        
        statuses
    }
}

/// 서버 상태 정보
#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub server_id: String,
    pub current_connections: u32,
    pub max_connections: u32,
    pub processed_messages: u64,
    pub is_connected: bool,
    pub last_activity: std::time::SystemTime,
}

/// Dead Letter Queue Consumer
pub struct DeadLetterQueueConsumer {
    worker: RabbitMQConsumerWorker,
    retry_count: Arc<Mutex<u32>>,
    max_retries: u32,
}

impl DeadLetterQueueConsumer {
    /// 새 Dead Letter Queue Consumer 생성 (Mock)
    pub async fn new(config: RabbitMQConsumerConfig, max_retries: u32) -> Result<Self, String> {
        let worker = RabbitMQConsumerWorker::new(config).await?;
        
        info!("Dead Letter Queue Consumer 초기화 완료 (Mock)");
        
        Ok(Self {
            worker,
            retry_count: Arc::new(Mutex::new(0)),
            max_retries,
        })
    }

    /// Dead Letter Queue Consumer 실행 (Mock)
    pub async fn run(&self) -> Result<(), String> {
        info!("Dead Letter Queue Consumer 시작 (Mock)");
        
        // Dead Letter Queue 전용 처리 로직
        self.worker.run().await
    }

    /// 실패한 메시지 재처리
    pub async fn retry_failed_message(&self, message: &WebSocketNotificationMessage) -> Result<(), String> {
        let mut retry_count = self.retry_count.lock().await;
        
        if *retry_count < self.max_retries {
            *retry_count += 1;
            info!("실패한 메시지 재처리 (Mock): {} (재시도: {}/{})", 
                  message.message_id, *retry_count, self.max_retries);
            
            // TODO: 실제 재처리 로직 구현
            // - 원본 큐로 메시지 재전송
            // - 지수 백오프 적용
            // - 최대 재시도 횟수 초과 시 영구 실패 처리
            
            Ok(())
        } else {
            error!("최대 재시도 횟수 초과 (Mock): {}", message.message_id);
            Err(format!("최대 재시도 횟수 초과: {}", self.max_retries))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rabbitmq_consumer_config() {
        let config = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "ws-server-1".to_string(),
            routing_patterns: vec!["execution.*".to_string(), "orderbook.*".to_string()],
            worker_id: "ws-worker-1".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        assert_eq!(config.worker_id, "ws-worker-1");
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.routing_patterns.len(), 2);
    }

    #[tokio::test]
    async fn test_rabbitmq_consumer_worker_mock() {
        let config = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "ws-server-1".to_string(),
            routing_patterns: vec!["execution.*".to_string()],
            worker_id: "ws-worker-1".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        let worker = RabbitMQConsumerWorker::new(config).await.unwrap();
        assert_eq!(worker.worker_id, "ws-worker-1");
        assert_eq!(worker.exchange_name, "websocket_notifications");
    }

    #[tokio::test]
    async fn test_pattern_matching() {
        let config = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "ws-server-1".to_string(),
            routing_patterns: vec!["execution.*".to_string(), "orderbook.*".to_string()],
            worker_id: "ws-worker-1".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        let worker = RabbitMQConsumerWorker::new(config).await.unwrap();
        
        // 패턴 매칭 테스트
        assert!(worker.matches_routing_patterns("execution.BTC-KRW"));
        assert!(worker.matches_routing_patterns("orderbook.delta.BTC-KRW"));
        assert!(!worker.matches_routing_patterns("market.stats.BTC-KRW"));
    }

    #[tokio::test]
    async fn test_websocket_server_consumer_mock() {
        let config = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "ws-server-1".to_string(),
            routing_patterns: vec!["execution.*".to_string()],
            worker_id: "ws-worker-1".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        let consumer = WebSocketServerConsumer::new(config, "ws-server-1".to_string(), 1000).await.unwrap();
        assert_eq!(consumer.server_id, "ws-server-1");
        assert_eq!(consumer.max_connections, 1000);
        
        // 연결 수 증가 테스트
        let result = consumer.increment_connections().await;
        assert!(result.is_ok());
        
        let connections = consumer.get_current_connections().await;
        assert_eq!(connections, 1);
    }

    #[tokio::test]
    async fn test_load_balancer_consumer_mock() {
        let config1 = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "ws-server-1".to_string(),
            routing_patterns: vec!["execution.*".to_string()],
            worker_id: "ws-worker-1".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        let config2 = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "ws-server-2".to_string(),
            routing_patterns: vec!["execution.*".to_string()],
            worker_id: "ws-worker-2".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        let server_configs = vec![
            (config1, "ws-server-1".to_string(), 1000),
            (config2, "ws-server-2".to_string(), 1000),
        ];
        
        let load_balancer = LoadBalancerConsumer::new(server_configs).await.unwrap();
        
        // 서버 선택 테스트
        let server1 = load_balancer.select_server().await;
        assert!(server1.is_some());
        
        let server2 = load_balancer.select_server().await;
        assert!(server2.is_some());
        
        // 서버 상태 조회 테스트
        let statuses = load_balancer.get_all_server_status().await;
        assert_eq!(statuses.len(), 2);
    }
}