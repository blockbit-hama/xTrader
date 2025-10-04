//! MQ 연결 상태 모니터링 시스템
//!
//! 이 모듈은 Redis, Kafka, RabbitMQ의 연결 상태를 모니터링하고
//! 장애를 감지하여 자동 복구를 수행하는 기능을 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use crate::mq::{LocalBackupQueue, MQType, BackupMessageBuilder};

/// MQ 연결 상태
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    Failed,
}

/// MQ 헬스 상태
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: ConnectionStatus,
    pub last_check_time: u64,
    pub consecutive_failures: u32,
    pub total_checks: u32,
    pub success_rate: f64,
    pub last_error: Option<String>,
    pub response_time_ms: u64,
}

/// MQ 타입별 헬스 상태
#[derive(Debug, Clone)]
pub struct MQHealthStatus {
    pub redis: HealthStatus,
    pub kafka: HealthStatus,
    pub rabbitmq: HealthStatus,
}

/// 헬스체크 설정
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub check_interval_ms: u64,
    pub timeout_ms: u64,
    pub max_consecutive_failures: u32,
    pub reconnect_interval_ms: u64,
    pub max_reconnect_attempts: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 5000,      // 5초마다 체크
            timeout_ms: 3000,             // 3초 타임아웃
            max_consecutive_failures: 3,  // 3회 연속 실패 시 장애로 판단
            reconnect_interval_ms: 10000, // 10초마다 재연결 시도
            max_reconnect_attempts: 5,    // 최대 5회 재연결 시도
        }
    }
}

/// MQ 헬스 모니터
pub struct MQHealthMonitor {
    /// 헬스 상태 저장소
    health_status: Arc<RwLock<MQHealthStatus>>,
    /// 설정
    config: HealthCheckConfig,
    /// 모니터링 활성화 상태
    is_monitoring: Arc<Mutex<bool>>,
    /// 백업 큐 참조
    backup_queue: Arc<LocalBackupQueue>,
}

impl MQHealthMonitor {
    /// 새 헬스 모니터 생성
    pub fn new(config: HealthCheckConfig, backup_queue: Arc<LocalBackupQueue>) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let initial_status = HealthStatus {
            status: ConnectionStatus::Disconnected,
            last_check_time: current_time,
            consecutive_failures: 0,
            total_checks: 0,
            success_rate: 0.0,
            last_error: None,
            response_time_ms: 0,
        };

        Self {
            health_status: Arc::new(RwLock::new(MQHealthStatus {
                redis: initial_status.clone(),
                kafka: initial_status.clone(),
                rabbitmq: initial_status.clone(),
            })),
            config,
            is_monitoring: Arc::new(Mutex::new(false)),
            backup_queue,
        }
    }

    /// 헬스 모니터링 시작
    pub async fn start_monitoring(&self) {
        let mut is_monitoring = self.is_monitoring.lock().await;
        if *is_monitoring {
            warn!("헬스 모니터링이 이미 실행 중입니다");
            return;
        }
        *is_monitoring = true;
        drop(is_monitoring);

        info!("MQ 헬스 모니터링 시작");

        let health_status = self.health_status.clone();
        let config = self.config.clone();
        let backup_queue = self.backup_queue.clone();
        let is_monitoring = self.is_monitoring.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.check_interval_ms));

            loop {
                interval.tick().await;

                // 모니터링 중단 확인
                {
                    let monitoring = is_monitoring.lock().await;
                    if !*monitoring {
                        break;
                    }
                }

                // 각 MQ별 헬스체크 수행
                Self::check_redis_health(&health_status, &config, &backup_queue).await;
                Self::check_kafka_health(&health_status, &config, &backup_queue).await;
                Self::check_rabbitmq_health(&health_status, &config, &backup_queue).await;
            }

            info!("MQ 헬스 모니터링 종료");
        });
    }

    /// 헬스 모니터링 중단
    pub async fn stop_monitoring(&self) {
        let mut is_monitoring = self.is_monitoring.lock().await;
        *is_monitoring = false;
        info!("MQ 헬스 모니터링 중단 요청");
    }

    /// Redis 헬스체크
    async fn check_redis_health(
        health_status: &Arc<RwLock<MQHealthStatus>>,
        config: &HealthCheckConfig,
        backup_queue: &Arc<LocalBackupQueue>,
    ) {
        let start_time = SystemTime::now();
        let mut is_healthy = false;
        let mut error_message = None;

        // Redis 연결 테스트 (Mock)
        match Self::test_redis_connection().await {
            Ok(_) => {
                is_healthy = true;
                debug!("Redis 헬스체크 성공");
            }
            Err(e) => {
                error_message = Some(e);
                warn!("Redis 헬스체크 실패: {}", error_message.as_ref().unwrap());
            }
        }

        let response_time = start_time.elapsed().unwrap().as_millis() as u64;
        Self::update_health_status(
            health_status,
            |mq_status| &mut mq_status.redis,
            is_healthy,
            response_time,
            error_message,
            config,
            backup_queue,
            MQType::RedisStreams,
        ).await;
    }

    /// Kafka 헬스체크
    async fn check_kafka_health(
        health_status: &Arc<RwLock<MQHealthStatus>>,
        config: &HealthCheckConfig,
        backup_queue: &Arc<LocalBackupQueue>,
    ) {
        let start_time = SystemTime::now();
        let mut is_healthy = false;
        let mut error_message = None;

        // Kafka 연결 테스트 (Mock)
        match Self::test_kafka_connection().await {
            Ok(_) => {
                is_healthy = true;
                debug!("Kafka 헬스체크 성공");
            }
            Err(e) => {
                error_message = Some(e);
                warn!("Kafka 헬스체크 실패: {}", error_message.as_ref().unwrap());
            }
        }

        let response_time = start_time.elapsed().unwrap().as_millis() as u64;
        Self::update_health_status(
            health_status,
            |mq_status| &mut mq_status.kafka,
            is_healthy,
            response_time,
            error_message,
            config,
            backup_queue,
            MQType::Kafka,
        ).await;
    }

    /// RabbitMQ 헬스체크
    async fn check_rabbitmq_health(
        health_status: &Arc<RwLock<MQHealthStatus>>,
        config: &HealthCheckConfig,
        backup_queue: &Arc<LocalBackupQueue>,
    ) {
        let start_time = SystemTime::now();
        let mut is_healthy = false;
        let mut error_message = None;

        // RabbitMQ 연결 테스트 (Mock)
        match Self::test_rabbitmq_connection().await {
            Ok(_) => {
                is_healthy = true;
                debug!("RabbitMQ 헬스체크 성공");
            }
            Err(e) => {
                error_message = Some(e);
                warn!("RabbitMQ 헬스체크 실패: {}", error_message.as_ref().unwrap());
            }
        }

        let response_time = start_time.elapsed().unwrap().as_millis() as u64;
        Self::update_health_status(
            health_status,
            |mq_status| &mut mq_status.rabbitmq,
            is_healthy,
            response_time,
            error_message,
            config,
            backup_queue,
            MQType::RabbitMQ,
        ).await;
    }

    /// 헬스 상태 업데이트
    async fn update_health_status<F>(
        health_status: &Arc<RwLock<MQHealthStatus>>,
        get_status: F,
        is_healthy: bool,
        response_time: u64,
        error_message: Option<String>,
        config: &HealthCheckConfig,
        backup_queue: &Arc<LocalBackupQueue>,
        mq_type: MQType,
    ) where
        F: FnOnce(&mut MQHealthStatus) -> &mut HealthStatus,
    {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut mq_status = health_status.write().await;
        let status = get_status(&mut *mq_status);

        status.last_check_time = current_time;
        status.total_checks += 1;
        status.response_time_ms = response_time;

        if is_healthy {
            status.consecutive_failures = 0;
            status.status = ConnectionStatus::Connected;
            status.last_error = None;
        } else {
            status.consecutive_failures += 1;
            status.last_error = error_message;

            if status.consecutive_failures >= config.max_consecutive_failures {
                status.status = ConnectionStatus::Failed;
                warn!("MQ 장애 감지: {:?} (연속 실패: {})", mq_type, status.consecutive_failures);
                
                // 장애 시 백업 큐 활성화 알림
                Self::notify_failure_to_backup_queue(backup_queue, mq_type, status.last_error.clone()).await;
            } else {
                status.status = ConnectionStatus::Disconnected;
            }
        }

        // 성공률 계산
        let success_count = status.total_checks - status.consecutive_failures;
        status.success_rate = if status.total_checks > 0 {
            success_count as f64 / status.total_checks as f64
        } else {
            0.0
        };
    }

    /// 백업 큐에 장애 알림
    async fn notify_failure_to_backup_queue(
        backup_queue: &Arc<LocalBackupQueue>,
        mq_type: MQType,
        error_message: Option<String>,
    ) {
        let failure_message = BackupMessageBuilder::new(mq_type.clone(), "health_monitor".to_string())
            .message_data(serde_json::json!({
                "type": "mq_failure",
                "error": error_message,
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
            }))
            .priority(0) // 최고 우선순위
            .max_retries(1)
            .build();

        if let Err(e) = backup_queue.backup_message(failure_message).await {
            error!("백업 큐에 장애 알림 실패: {}", e);
        }
    }

    /// Redis 연결 테스트 (Mock)
    async fn test_redis_connection() -> Result<(), String> {
        // Mock: 실제로는 Redis PING 명령 실행
        sleep(Duration::from_millis(10)).await; // 네트워크 지연 시뮬레이션
        
        // 90% 성공률로 시뮬레이션
        if fastrand::f32() < 0.9 {
            Ok(())
        } else {
            Err("Redis 연결 실패 (Mock)".to_string())
        }
    }

    /// Kafka 연결 테스트 (Mock)
    async fn test_kafka_connection() -> Result<(), String> {
        // Mock: 실제로는 Kafka 메타데이터 요청
        sleep(Duration::from_millis(20)).await; // 네트워크 지연 시뮬레이션
        
        // 85% 성공률로 시뮬레이션
        if fastrand::f32() < 0.85 {
            Ok(())
        } else {
            Err("Kafka 연결 실패 (Mock)".to_string())
        }
    }

    /// RabbitMQ 연결 테스트 (Mock)
    async fn test_rabbitmq_connection() -> Result<(), String> {
        // Mock: 실제로는 RabbitMQ 연결 확인
        sleep(Duration::from_millis(15)).await; // 네트워크 지연 시뮬레이션
        
        // 80% 성공률로 시뮬레이션
        if fastrand::f32() < 0.8 {
            Ok(())
        } else {
            Err("RabbitMQ 연결 실패 (Mock)".to_string())
        }
    }

    /// 전체 헬스 상태 조회
    pub async fn get_health_status(&self) -> MQHealthStatus {
        self.health_status.read().await.clone()
    }

    /// 특정 MQ 상태 조회
    pub async fn get_mq_status(&self, mq_type: &MQType) -> Option<HealthStatus> {
        let status = self.health_status.read().await;
        match mq_type {
            MQType::RedisStreams => Some(status.redis.clone()),
            MQType::Kafka => Some(status.kafka.clone()),
            MQType::RabbitMQ => Some(status.rabbitmq.clone()),
        }
    }

    /// 모든 MQ가 정상인지 확인
    pub async fn is_all_healthy(&self) -> bool {
        let status = self.health_status.read().await;
        status.redis.status == ConnectionStatus::Connected &&
        status.kafka.status == ConnectionStatus::Connected &&
        status.rabbitmq.status == ConnectionStatus::Connected
    }

    /// 특정 MQ가 정상인지 확인
    pub async fn is_mq_healthy(&self, mq_type: &MQType) -> bool {
        if let Some(status) = self.get_mq_status(mq_type).await {
            status.status == ConnectionStatus::Connected
        } else {
            false
        }
    }

    /// 헬스 리포트 생성
    pub async fn generate_health_report(&self) -> String {
        let status = self.get_health_status().await;
        
        format!(
            "=== MQ 헬스 리포트 ===\n\
            Redis: {:?} (성공률: {:.1}%, 응답시간: {}ms)\n\
            Kafka: {:?} (성공률: {:.1}%, 응답시간: {}ms)\n\
            RabbitMQ: {:?} (성공률: {:.1}%, 응답시간: {}ms)\n\
            전체 상태: {}\n\
            ====================",
            status.redis.status,
            status.redis.success_rate * 100.0,
            status.redis.response_time_ms,
            status.kafka.status,
            status.kafka.success_rate * 100.0,
            status.kafka.response_time_ms,
            status.rabbitmq.status,
            status.rabbitmq.success_rate * 100.0,
            status.rabbitmq.response_time_ms,
            if self.is_all_healthy().await { "정상" } else { "장애" }
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
    async fn test_health_monitor_creation() {
        let config = HealthCheckConfig::default();
        let backup_queue = Arc::new(LocalBackupQueue::new(
            "/tmp/test_health".to_string(),
            100,
            1000,
        ));
        
        let monitor = MQHealthMonitor::new(config, backup_queue);
        
        let status = monitor.get_health_status().await;
        assert_eq!(status.redis.status, ConnectionStatus::Disconnected);
        assert_eq!(status.kafka.status, ConnectionStatus::Disconnected);
        assert_eq!(status.rabbitmq.status, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_health_status_update() {
        let config = HealthCheckConfig::default();
        let backup_queue = Arc::new(LocalBackupQueue::new(
            "/tmp/test_health_update".to_string(),
            100,
            1000,
        ));
        
        let monitor = MQHealthMonitor::new(config, backup_queue);
        
        // 초기 상태 확인
        assert!(!monitor.is_all_healthy().await);
        assert!(!monitor.is_mq_healthy(&MQType::RedisStreams).await);
        
        // 헬스 리포트 생성
        let report = monitor.generate_health_report().await;
        assert!(report.contains("Redis:"));
        assert!(report.contains("Kafka:"));
        assert!(report.contains("RabbitMQ:"));
    }

    #[tokio::test]
    async fn test_health_check_config() {
        let config = HealthCheckConfig {
            check_interval_ms: 1000,
            timeout_ms: 500,
            max_consecutive_failures: 2,
            reconnect_interval_ms: 5000,
            max_reconnect_attempts: 3,
        };
        
        assert_eq!(config.check_interval_ms, 1000);
        assert_eq!(config.max_consecutive_failures, 2);
    }
}