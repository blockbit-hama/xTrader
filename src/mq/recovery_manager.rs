//! MQ 자동 재발행 및 복구 관리자
//!
//! 이 모듈은 MQ 장애 시 백업된 메시지를 자동으로 재발행하고
//! 지수 백오프를 통한 재시도 메커니즘을 제공합니다.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use crate::mq::backup_queue::{LocalBackupQueue, BackupMessage, MQType, BackupMessageBuilder};
use crate::mq::health_monitor::{MQHealthMonitor, MQHealthStatus};

/// 재발행 상태
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryStatus {
    Idle,
    Recovering,
    Paused,
    Failed,
}

/// 재발행 통계
#[derive(Debug, Clone)]
pub struct RecoveryStats {
    pub total_recovered: u64,
    pub total_failed: u64,
    pub current_batch_size: usize,
    pub last_recovery_time: u64,
    pub status: RecoveryStatus,
}

/// 재발행 설정
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub batch_size: usize,
    pub recovery_interval_ms: u64,
    pub max_retry_delay_ms: u64,
    pub base_retry_delay_ms: u64,
    pub max_batch_size: usize,
    pub min_batch_size: usize,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            recovery_interval_ms: 1000,      // 1초마다 재발행 시도
            max_retry_delay_ms: 30000,      // 최대 30초 지연
            base_retry_delay_ms: 1000,       // 기본 1초 지연
            max_batch_size: 100,
            min_batch_size: 1,
        }
    }
}

/// MQ 복구 관리자
pub struct RecoveryManager {
    /// 백업 큐 참조
    backup_queue: Arc<LocalBackupQueue>,
    /// 헬스 모니터 참조
    health_monitor: Arc<MQHealthMonitor>,
    /// 설정
    config: RecoveryConfig,
    /// 복구 상태
    recovery_status: Arc<RwLock<RecoveryStatus>>,
    /// 통계
    stats: Arc<RwLock<RecoveryStats>>,
    /// 복구 활성화 상태
    is_recovery_active: Arc<Mutex<bool>>,
}

impl RecoveryManager {
    /// 새 복구 관리자 생성
    pub fn new(
        backup_queue: Arc<LocalBackupQueue>,
        health_monitor: Arc<MQHealthMonitor>,
        config: RecoveryConfig,
    ) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            backup_queue,
            health_monitor,
            config,
            recovery_status: Arc::new(RwLock::new(RecoveryStatus::Idle)),
            stats: Arc::new(RwLock::new(RecoveryStats {
                total_recovered: 0,
                total_failed: 0,
                current_batch_size: 0,
                last_recovery_time: current_time,
                status: RecoveryStatus::Idle,
            })),
            is_recovery_active: Arc::new(Mutex::new(false)),
        }
    }

    /// 복구 프로세스 시작
    pub async fn start_recovery(&self) {
        let mut is_active = self.is_recovery_active.lock().await;
        if *is_active {
            warn!("복구 프로세스가 이미 실행 중입니다");
            return;
        }
        *is_active = true;
        drop(is_active);

        info!("MQ 복구 프로세스 시작");

        let backup_queue = self.backup_queue.clone();
        let health_monitor = self.health_monitor.clone();
        let config = self.config.clone();
        let recovery_status = self.recovery_status.clone();
        let stats = self.stats.clone();
        let is_recovery_active = self.is_recovery_active.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.recovery_interval_ms));

            loop {
                interval.tick().await;

                // 복구 프로세스 중단 확인
                {
                    let active = is_recovery_active.lock().await;
                    if !*active {
                        break;
                    }
                }

                // 각 MQ별 복구 수행
                Self::recover_redis_messages(&backup_queue, &health_monitor, &config, &recovery_status, &stats).await;
                Self::recover_kafka_messages(&backup_queue, &health_monitor, &config, &recovery_status, &stats).await;
                Self::recover_rabbitmq_messages(&backup_queue, &health_monitor, &config, &recovery_status, &stats).await;
            }

            info!("MQ 복구 프로세스 종료");
        });
    }

    /// 복구 프로세스 중단
    pub async fn stop_recovery(&self) {
        let mut is_active = self.is_recovery_active.lock().await;
        *is_active = false;
        
        let mut status = self.recovery_status.write().await;
        *status = RecoveryStatus::Idle;
        
        info!("MQ 복구 프로세스 중단 요청");
    }

    /// Redis 메시지 복구
    async fn recover_redis_messages(
        backup_queue: &Arc<LocalBackupQueue>,
        health_monitor: &Arc<MQHealthMonitor>,
        config: &RecoveryConfig,
        recovery_status: &Arc<RwLock<RecoveryStatus>>,
        stats: &Arc<RwLock<RecoveryStats>>,
    ) {
        // Redis가 정상인지 확인
        if !health_monitor.is_mq_healthy(&MQType::RedisStreams).await {
            return;
        }

        // 백업된 메시지 복구
        match backup_queue.recover_messages(&MQType::RedisStreams, config.batch_size).await {
            Ok(messages) => {
                if messages.is_empty() {
                    return;
                }

                debug!("Redis 메시지 복구 시작: {}개", messages.len());

                let message_count = messages.len();
                for message in messages {
                    match Self::republish_redis_message(&message).await {
                        Ok(_) => {
                            // 성공적으로 재발행된 경우 백업에서 제거
                            if let Err(e) = backup_queue.remove_message(&message.id).await {
                                error!("백업 메시지 제거 실패: {}", e);
                            }
                            
                            // 통계 업데이트
                            Self::update_recovery_stats(stats, true, message_count).await;
                        }
                        Err(e) => {
                            error!("Redis 메시지 재발행 실패: {} - {}", message.id, e);
                            
                            // 재시도 카운트 증가
                            if let Err(e) = backup_queue.increment_retry_count(&message.id).await {
                                error!("재시도 카운트 증가 실패: {}", e);
                            }
                            
                            // 통계 업데이트
                            Self::update_recovery_stats(stats, false, message_count).await;
                            
                            // 지수 백오프 적용
                            Self::apply_exponential_backoff(&message, config).await;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Redis 메시지 복구 실패: {}", e);
            }
        }
    }

    /// Kafka 메시지 복구
    async fn recover_kafka_messages(
        backup_queue: &Arc<LocalBackupQueue>,
        health_monitor: &Arc<MQHealthMonitor>,
        config: &RecoveryConfig,
        recovery_status: &Arc<RwLock<RecoveryStatus>>,
        stats: &Arc<RwLock<RecoveryStats>>,
    ) {
        // Kafka가 정상인지 확인
        if !health_monitor.is_mq_healthy(&MQType::Kafka).await {
            return;
        }

        // 백업된 메시지 복구
        match backup_queue.recover_messages(&MQType::Kafka, config.batch_size).await {
            Ok(messages) => {
                if messages.is_empty() {
                    return;
                }

                debug!("Kafka 메시지 복구 시작: {}개", messages.len());

                let message_count = messages.len();
                for message in messages {
                    match Self::republish_kafka_message(&message).await {
                        Ok(_) => {
                            // 성공적으로 재발행된 경우 백업에서 제거
                            if let Err(e) = backup_queue.remove_message(&message.id).await {
                                error!("백업 메시지 제거 실패: {}", e);
                            }
                            
                            // 통계 업데이트
                            Self::update_recovery_stats(stats, true, message_count).await;
                        }
                        Err(e) => {
                            error!("Kafka 메시지 재발행 실패: {} - {}", message.id, e);
                            
                            // 재시도 카운트 증가
                            if let Err(e) = backup_queue.increment_retry_count(&message.id).await {
                                error!("재시도 카운트 증가 실패: {}", e);
                            }
                            
                            // 통계 업데이트
                            Self::update_recovery_stats(stats, false, message_count).await;
                            
                            // 지수 백오프 적용
                            Self::apply_exponential_backoff(&message, config).await;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Kafka 메시지 복구 실패: {}", e);
            }
        }
    }

    /// RabbitMQ 메시지 복구
    async fn recover_rabbitmq_messages(
        backup_queue: &Arc<LocalBackupQueue>,
        health_monitor: &Arc<MQHealthMonitor>,
        config: &RecoveryConfig,
        recovery_status: &Arc<RwLock<RecoveryStatus>>,
        stats: &Arc<RwLock<RecoveryStats>>,
    ) {
        // RabbitMQ가 정상인지 확인
        if !health_monitor.is_mq_healthy(&MQType::RabbitMQ).await {
            return;
        }

        // 백업된 메시지 복구
        match backup_queue.recover_messages(&MQType::RabbitMQ, config.batch_size).await {
            Ok(messages) => {
                if messages.is_empty() {
                    return;
                }

                debug!("RabbitMQ 메시지 복구 시작: {}개", messages.len());

                let message_count = messages.len();
                for message in messages {
                    match Self::republish_rabbitmq_message(&message).await {
                        Ok(_) => {
                            // 성공적으로 재발행된 경우 백업에서 제거
                            if let Err(e) = backup_queue.remove_message(&message.id).await {
                                error!("백업 메시지 제거 실패: {}", e);
                            }
                            
                            // 통계 업데이트
                            Self::update_recovery_stats(stats, true, message_count).await;
                        }
                        Err(e) => {
                            error!("RabbitMQ 메시지 재발행 실패: {} - {}", message.id, e);
                            
                            // 재시도 카운트 증가
                            if let Err(e) = backup_queue.increment_retry_count(&message.id).await {
                                error!("재시도 카운트 증가 실패: {}", e);
                            }
                            
                            // 통계 업데이트
                            Self::update_recovery_stats(stats, false, message_count).await;
                            
                            // 지수 백오프 적용
                            Self::apply_exponential_backoff(&message, config).await;
                        }
                    }
                }
            }
            Err(e) => {
                error!("RabbitMQ 메시지 복구 실패: {}", e);
            }
        }
    }

    /// Redis 메시지 재발행 (Mock)
    async fn republish_redis_message(message: &BackupMessage) -> Result<(), String> {
        // Mock: 실제로는 Redis Streams에 메시지 재발행
        sleep(Duration::from_millis(5)).await;
        
        // 95% 성공률로 시뮬레이션
        if fastrand::f32() < 0.95 {
            debug!("Redis 메시지 재발행 성공: {}", message.id);
            Ok(())
        } else {
            Err("Redis 재발행 실패 (Mock)".to_string())
        }
    }

    /// Kafka 메시지 재발행 (Mock)
    async fn republish_kafka_message(message: &BackupMessage) -> Result<(), String> {
        // Mock: 실제로는 Kafka Topic에 메시지 재발행
        sleep(Duration::from_millis(8)).await;
        
        // 90% 성공률로 시뮬레이션
        if fastrand::f32() < 0.90 {
            debug!("Kafka 메시지 재발행 성공: {}", message.id);
            Ok(())
        } else {
            Err("Kafka 재발행 실패 (Mock)".to_string())
        }
    }

    /// RabbitMQ 메시지 재발행 (Mock)
    async fn republish_rabbitmq_message(message: &BackupMessage) -> Result<(), String> {
        // Mock: 실제로는 RabbitMQ Exchange에 메시지 재발행
        sleep(Duration::from_millis(6)).await;
        
        // 92% 성공률로 시뮬레이션
        if fastrand::f32() < 0.92 {
            debug!("RabbitMQ 메시지 재발행 성공: {}", message.id);
            Ok(())
        } else {
            Err("RabbitMQ 재발행 실패 (Mock)".to_string())
        }
    }

    /// 지수 백오프 적용
    async fn apply_exponential_backoff(message: &BackupMessage, config: &RecoveryConfig) {
        let delay = std::cmp::min(
            config.base_retry_delay_ms * (2_u64.pow(message.retry_count)),
            config.max_retry_delay_ms,
        );
        
        debug!("지수 백오프 적용: {}ms (재시도: {})", delay, message.retry_count);
        sleep(Duration::from_millis(delay)).await;
    }

    /// 복구 통계 업데이트
    async fn update_recovery_stats(
        stats: &Arc<RwLock<RecoveryStats>>,
        success: bool,
        batch_size: usize,
    ) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut stats_guard = stats.write().await;
        
        if success {
            stats_guard.total_recovered += 1;
        } else {
            stats_guard.total_failed += 1;
        }
        
        stats_guard.current_batch_size = batch_size;
        stats_guard.last_recovery_time = current_time;
    }

    /// 복구 상태 조회
    pub async fn get_recovery_status(&self) -> RecoveryStatus {
        self.recovery_status.read().await.clone()
    }

    /// 복구 통계 조회
    pub async fn get_recovery_stats(&self) -> RecoveryStats {
        self.stats.read().await.clone()
    }

    /// 수동 복구 트리거
    pub async fn trigger_manual_recovery(&self, mq_type: &MQType) -> Result<usize, String> {
        info!("수동 복구 트리거: {:?}", mq_type);
        
        // 백업된 메시지 복구
        let messages = self.backup_queue.recover_messages(mq_type, self.config.max_batch_size).await?;
        
        if messages.is_empty() {
            return Ok(0);
        }

        let mut recovered_count = 0;
        
        for message in messages {
            let result = match mq_type {
                MQType::RedisStreams => Self::republish_redis_message(&message).await,
                MQType::Kafka => Self::republish_kafka_message(&message).await,
                MQType::RabbitMQ => Self::republish_rabbitmq_message(&message).await,
            };
            
            match result {
                Ok(_) => {
                    if let Err(e) = self.backup_queue.remove_message(&message.id).await {
                        error!("백업 메시지 제거 실패: {}", e);
                    }
                    recovered_count += 1;
                }
                Err(e) => {
                    error!("수동 복구 실패: {} - {}", message.id, e);
                    if let Err(e) = self.backup_queue.increment_retry_count(&message.id).await {
                        error!("재시도 카운트 증가 실패: {}", e);
                    }
                }
            }
        }
        
        info!("수동 복구 완료: {}개 메시지", recovered_count);
        Ok(recovered_count)
    }

    /// 복구 리포트 생성
    pub async fn generate_recovery_report(&self) -> String {
        let stats = self.get_recovery_stats().await;
        let status = self.get_recovery_status().await;
        let backup_stats = self.backup_queue.get_stats().await;
        
        format!(
            "=== 복구 리포트 ===\n\
            상태: {:?}\n\
            총 복구: {}개\n\
            총 실패: {}개\n\
            현재 배치 크기: {}\n\
            마지막 복구 시간: {}\n\
            백업 큐 크기: {}개\n\
            대기 중: {}개\n\
            실패: {}개\n\
            =================",
            status,
            stats.total_recovered,
            stats.total_failed,
            stats.current_batch_size,
            stats.last_recovery_time,
            backup_stats.total_messages,
            backup_stats.pending_messages,
            backup_stats.failed_messages
        )
    }
}

// fastrand 의존성을 위해 간단한 랜덤 함수 구현
mod fastrand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn f32() -> f32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        (hash % 1000) as f32 / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recovery_manager_creation() {
        let backup_queue = Arc::new(LocalBackupQueue::new(
            "/tmp/test_recovery".to_string(),
            100,
            1000,
        ));
        
        let health_monitor = Arc::new(MQHealthMonitor::new(
            HealthCheckConfig::default(),
            backup_queue.clone(),
        ));
        
        let config = RecoveryConfig::default();
        let manager = RecoveryManager::new(backup_queue, health_monitor, config);
        
        let status = manager.get_recovery_status().await;
        assert_eq!(status, RecoveryStatus::Idle);
        
        let stats = manager.get_recovery_stats().await;
        assert_eq!(stats.total_recovered, 0);
        assert_eq!(stats.total_failed, 0);
    }

    #[tokio::test]
    async fn test_recovery_config() {
        let config = RecoveryConfig {
            batch_size: 20,
            recovery_interval_ms: 2000,
            max_retry_delay_ms: 60000,
            base_retry_delay_ms: 2000,
            max_batch_size: 200,
            min_batch_size: 5,
        };
        
        assert_eq!(config.batch_size, 20);
        assert_eq!(config.recovery_interval_ms, 2000);
        assert_eq!(config.max_retry_delay_ms, 60000);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let config = RecoveryConfig::default();
        let message = BackupMessageBuilder::new(MQType::RedisStreams, "test".to_string())
            .max_retries(3)
            .build();
        
        // 지수 백오프 계산 테스트
        let delay = std::cmp::min(
            config.base_retry_delay_ms * (2_u64.pow(message.retry_count)),
            config.max_retry_delay_ms,
        );
        
        assert_eq!(delay, config.base_retry_delay_ms); // 첫 번째 재시도
    }
}