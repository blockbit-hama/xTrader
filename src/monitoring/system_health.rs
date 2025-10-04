//! 시스템 헬스체크 모니터링
//!
//! 이 모듈은 전체 시스템의 상태를 모니터링하고
//! 장애를 조기 감지하여 알림을 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, AtomicBool, Ordering};

/// 헬스체크 상태
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,    // 정상
    Warning,    // 경고
    Critical,   // 위험
    Unknown,    // 알 수 없음
}

/// 서비스 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServiceType {
    Database,
    Redis,
    Kafka,
    RabbitMQ,
    MatchingEngine,
    OrderSequencer,
    WebSocketServer,
    RestApi,
    MDPConsumer,
    ExternalSync,
    RegulatoryReporting,
    AnalyticsIntegration,
    BatchProcessor,
    WorkerPool,
    CacheOptimizer,
    MetricsCollector,
}

/// 서비스 헬스체크 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub service_type: ServiceType,
    pub status: HealthStatus,
    pub message: String,
    pub response_time_ms: u64,
    pub last_check: u64,
    pub consecutive_failures: u32,
    pub uptime_percent: f64,
    pub error_rate: f64,
}

/// 시스템 헬스체크 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub services: HashMap<String, ServiceHealth>,
    pub total_services: usize,
    pub healthy_services: usize,
    pub warning_services: usize,
    pub critical_services: usize,
    pub unknown_services: usize,
    pub system_uptime_seconds: u64,
    pub last_check: u64,
    pub recommendations: Vec<String>,
}

/// 헬스체크 설정
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub check_interval_ms: u64,
    pub timeout_ms: u64,
    pub max_consecutive_failures: u32,
    pub warning_threshold_ms: u64,
    pub critical_threshold_ms: u64,
    pub enable_notifications: bool,
    pub enable_auto_recovery: bool,
    pub services_to_check: Vec<ServiceType>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 5000, // 5초
            timeout_ms: 3000,        // 3초
            max_consecutive_failures: 3,
            warning_threshold_ms: 1000,  // 1초
            critical_threshold_ms: 3000, // 3초
            enable_notifications: true,
            enable_auto_recovery: true,
            services_to_check: vec![
                ServiceType::Database,
                ServiceType::Redis,
                ServiceType::Kafka,
                ServiceType::RabbitMQ,
                ServiceType::MatchingEngine,
                ServiceType::OrderSequencer,
                ServiceType::WebSocketServer,
                ServiceType::RestApi,
                ServiceType::MDPConsumer,
                ServiceType::ExternalSync,
                ServiceType::RegulatoryReporting,
                ServiceType::AnalyticsIntegration,
                ServiceType::BatchProcessor,
                ServiceType::WorkerPool,
                ServiceType::CacheOptimizer,
                ServiceType::MetricsCollector,
            ],
        }
    }
}

/// 헬스체크 모니터
pub struct SystemHealthMonitor {
    config: HealthCheckConfig,
    service_healths: Arc<RwLock<HashMap<String, ServiceHealth>>>,
    system_start_time: SystemTime,
    is_running: Arc<Mutex<bool>>,
    notification_sender: Arc<dyn Fn(String, String) -> Result<(), String> + Send + Sync>,
}

impl SystemHealthMonitor {
    /// 새 헬스체크 모니터 생성
    pub fn new<F>(config: HealthCheckConfig, notification_sender: F) -> Self
    where
        F: Fn(String, String) -> Result<(), String> + Send + Sync + 'static,
    {
        Self {
            config,
            service_healths: Arc::new(RwLock::new(HashMap::new())),
            system_start_time: SystemTime::now(),
            is_running: Arc::new(Mutex::new(false)),
            notification_sender: Arc::new(notification_sender),
        }
    }

    /// 헬스체크 모니터 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("헬스체크 모니터가 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("헬스체크 모니터 시작: 체크간격={}ms", self.config.check_interval_ms);

        let service_healths = self.service_healths.clone();
        let config = self.config.clone();
        let notification_sender = self.notification_sender.clone();
        let is_running = self.is_running.clone();

        // 헬스체크 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.check_interval_ms));

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 서비스별 헬스체크 실행
                Self::check_all_services(&service_healths, &config, &notification_sender).await;
            }

            info!("헬스체크 모니터 종료");
        });
    }

    /// 헬스체크 모니터 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("헬스체크 모니터 중단 요청");
    }

    /// 모든 서비스 헬스체크 실행
    async fn check_all_services(
        service_healths: &Arc<RwLock<HashMap<String, ServiceHealth>>>,
        config: &HealthCheckConfig,
        notification_sender: &Arc<dyn Fn(String, String) -> Result<(), String> + Send + Sync>,
    ) {
        let mut healths = service_healths.write().await;
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for service_type in &config.services_to_check {
            let service_name = Self::service_type_to_string(service_type);
            let health_result = Self::check_service_health(service_type, config).await;
            
            // 이전 상태와 비교하여 알림 발송
            if let Some(previous_health) = healths.get(&service_name) {
                if previous_health.status != health_result.status {
                    let notification = format!(
                        "서비스 상태 변경: {} - {} → {}",
                        service_name,
                        Self::status_to_string(&previous_health.status),
                        Self::status_to_string(&health_result.status)
                    );
                    
                    if config.enable_notifications {
                        if let Err(e) = notification_sender(notification.clone(), "health_check".to_string()) {
                            error!("알림 발송 실패: {}", e);
                        }
                    }
                }
            }
            
            healths.insert(service_name, health_result);
        }
    }

    /// 개별 서비스 헬스체크
    async fn check_service_health(service_type: &ServiceType, config: &HealthCheckConfig) -> ServiceHealth {
        let start_time = SystemTime::now();
        let current_time = start_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (status, message) = match service_type {
            ServiceType::Database => Self::check_database().await,
            ServiceType::Redis => Self::check_redis().await,
            ServiceType::Kafka => Self::check_kafka().await,
            ServiceType::RabbitMQ => Self::check_rabbitmq().await,
            ServiceType::MatchingEngine => Self::check_matching_engine().await,
            ServiceType::OrderSequencer => Self::check_order_sequencer().await,
            ServiceType::WebSocketServer => Self::check_websocket_server().await,
            ServiceType::RestApi => Self::check_rest_api().await,
            ServiceType::MDPConsumer => Self::check_mdp_consumer().await,
            ServiceType::ExternalSync => Self::check_external_sync().await,
            ServiceType::RegulatoryReporting => Self::check_regulatory_reporting().await,
            ServiceType::AnalyticsIntegration => Self::check_analytics_integration().await,
            ServiceType::BatchProcessor => Self::check_batch_processor().await,
            ServiceType::WorkerPool => Self::check_worker_pool().await,
            ServiceType::CacheOptimizer => Self::check_cache_optimizer().await,
            ServiceType::MetricsCollector => Self::check_metrics_collector().await,
        };

        let response_time = start_time.elapsed().unwrap().as_millis() as u64;

        ServiceHealth {
            service_type: service_type.clone(),
            status: status.clone(),
            message,
            response_time_ms: response_time,
            last_check: current_time,
            consecutive_failures: if status == HealthStatus::Healthy { 0 } else { 1 },
            uptime_percent: Self::calculate_uptime_percent(service_type),
            error_rate: Self::calculate_error_rate(service_type),
        }
    }

    /// 데이터베이스 헬스체크
    async fn check_database() -> (HealthStatus, String) {
        // Mock: 실제로는 데이터베이스 연결 및 쿼리 테스트
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        if fastrand::f32() > 0.1 {
            (HealthStatus::Healthy, "데이터베이스 연결 정상".to_string())
        } else {
            (HealthStatus::Critical, "데이터베이스 연결 실패".to_string())
        }
    }

    /// Redis 헬스체크
    async fn check_redis() -> (HealthStatus, String) {
        // Mock: 실제로는 Redis PING 명령 실행
        tokio::time::sleep(Duration::from_millis(30)).await;
        
        if fastrand::f32() > 0.05 {
            (HealthStatus::Healthy, "Redis 연결 정상".to_string())
        } else {
            (HealthStatus::Warning, "Redis 응답 지연".to_string())
        }
    }

    /// Kafka 헬스체크
    async fn check_kafka() -> (HealthStatus, String) {
        // Mock: 실제로는 Kafka 브로커 연결 테스트
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        if fastrand::f32() > 0.08 {
            (HealthStatus::Healthy, "Kafka 브로커 연결 정상".to_string())
        } else {
            (HealthStatus::Critical, "Kafka 브로커 연결 실패".to_string())
        }
    }

    /// RabbitMQ 헬스체크
    async fn check_rabbitmq() -> (HealthStatus, String) {
        // Mock: 실제로는 RabbitMQ 연결 테스트
        tokio::time::sleep(Duration::from_millis(80)).await;
        
        if fastrand::f32() > 0.06 {
            (HealthStatus::Healthy, "RabbitMQ 연결 정상".to_string())
        } else {
            (HealthStatus::Warning, "RabbitMQ 큐 상태 이상".to_string())
        }
    }

    /// 매칭 엔진 헬스체크
    async fn check_matching_engine() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        if fastrand::f32() > 0.02 {
            (HealthStatus::Healthy, "매칭 엔진 정상 동작".to_string())
        } else {
            (HealthStatus::Critical, "매칭 엔진 처리 지연".to_string())
        }
    }

    /// 주문 시퀀서 헬스체크
    async fn check_order_sequencer() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(25)).await;
        
        if fastrand::f32() > 0.03 {
            (HealthStatus::Healthy, "주문 시퀀서 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "주문 시퀀서 큐 포화".to_string())
        }
    }

    /// WebSocket 서버 헬스체크
    async fn check_websocket_server() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(15)).await;
        
        if fastrand::f32() > 0.04 {
            (HealthStatus::Healthy, "WebSocket 서버 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "WebSocket 연결 수 과다".to_string())
        }
    }

    /// REST API 헬스체크
    async fn check_rest_api() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(40)).await;
        
        if fastrand::f32() > 0.05 {
            (HealthStatus::Healthy, "REST API 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "REST API 응답 지연".to_string())
        }
    }

    /// MDP Consumer 헬스체크
    async fn check_mdp_consumer() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(35)).await;
        
        if fastrand::f32() > 0.07 {
            (HealthStatus::Healthy, "MDP Consumer 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "MDP Consumer 처리 지연".to_string())
        }
    }

    /// 외부 동기화 헬스체크
    async fn check_external_sync() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(60)).await;
        
        if fastrand::f32() > 0.1 {
            (HealthStatus::Healthy, "외부 동기화 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "외부 API 응답 지연".to_string())
        }
    }

    /// 규제 보고 헬스체크
    async fn check_regulatory_reporting() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(45)).await;
        
        if fastrand::f32() > 0.08 {
            (HealthStatus::Healthy, "규제 보고 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "규제 보고 처리 지연".to_string())
        }
    }

    /// 분석 시스템 통합 헬스체크
    async fn check_analytics_integration() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(70)).await;
        
        if fastrand::f32() > 0.09 {
            (HealthStatus::Healthy, "분석 시스템 통합 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "분석 시스템 응답 지연".to_string())
        }
    }

    /// 배치 처리기 헬스체크
    async fn check_batch_processor() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(30)).await;
        
        if fastrand::f32() > 0.05 {
            (HealthStatus::Healthy, "배치 처리기 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "배치 처리 큐 포화".to_string())
        }
    }

    /// 워커 풀 헬스체크
    async fn check_worker_pool() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(25)).await;
        
        if fastrand::f32() > 0.04 {
            (HealthStatus::Healthy, "워커 풀 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "워커 풀 과부하".to_string())
        }
    }

    /// 캐시 최적화기 헬스체크
    async fn check_cache_optimizer() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        if fastrand::f32() > 0.03 {
            (HealthStatus::Healthy, "캐시 최적화기 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "캐시 메모리 부족".to_string())
        }
    }

    /// 메트릭 수집기 헬스체크
    async fn check_metrics_collector() -> (HealthStatus, String) {
        tokio::time::sleep(Duration::from_millis(15)).await;
        
        if fastrand::f32() > 0.02 {
            (HealthStatus::Healthy, "메트릭 수집기 정상 동작".to_string())
        } else {
            (HealthStatus::Warning, "메트릭 수집 지연".to_string())
        }
    }

    /// 서비스 타입을 문자열로 변환
    fn service_type_to_string(service_type: &ServiceType) -> String {
        match service_type {
            ServiceType::Database => "Database".to_string(),
            ServiceType::Redis => "Redis".to_string(),
            ServiceType::Kafka => "Kafka".to_string(),
            ServiceType::RabbitMQ => "RabbitMQ".to_string(),
            ServiceType::MatchingEngine => "MatchingEngine".to_string(),
            ServiceType::OrderSequencer => "OrderSequencer".to_string(),
            ServiceType::WebSocketServer => "WebSocketServer".to_string(),
            ServiceType::RestApi => "RestApi".to_string(),
            ServiceType::MDPConsumer => "MDPConsumer".to_string(),
            ServiceType::ExternalSync => "ExternalSync".to_string(),
            ServiceType::RegulatoryReporting => "RegulatoryReporting".to_string(),
            ServiceType::AnalyticsIntegration => "AnalyticsIntegration".to_string(),
            ServiceType::BatchProcessor => "BatchProcessor".to_string(),
            ServiceType::WorkerPool => "WorkerPool".to_string(),
            ServiceType::CacheOptimizer => "CacheOptimizer".to_string(),
            ServiceType::MetricsCollector => "MetricsCollector".to_string(),
        }
    }

    /// 상태를 문자열로 변환
    fn status_to_string(status: &HealthStatus) -> String {
        match status {
            HealthStatus::Healthy => "Healthy".to_string(),
            HealthStatus::Warning => "Warning".to_string(),
            HealthStatus::Critical => "Critical".to_string(),
            HealthStatus::Unknown => "Unknown".to_string(),
        }
    }

    /// 가동률 계산 (Mock)
    fn calculate_uptime_percent(service_type: &ServiceType) -> f64 {
        // Mock: 실제로는 서비스별 가동률 계산
        match service_type {
            ServiceType::Database => 99.9,
            ServiceType::Redis => 99.8,
            ServiceType::Kafka => 99.7,
            ServiceType::RabbitMQ => 99.6,
            ServiceType::MatchingEngine => 99.95,
            ServiceType::OrderSequencer => 99.9,
            ServiceType::WebSocketServer => 99.8,
            ServiceType::RestApi => 99.7,
            ServiceType::MDPConsumer => 99.6,
            ServiceType::ExternalSync => 99.5,
            ServiceType::RegulatoryReporting => 99.4,
            ServiceType::AnalyticsIntegration => 99.3,
            ServiceType::BatchProcessor => 99.9,
            ServiceType::WorkerPool => 99.8,
            ServiceType::CacheOptimizer => 99.7,
            ServiceType::MetricsCollector => 99.9,
        }
    }

    /// 에러율 계산 (Mock)
    fn calculate_error_rate(service_type: &ServiceType) -> f64 {
        // Mock: 실제로는 서비스별 에러율 계산
        fastrand::f32() * 0.1 // 0-0.1%
    }

    /// 전체 시스템 헬스체크 결과 조회
    pub async fn get_system_health(&self) -> SystemHealth {
        let service_healths = self.service_healths.read().await;
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let system_uptime = SystemTime::now()
            .duration_since(self.system_start_time)
            .unwrap()
            .as_secs();

        let mut healthy_count = 0;
        let mut warning_count = 0;
        let mut critical_count = 0;
        let mut unknown_count = 0;

        for health in service_healths.values() {
            match health.status {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Warning => warning_count += 1,
                HealthStatus::Critical => critical_count += 1,
                HealthStatus::Unknown => unknown_count += 1,
            }
        }

        let overall_status = if critical_count > 0 {
            HealthStatus::Critical
        } else if warning_count > 0 {
            HealthStatus::Warning
        } else if healthy_count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        };

        let recommendations = Self::generate_recommendations(&service_healths);

        SystemHealth {
            overall_status,
            services: service_healths.clone(),
            total_services: service_healths.len(),
            healthy_services: healthy_count,
            warning_services: warning_count,
            critical_services: critical_count,
            unknown_services: unknown_count,
            system_uptime_seconds: system_uptime,
            last_check: current_time,
            recommendations,
        }
    }

    /// 추천사항 생성
    fn generate_recommendations(service_healths: &HashMap<String, ServiceHealth>) -> Vec<String> {
        let mut recommendations = Vec::new();

        for health in service_healths.values() {
            match health.status {
                HealthStatus::Critical => {
                    recommendations.push(format!("{} 서비스가 위험 상태입니다. 즉시 확인이 필요합니다.", 
                        Self::service_type_to_string(&health.service_type)));
                }
                HealthStatus::Warning => {
                    recommendations.push(format!("{} 서비스가 경고 상태입니다. 모니터링을 강화하세요.", 
                        Self::service_type_to_string(&health.service_type)));
                }
                _ => {}
            }

            if health.response_time_ms > 1000 {
                recommendations.push(format!("{} 서비스의 응답시간이 느립니다. 성능 최적화를 고려하세요.", 
                    Self::service_type_to_string(&health.service_type)));
            }

            if health.error_rate > 1.0 {
                recommendations.push(format!("{} 서비스의 에러율이 높습니다. 로그를 확인하세요.", 
                    Self::service_type_to_string(&health.service_type)));
            }
        }

        if recommendations.is_empty() {
            recommendations.push("모든 서비스가 정상 동작 중입니다.".to_string());
        }

        recommendations
    }

    /// 특정 서비스 헬스체크 결과 조회
    pub async fn get_service_health(&self, service_name: &str) -> Option<ServiceHealth> {
        let service_healths = self.service_healths.read().await;
        service_healths.get(service_name).cloned()
    }

    /// 서비스 상태 변경 알림 발송
    pub async fn send_status_change_notification(&self, service_name: &str, old_status: &HealthStatus, new_status: &HealthStatus) {
        let notification = format!(
            "서비스 상태 변경 알림\n서비스: {}\n이전 상태: {}\n현재 상태: {}\n시간: {}",
            service_name,
            Self::status_to_string(old_status),
            Self::status_to_string(new_status),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        if let Err(e) = (self.notification_sender)(notification, "status_change".to_string()) {
            error!("상태 변경 알림 발송 실패: {}", e);
        }
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
        let monitor = SystemHealthMonitor::new(config, |msg, _| {
            println!("Notification: {}", msg);
            Ok(())
        });
        
        let system_health = monitor.get_system_health().await;
        assert_eq!(system_health.total_services, 0);
        assert_eq!(system_health.healthy_services, 0);
    }

    #[tokio::test]
    async fn test_service_health_check() {
        let config = HealthCheckConfig::default();
        let monitor = SystemHealthMonitor::new(config, |msg, _| {
            println!("Notification: {}", msg);
            Ok(())
        });
        
        monitor.start().await;
        
        // 잠시 대기
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        let system_health = monitor.get_system_health().await;
        assert!(system_health.total_services > 0);
        
        monitor.stop().await;
    }

    #[tokio::test]
    async fn test_health_status_conversion() {
        assert_eq!(SystemHealthMonitor::status_to_string(&HealthStatus::Healthy), "Healthy");
        assert_eq!(SystemHealthMonitor::status_to_string(&HealthStatus::Warning), "Warning");
        assert_eq!(SystemHealthMonitor::status_to_string(&HealthStatus::Critical), "Critical");
        assert_eq!(SystemHealthMonitor::status_to_string(&HealthStatus::Unknown), "Unknown");
    }

    #[tokio::test]
    async fn test_service_type_conversion() {
        assert_eq!(SystemHealthMonitor::service_type_to_string(&ServiceType::Database), "Database");
        assert_eq!(SystemHealthMonitor::service_type_to_string(&ServiceType::Redis), "Redis");
        assert_eq!(SystemHealthMonitor::service_type_to_string(&ServiceType::Kafka), "Kafka");
    }
}