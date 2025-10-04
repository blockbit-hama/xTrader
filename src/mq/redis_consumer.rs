//! Redis Streams Consumer 구현 (간단 버전)
//!
//! 이 모듈은 체결 내역을 처리하는 기본 Consumer 기능을 제공합니다.

use redis::{RedisResult, AsyncCommands};
use redis::aio::Connection;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};
use crate::mq::redis_streams::{RedisStreamsProducer, ExecutionMessage};

/// Redis Consumer Worker
pub struct RedisConsumerWorker {
    client: Arc<redis::Client>,
    connection: Arc<Mutex<Connection>>,
    stream_name: String,
    consumer_group: String,
    consumer_name: String,
    db_pool: SqlitePool,
    worker_id: String,
    batch_size: usize,
    processing_interval_ms: u64,
}

/// Consumer Worker 설정
pub struct ConsumerConfig {
    pub redis_url: String,
    pub stream_name: String,
    pub consumer_group: String,
    pub worker_id: String,
    pub batch_size: usize,
    pub processing_interval_ms: u64,
}

impl RedisConsumerWorker {
    /// 새 Consumer Worker 생성
    pub async fn new(config: ConsumerConfig, db_pool: SqlitePool) -> RedisResult<Self> {
        let client = redis::Client::open(config.redis_url)?;
        let connection = client.get_async_connection().await?;
        
        info!("Redis Consumer Worker 초기화 완료: {}", config.worker_id);
        
        Ok(Self {
            client: Arc::new(client),
            connection: Arc::new(Mutex::new(connection)),
            stream_name: config.stream_name,
            consumer_group: config.consumer_group,
            consumer_name: config.worker_id.clone(),
            db_pool,
            worker_id: config.worker_id,
            batch_size: config.batch_size,
            processing_interval_ms: config.processing_interval_ms,
        })
    }

    /// Consumer Worker 실행 (메인 루프)
    pub async fn run(&self) -> RedisResult<()> {
        info!("Consumer Worker 시작: {}", self.worker_id);
        
        loop {
            match self.process_batch().await {
                Ok(processed_count) => {
                    if processed_count > 0 {
                        info!("Worker {} 처리 완료: {}개 메시지", self.worker_id, processed_count);
                    }
                }
                Err(e) => {
                    error!("Worker {} 처리 오류: {}", self.worker_id, e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
            }
            
            // 처리 간격 대기
            tokio::time::sleep(tokio::time::Duration::from_millis(self.processing_interval_ms)).await;
        }
    }

    /// 메시지 배치 처리 (간단 버전)
    async fn process_batch(&self) -> RedisResult<usize> {
        // 간단한 구현: 실제로는 Consumer Group 기반으로 구현해야 함
        info!("Worker {} 배치 처리 중...", self.worker_id);
        
        // TODO: 실제 Consumer Group 기반 메시지 읽기 구현
        // 현재는 기본 구조만 제공
        
        Ok(0)
    }
}

/// Consumer Manager (여러 Worker 관리)
pub struct RedisConsumerManager {
    workers: Vec<Arc<RedisConsumerWorker>>,
    db_pool: SqlitePool,
}

impl RedisConsumerManager {
    /// 새 Consumer Manager 생성
    pub async fn new(
        configs: Vec<ConsumerConfig>,
        db_pool: SqlitePool,
    ) -> RedisResult<Self> {
        let mut workers = Vec::new();
        
        for config in configs {
            let worker = Arc::new(RedisConsumerWorker::new(config, db_pool.clone()).await?);
            workers.push(worker);
        }
        
        info!("Consumer Manager 초기화 완료: {}개 Worker", workers.len());
        
        Ok(Self {
            workers,
            db_pool,
        })
    }

    /// 모든 Worker 시작
    pub async fn start_all_workers(&self) -> RedisResult<()> {
        let mut handles = Vec::new();
        
        for worker in &self.workers {
            let worker_clone = worker.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = worker_clone.run().await {
                    error!("Worker {} 실행 오류: {}", worker_clone.worker_id, e);
                }
            });
            handles.push(handle);
        }
        
        info!("모든 Consumer Worker 시작 완료: {}개", self.workers.len());
        
        // 모든 Worker가 종료될 때까지 대기
        for handle in handles {
            let _ = handle.await;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consumer_config() {
        let config = ConsumerConfig {
            redis_url: "redis://localhost:6379".to_string(),
            stream_name: "test-executions".to_string(),
            consumer_group: "test-group".to_string(),
            worker_id: "test-worker-1".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000,
        };
        
        assert_eq!(config.worker_id, "test-worker-1");
        assert_eq!(config.batch_size, 100);
    }
}