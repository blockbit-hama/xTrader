//! 로컬 백업 큐 시스템
//!
//! 이 모듈은 MQ 장애 시 메시지를 로컬에 백업하고
//! 복구 시 자동으로 재발행하는 기능을 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, error, warn, debug};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, BufReader, Write, Read, BufRead};
use std::path::Path;

/// 백업 메시지 구조
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupMessage {
    pub id: String,
    pub mq_type: MQType,
    pub topic_stream: String,
    pub routing_key: Option<String>,
    pub message_data: serde_json::Value,
    pub timestamp: u64,
    pub retry_count: u32,
    pub max_retries: u32,
    pub priority: u8,
    pub created_at: u64,
}

/// MQ 타입
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MQType {
    RedisStreams,
    Kafka,
    RabbitMQ,
}

impl std::fmt::Display for MQType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MQType::RedisStreams => write!(f, "RedisStreams"),
            MQType::Kafka => write!(f, "Kafka"),
            MQType::RabbitMQ => write!(f, "RabbitMQ"),
        }
    }
}

/// 백업 큐 상태
#[derive(Debug, Clone)]
pub struct BackupQueueStats {
    pub total_messages: usize,
    pub pending_messages: usize,
    pub failed_messages: usize,
    pub oldest_message_age: u64,
    pub last_backup_time: u64,
}

/// 로컬 백업 큐
pub struct LocalBackupQueue {
    /// 메모리 큐 (빠른 접근)
    memory_queue: Arc<Mutex<VecDeque<BackupMessage>>>,
    /// 디스크 백업 파일 경로
    backup_file_path: String,
    /// 최대 메모리 큐 크기
    max_memory_size: usize,
    /// 백업 간격 (밀리초)
    backup_interval_ms: u64,
    /// 마지막 백업 시간
    last_backup_time: Arc<Mutex<u64>>,
    /// 큐 통계
    stats: Arc<RwLock<BackupQueueStats>>,
}

impl LocalBackupQueue {
    /// 새 백업 큐 생성
    pub fn new(backup_file_path: String, max_memory_size: usize, backup_interval_ms: u64) -> Self {
        let stats = BackupQueueStats {
            total_messages: 0,
            pending_messages: 0,
            failed_messages: 0,
            oldest_message_age: 0,
            last_backup_time: 0,
        };

        Self {
            memory_queue: Arc::new(Mutex::new(VecDeque::new())),
            backup_file_path,
            max_memory_size,
            backup_interval_ms,
            last_backup_time: Arc::new(Mutex::new(0)),
            stats: Arc::new(RwLock::new(stats)),
        }
    }

    /// 메시지 백업 추가
    pub async fn backup_message(&self, message: BackupMessage) -> Result<(), String> {
        let mut queue = self.memory_queue.lock().await;
        
        // 메모리 큐 크기 확인
        if queue.len() >= self.max_memory_size {
            // 가장 오래된 메시지를 디스크로 이동
            if let Some(oldest_message) = queue.pop_front() {
                self.write_to_disk(&oldest_message).await?;
            }
        }
        
        queue.push_back(message.clone());
        
        // 통계 업데이트
        self.update_stats().await;
        
        debug!("메시지 백업 완료: {} ({})", message.id, message.mq_type);
        
        // 주기적 디스크 백업
        self.schedule_backup().await;
        
        Ok(())
    }

    /// 메시지 복구 (재발행용)
    pub async fn recover_messages(&self, mq_type: &MQType, limit: usize) -> Result<Vec<BackupMessage>, String> {
        let mut recovered_messages = Vec::new();
        
        // 메모리에서 복구
        let mut queue = self.memory_queue.lock().await;
        let mut temp_queue = VecDeque::new();
        
        while let Some(message) = queue.pop_front() {
            if message.mq_type == *mq_type && recovered_messages.len() < limit {
                recovered_messages.push(message);
            } else {
                temp_queue.push_back(message);
            }
        }
        
        *queue = temp_queue;
        
        // 디스크에서 복구
        if recovered_messages.len() < limit {
            let disk_messages = self.read_from_disk(mq_type, limit - recovered_messages.len()).await?;
            recovered_messages.extend(disk_messages);
        }
        
        // 통계 업데이트
        self.update_stats().await;
        
        info!("메시지 복구 완료: {}개 ({})", recovered_messages.len(), mq_type);
        
        Ok(recovered_messages)
    }

    /// 메시지 제거 (성공적으로 재발행된 경우)
    pub async fn remove_message(&self, message_id: &str) -> Result<bool, String> {
        let mut queue = self.memory_queue.lock().await;
        let mut temp_queue = VecDeque::new();
        let mut found = false;
        
        while let Some(message) = queue.pop_front() {
            if message.id == message_id {
                found = true;
                debug!("백업 메시지 제거: {}", message_id);
            } else {
                temp_queue.push_back(message);
            }
        }
        
        *queue = temp_queue;
        
        // 통계 업데이트
        self.update_stats().await;
        
        Ok(found)
    }

    /// 실패한 메시지 재시도 카운트 증가
    pub async fn increment_retry_count(&self, message_id: &str) -> Result<bool, String> {
        let mut queue = self.memory_queue.lock().await;
        let mut temp_queue = VecDeque::new();
        let mut found = false;
        
        while let Some(mut message) = queue.pop_front() {
            if message.id == message_id {
                message.retry_count += 1;
                found = true;
                debug!("재시도 카운트 증가: {} ({}/{})", message_id, message.retry_count, message.max_retries);
            }
            temp_queue.push_back(message);
        }
        
        *queue = temp_queue;
        
        // 통계 업데이트
        self.update_stats().await;
        
        Ok(found)
    }

    /// 디스크에 메시지 쓰기
    async fn write_to_disk(&self, message: &BackupMessage) -> Result<(), String> {
        let file_path = format!("{}.backup", self.backup_file_path);
        let message_clone = message.clone();
        
        tokio::task::spawn_blocking(move || {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .map_err(|e| format!("디스크 쓰기 실패: {}", e))?;
            
            let json_line = serde_json::to_string(&message_clone)
                .map_err(|e| format!("JSON 직렬화 실패: {}", e))?;
            
            writeln!(file, "{}", json_line)
                .map_err(|e| format!("파일 쓰기 실패: {}", e))?;
            
            Ok::<(), String>(())
        }).await.map_err(|e| format!("백업 작업 실패: {}", e))?
    }

    /// 디스크에서 메시지 읽기
    async fn read_from_disk(&self, mq_type: &MQType, limit: usize) -> Result<Vec<BackupMessage>, String> {
        let file_path = format!("{}.backup", self.backup_file_path);
        let mq_type_clone = mq_type.clone();
        
        if !Path::new(&file_path).exists() {
            return Ok(Vec::new());
        }
        
        tokio::task::spawn_blocking(move || {
            let file = File::open(&file_path)
                .map_err(|e| format!("디스크 읽기 실패: {}", e))?;
            
            let reader = BufReader::new(file);
            let mut messages = Vec::new();
            
            for line in reader.lines() {
                let line = line.map_err(|e| format!("라인 읽기 실패: {}", e))?;
                if let Ok(message) = serde_json::from_str::<BackupMessage>(&line) {
                    if message.mq_type == mq_type_clone && messages.len() < limit {
                        messages.push(message);
                    }
                }
            }
            
            Ok::<Vec<BackupMessage>, String>(messages)
        }).await.map_err(|e| format!("복구 작업 실패: {}", e))?
    }

    /// 주기적 백업 스케줄링
    async fn schedule_backup(&self) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let mut last_backup = self.last_backup_time.lock().await;
        
        if current_time - *last_backup >= self.backup_interval_ms {
            if let Err(e) = self.backup_to_disk().await {
                error!("주기적 백업 실패: {}", e);
            } else {
                *last_backup = current_time;
            }
        }
    }

    /// 전체 큐를 디스크에 백업
    async fn backup_to_disk(&self) -> Result<(), String> {
        let queue = self.memory_queue.lock().await;
        
        for message in queue.iter() {
            self.write_to_disk(message).await?;
        }
        
        info!("전체 큐 디스크 백업 완료: {}개 메시지", queue.len());
        
        Ok(())
    }

    /// 통계 업데이트
    async fn update_stats(&self) {
        let queue = self.memory_queue.lock().await;
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let mut stats = self.stats.write().await;
        stats.total_messages = queue.len();
        stats.pending_messages = queue.iter().filter(|m| m.retry_count < m.max_retries).count();
        stats.failed_messages = queue.iter().filter(|m| m.retry_count >= m.max_retries).count();
        
        if let Some(oldest) = queue.front() {
            stats.oldest_message_age = current_time - oldest.created_at;
        }
        
        stats.last_backup_time = current_time;
    }

    /// 큐 통계 조회
    pub async fn get_stats(&self) -> BackupQueueStats {
        self.stats.read().await.clone()
    }

    /// 큐 크기 조회
    pub async fn size(&self) -> usize {
        let queue = self.memory_queue.lock().await;
        queue.len()
    }

    /// 큐 비우기 (긴급 상황용)
    pub async fn clear(&self) -> Result<(), String> {
        let mut queue = self.memory_queue.lock().await;
        queue.clear();
        
        // 통계 업데이트
        self.update_stats().await;
        
        info!("백업 큐 비우기 완료");
        
        Ok(())
    }
}

/// 백업 메시지 생성 헬퍼
pub struct BackupMessageBuilder {
    message: BackupMessage,
}

impl BackupMessageBuilder {
    /// 새 백업 메시지 빌더 생성
    pub fn new(mq_type: MQType, topic_stream: String) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        Self {
            message: BackupMessage {
                id: uuid::Uuid::new_v4().to_string(),
                mq_type,
                topic_stream,
                routing_key: None,
                message_data: serde_json::Value::Null,
                timestamp: current_time,
                retry_count: 0,
                max_retries: 3,
                priority: 5,
                created_at: current_time,
            },
        }
    }

    /// 라우팅 키 설정
    pub fn routing_key(mut self, routing_key: String) -> Self {
        self.message.routing_key = Some(routing_key);
        self
    }

    /// 메시지 데이터 설정
    pub fn message_data(mut self, data: serde_json::Value) -> Self {
        self.message.message_data = data;
        self
    }

    /// 최대 재시도 횟수 설정
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.message.max_retries = max_retries;
        self
    }

    /// 우선순위 설정
    pub fn priority(mut self, priority: u8) -> Self {
        self.message.priority = priority;
        self
    }

    /// 백업 메시지 빌드
    pub fn build(self) -> BackupMessage {
        self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backup_message_creation() {
        let message = BackupMessageBuilder::new(MQType::RedisStreams, "executions".to_string())
            .routing_key("BTC-KRW".to_string())
            .message_data(serde_json::json!({"test": "data"}))
            .max_retries(5)
            .priority(1)
            .build();
        
        assert_eq!(message.mq_type, MQType::RedisStreams);
        assert_eq!(message.topic_stream, "executions");
        assert_eq!(message.routing_key, Some("BTC-KRW".to_string()));
        assert_eq!(message.max_retries, 5);
        assert_eq!(message.priority, 1);
    }

    #[tokio::test]
    async fn test_backup_queue_operations() {
        let queue = LocalBackupQueue::new(
            "/tmp/test_backup".to_string(),
            100,
            1000,
        );
        
        let message = BackupMessageBuilder::new(MQType::Kafka, "market-data".to_string())
            .message_data(serde_json::json!({"symbol": "BTC-KRW"}))
            .build();
        
        // 메시지 백업
        let result = queue.backup_message(message.clone()).await;
        assert!(result.is_ok());
        
        // 큐 크기 확인
        let size = queue.size().await;
        assert_eq!(size, 1);
        
        // 메시지 복구
        let recovered = queue.recover_messages(&MQType::Kafka, 10).await.unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, message.id);
        
        // 메시지 제거
        let removed = queue.remove_message(&message.id).await.unwrap();
        assert!(removed);
        
        // 큐 크기 확인
        let size = queue.size().await;
        assert_eq!(size, 0);
    }

    #[tokio::test]
    async fn test_retry_count_increment() {
        let queue = LocalBackupQueue::new(
            "/tmp/test_backup_retry".to_string(),
            100,
            1000,
        );
        
        let message = BackupMessageBuilder::new(MQType::RabbitMQ, "websocket".to_string())
            .max_retries(3)
            .build();
        
        queue.backup_message(message.clone()).await.unwrap();
        
        // 재시도 카운트 증가
        let incremented = queue.increment_retry_count(&message.id).await.unwrap();
        assert!(incremented);
        
        // 복구하여 확인
        let recovered = queue.recover_messages(&MQType::RabbitMQ, 10).await.unwrap();
        assert_eq!(recovered[0].retry_count, 1);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = LocalBackupQueue::new(
            "/tmp/test_backup_stats".to_string(),
            100,
            1000,
        );
        
        let message1 = BackupMessageBuilder::new(MQType::RedisStreams, "executions".to_string())
            .max_retries(2)
            .build();
        
        let message2 = BackupMessageBuilder::new(MQType::Kafka, "market-data".to_string())
            .max_retries(1)
            .build();
        
        queue.backup_message(message1).await.unwrap();
        queue.backup_message(message2).await.unwrap();
        
        let stats = queue.get_stats().await;
        assert_eq!(stats.total_messages, 2);
        assert_eq!(stats.pending_messages, 2);
        assert_eq!(stats.failed_messages, 0);
    }
}