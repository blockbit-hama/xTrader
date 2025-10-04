//! 알림 시스템
//!
//! 이 모듈은 시스템 장애, 성능 이슈, 상태 변경에 대한
//! 다양한 채널을 통한 알림을 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

/// 알림 채널 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email,
    Slack,
    Discord,
    Webhook,
    SMS,
    Push,
    Log,
    Console,
}

/// 알림 우선순위
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,      // 낮음
    Normal,   // 보통
    High,     // 높음
    Critical, // 긴급
}

/// 알림 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NotificationType {
    HealthCheck,      // 헬스체크
    PerformanceAlert, // 성능 경고
    ErrorAlert,       // 에러 알림
    StatusChange,     // 상태 변경
    Maintenance,      // 유지보수
    SecurityAlert,    // 보안 경고
    Custom,           // 사용자 정의
}

/// 알림 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub id: String,
    pub title: String,
    pub content: String,
    pub channel: NotificationChannel,
    pub priority: NotificationPriority,
    pub notification_type: NotificationType,
    pub timestamp: u64,
    pub retry_count: u32,
    pub max_retries: u32,
    pub metadata: HashMap<String, String>,
}

/// 알림 전송 결과
#[derive(Debug, Clone)]
pub struct NotificationResult {
    pub message_id: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub sent_at: u64,
    pub channel: NotificationChannel,
}

/// 알림 통계
#[derive(Debug, Clone)]
pub struct NotificationStats {
    pub total_sent: u64,
    pub total_failed: u64,
    pub success_rate: f64,
    pub channel_stats: HashMap<String, ChannelStats>,
    pub priority_stats: HashMap<String, u64>,
    pub type_stats: HashMap<String, u64>,
    pub last_24h_sent: u64,
    pub last_24h_failed: u64,
}

/// 채널별 통계
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel: NotificationChannel,
    pub sent: u64,
    pub failed: u64,
    pub success_rate: f64,
    pub average_response_time_ms: u64,
}

/// 알림 설정
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    pub enabled_channels: Vec<NotificationChannel>,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub rate_limit_per_minute: u32,
    pub enable_deduplication: bool,
    pub deduplication_window_ms: u64,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled_channels: vec![
                NotificationChannel::Console,
                NotificationChannel::Log,
            ],
            retry_attempts: 3,
            retry_delay_ms: 1000,
            batch_size: 10,
            batch_timeout_ms: 5000,
            rate_limit_per_minute: 100,
            enable_deduplication: true,
            deduplication_window_ms: 60000, // 1분
        }
    }
}

/// 알림 시스템
pub struct NotificationSystem {
    config: NotificationConfig,
    message_queue: Arc<Mutex<Vec<NotificationMessage>>>,
    stats: Arc<RwLock<NotificationStats>>,
    is_running: Arc<Mutex<bool>>,
    channel_handlers: Arc<RwLock<HashMap<NotificationChannel, Arc<dyn NotificationChannelHandler + Send + Sync>>>>,
    rate_limiter: Arc<Mutex<RateLimiter>>,
    deduplicator: Arc<Mutex<Deduplicator>>,
}

/// 알림 채널 핸들러 트레이트
pub trait NotificationChannelHandler: Send + Sync {
    async fn send(&self, message: &NotificationMessage) -> Result<NotificationResult, String>;
    fn get_channel(&self) -> NotificationChannel;
    fn is_enabled(&self) -> bool;
}

/// 콘솔 알림 핸들러
pub struct ConsoleNotificationHandler;

impl NotificationChannelHandler for ConsoleNotificationHandler {
    async fn send(&self, message: &NotificationMessage) -> Result<NotificationResult, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        println!("🔔 알림 [{}] {}: {}", 
                 message.priority, message.title, message.content);

        Ok(NotificationResult {
            message_id: message.id.clone(),
            success: true,
            error_message: None,
            sent_at: timestamp,
            channel: NotificationChannel::Console,
        })
    }

    fn get_channel(&self) -> NotificationChannel {
        NotificationChannel::Console
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

/// 로그 알림 핸들러
pub struct LogNotificationHandler;

impl NotificationChannelHandler for LogNotificationHandler {
    async fn send(&self, message: &NotificationMessage) -> Result<NotificationResult, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        match message.priority {
            NotificationPriority::Critical => error!("🚨 CRITICAL: {} - {}", message.title, message.content),
            NotificationPriority::High => warn!("⚠️ HIGH: {} - {}", message.title, message.content),
            NotificationPriority::Normal => info!("ℹ️ NORMAL: {} - {}", message.title, message.content),
            NotificationPriority::Low => debug!("📝 LOW: {} - {}", message.title, message.content),
        }

        Ok(NotificationResult {
            message_id: message.id.clone(),
            success: true,
            error_message: None,
            sent_at: timestamp,
            channel: NotificationChannel::Log,
        })
    }

    fn get_channel(&self) -> NotificationChannel {
        NotificationChannel::Log
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

/// 이메일 알림 핸들러 (Mock)
pub struct EmailNotificationHandler {
    smtp_server: String,
    from_email: String,
}

impl EmailNotificationHandler {
    pub fn new(smtp_server: String, from_email: String) -> Self {
        Self { smtp_server, from_email }
    }
}

impl NotificationChannelHandler for EmailNotificationHandler {
    async fn send(&self, message: &NotificationMessage) -> Result<NotificationResult, String> {
        // Mock: 실제로는 SMTP를 통한 이메일 발송
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        debug!("📧 이메일 발송: {} -> {}", self.from_email, message.title);

        Ok(NotificationResult {
            message_id: message.id.clone(),
            success: true,
            error_message: None,
            sent_at: timestamp,
            channel: NotificationChannel::Email,
        })
    }

    fn get_channel(&self) -> NotificationChannel {
        NotificationChannel::Email
    }

    fn is_enabled(&self) -> bool {
        !self.smtp_server.is_empty()
    }
}

/// Slack 알림 핸들러 (Mock)
pub struct SlackNotificationHandler {
    webhook_url: String,
    channel: String,
}

impl SlackNotificationHandler {
    pub fn new(webhook_url: String, channel: String) -> Self {
        Self { webhook_url, channel }
    }
}

impl NotificationChannelHandler for SlackNotificationHandler {
    async fn send(&self, message: &NotificationMessage) -> Result<NotificationResult, String> {
        // Mock: 실제로는 Slack Webhook API 호출
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        debug!("💬 Slack 발송: #{} - {}", self.channel, message.title);

        Ok(NotificationResult {
            message_id: message.id.clone(),
            success: true,
            error_message: None,
            sent_at: timestamp,
            channel: NotificationChannel::Slack,
        })
    }

    fn get_channel(&self) -> NotificationChannel {
        NotificationChannel::Slack
    }

    fn is_enabled(&self) -> bool {
        !self.webhook_url.is_empty()
    }
}

/// 웹훅 알림 핸들러 (Mock)
pub struct WebhookNotificationHandler {
    webhook_url: String,
}

impl WebhookNotificationHandler {
    pub fn new(webhook_url: String) -> Self {
        Self { webhook_url }
    }
}

impl NotificationChannelHandler for WebhookNotificationHandler {
    async fn send(&self, message: &NotificationMessage) -> Result<NotificationResult, String> {
        // Mock: 실제로는 HTTP POST 요청
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        debug!("🔗 웹훅 발송: {} - {}", self.webhook_url, message.title);

        Ok(NotificationResult {
            message_id: message.id.clone(),
            success: true,
            error_message: None,
            sent_at: timestamp,
            channel: NotificationChannel::Webhook,
        })
    }

    fn get_channel(&self) -> NotificationChannel {
        NotificationChannel::Webhook
    }

    fn is_enabled(&self) -> bool {
        !self.webhook_url.is_empty()
    }
}

/// 속도 제한기
struct RateLimiter {
    requests: Vec<u64>,
    limit: u32,
    window_ms: u64,
}

impl RateLimiter {
    fn new(limit: u32, window_ms: u64) -> Self {
        Self {
            requests: Vec::new(),
            limit,
            window_ms,
        }
    }

    fn can_send(&mut self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 오래된 요청 제거
        self.requests.retain(|&time| now - time < self.window_ms);

        if self.requests.len() < self.limit as usize {
            self.requests.push(now);
            true
        } else {
            false
        }
    }
}

/// 중복 제거기
struct Deduplicator {
    messages: HashMap<String, u64>,
    window_ms: u64,
}

impl Deduplicator {
    fn new(window_ms: u64) -> Self {
        Self {
            messages: HashMap::new(),
            window_ms,
        }
    }

    fn is_duplicate(&mut self, message: &NotificationMessage) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 오래된 메시지 제거
        self.messages.retain(|_, &mut time| now - time < self.window_ms);

        let key = format!("{}:{}:{}", message.title, message.content, message.channel);
        
        if self.messages.contains_key(&key) {
            true
        } else {
            self.messages.insert(key, now);
            false
        }
    }
}

impl NotificationSystem {
    /// 새 알림 시스템 생성
    pub fn new(config: NotificationConfig) -> Self {
        let mut system = Self {
            config,
            message_queue: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(RwLock::new(NotificationStats {
                total_sent: 0,
                total_failed: 0,
                success_rate: 0.0,
                channel_stats: HashMap::new(),
                priority_stats: HashMap::new(),
                type_stats: HashMap::new(),
                last_24h_sent: 0,
                last_24h_failed: 0,
            })),
            is_running: Arc::new(Mutex::new(false)),
            channel_handlers: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(100, 60000))), // 1분에 100개
            deduplicator: Arc::new(Mutex::new(Deduplicator::new(60000))), // 1분 윈도우
        };

        // 기본 핸들러 등록
        system.register_default_handlers();
        system
    }

    /// 기본 핸들러 등록
    fn register_default_handlers(&mut self) {
        let mut handlers = self.channel_handlers.try_write().unwrap();
        
        handlers.insert(
            NotificationChannel::Console,
            Arc::new(ConsoleNotificationHandler),
        );
        
        handlers.insert(
            NotificationChannel::Log,
            Arc::new(LogNotificationHandler),
        );
    }

    /// 알림 시스템 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("알림 시스템이 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("알림 시스템 시작");

        let message_queue = self.message_queue.clone();
        let stats = self.stats.clone();
        let channel_handlers = self.channel_handlers.clone();
        let rate_limiter = self.rate_limiter.clone();
        let deduplicator = self.deduplicator.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        // 알림 처리 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.batch_timeout_ms));

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 배치 알림 처리
                Self::process_notification_batch(
                    &message_queue,
                    &stats,
                    &channel_handlers,
                    &rate_limiter,
                    &deduplicator,
                    &config,
                ).await;
            }

            info!("알림 시스템 종료");
        });
    }

    /// 알림 시스템 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("알림 시스템 중단 요청");
    }

    /// 알림 발송
    pub async fn send_notification(
        &self,
        title: String,
        content: String,
        channel: NotificationChannel,
        priority: NotificationPriority,
        notification_type: NotificationType,
    ) -> Result<String, String> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let message = NotificationMessage {
            id: message_id.clone(),
            title,
            content,
            channel,
            priority,
            notification_type,
            timestamp,
            retry_count: 0,
            max_retries: self.config.retry_attempts,
            metadata: HashMap::new(),
        };

        // 중복 체크
        if self.config.enable_deduplication {
            let mut deduplicator = self.deduplicator.lock().await;
            if deduplicator.is_duplicate(&message) {
                return Err("중복된 알림입니다".to_string());
            }
        }

        // 속도 제한 체크
        {
            let mut rate_limiter = self.rate_limiter.lock().await;
            if !rate_limiter.can_send() {
                return Err("속도 제한에 걸렸습니다".to_string());
            }
        }

        // 큐에 추가
        {
            let mut queue = self.message_queue.lock().await;
            queue.push(message);
        }

        Ok(message_id)
    }

    /// 배치 알림 처리
    async fn process_notification_batch(
        message_queue: &Arc<Mutex<Vec<NotificationMessage>>>,
        stats: &Arc<RwLock<NotificationStats>>,
        channel_handlers: &Arc<RwLock<HashMap<NotificationChannel, Arc<dyn NotificationChannelHandler + Send + Sync>>>>,
        rate_limiter: &Arc<Mutex<RateLimiter>>,
        deduplicator: &Arc<Mutex<Deduplicator>>,
        config: &NotificationConfig,
    ) {
        // 배치 메시지 수집
        let batch_messages = {
            let mut queue = message_queue.lock().await;
            let mut batch = Vec::new();
            
            for _ in 0..config.batch_size {
                if let Some(message) = queue.pop() {
                    batch.push(message);
                } else {
                    break;
                }
            }
            
            batch
        };

        if batch_messages.is_empty() {
            return;
        }

        let handlers = channel_handlers.read().await;

        for message in batch_messages {
            // 속도 제한 체크
            {
                let mut rate_limiter = rate_limiter.lock().await;
                if !rate_limiter.can_send() {
                    // 큐에 다시 추가
                    let mut queue = message_queue.lock().await;
                    queue.push(message);
                    break;
                }
            }

            // 중복 체크
            if config.enable_deduplication {
                let mut deduplicator = deduplicator.lock().await;
                if deduplicator.is_duplicate(&message) {
                    continue;
                }
            }

            // 채널 핸들러 찾기
            if let Some(handler) = handlers.get(&message.channel) {
                if handler.is_enabled() {
                    match handler.send(&message).await {
                        Ok(result) => {
                            Self::update_stats_on_success(stats, &message, &result).await;
                            debug!("알림 발송 성공: {} -> {}", message.title, message.channel);
                        }
                        Err(e) => {
                            Self::update_stats_on_failure(stats, &message, &e).await;
                            error!("알림 발송 실패: {} - {}", message.title, e);
                            
                            // 재시도 로직
                            if message.retry_count < message.max_retries {
                                let mut retry_message = message.clone();
                                retry_message.retry_count += 1;
                                
                                let mut queue = message_queue.lock().await;
                                queue.push(retry_message);
                            }
                        }
                    }
                }
            }
        }
    }

    /// 성공 통계 업데이트
    async fn update_stats_on_success(
        stats: &Arc<RwLock<NotificationStats>>,
        message: &NotificationMessage,
        result: &NotificationResult,
    ) {
        let mut stats_guard = stats.write().await;
        stats_guard.total_sent += 1;
        stats_guard.last_24h_sent += 1;

        // 채널별 통계 업데이트
        let channel_key = Self::channel_to_string(&message.channel);
        let channel_stats = stats_guard.channel_stats.entry(channel_key.clone()).or_insert(ChannelStats {
            channel: message.channel.clone(),
            sent: 0,
            failed: 0,
            success_rate: 0.0,
            average_response_time_ms: 0,
        });
        channel_stats.sent += 1;

        // 우선순위별 통계 업데이트
        let priority_key = Self::priority_to_string(&message.priority);
        *stats_guard.priority_stats.entry(priority_key).or_insert(0) += 1;

        // 타입별 통계 업데이트
        let type_key = Self::notification_type_to_string(&message.notification_type);
        *stats_guard.type_stats.entry(type_key).or_insert(0) += 1;

        // 성공률 계산
        let total = stats_guard.total_sent + stats_guard.total_failed;
        stats_guard.success_rate = if total > 0 {
            stats_guard.total_sent as f64 / total as f64
        } else {
            0.0
        };

        // 채널별 성공률 계산
        let channel_total = channel_stats.sent + channel_stats.failed;
        channel_stats.success_rate = if channel_total > 0 {
            channel_stats.sent as f64 / channel_total as f64
        } else {
            0.0
        };
    }

    /// 실패 통계 업데이트
    async fn update_stats_on_failure(
        stats: &Arc<RwLock<NotificationStats>>,
        message: &NotificationMessage,
        error: &str,
    ) {
        let mut stats_guard = stats.write().await;
        stats_guard.total_failed += 1;
        stats_guard.last_24h_failed += 1;

        // 채널별 통계 업데이트
        let channel_key = Self::channel_to_string(&message.channel);
        let channel_stats = stats_guard.channel_stats.entry(channel_key).or_insert(ChannelStats {
            channel: message.channel.clone(),
            sent: 0,
            failed: 0,
            success_rate: 0.0,
            average_response_time_ms: 0,
        });
        channel_stats.failed += 1;

        // 성공률 계산
        let total = stats_guard.total_sent + stats_guard.total_failed;
        stats_guard.success_rate = if total > 0 {
            stats_guard.total_sent as f64 / total as f64
        } else {
            0.0
        };
    }

    /// 채널 핸들러 등록
    pub async fn register_handler(&self, handler: Arc<dyn NotificationChannelHandler + Send + Sync>) {
        let mut handlers = self.channel_handlers.write().await;
        handlers.insert(handler.get_channel(), handler);
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> NotificationStats {
        self.stats.read().await.clone()
    }

    /// 큐 크기 조회
    pub async fn queue_size(&self) -> usize {
        let queue = self.message_queue.lock().await;
        queue.len()
    }

    /// 채널을 문자열로 변환
    fn channel_to_string(channel: &NotificationChannel) -> String {
        match channel {
            NotificationChannel::Email => "Email".to_string(),
            NotificationChannel::Slack => "Slack".to_string(),
            NotificationChannel::Discord => "Discord".to_string(),
            NotificationChannel::Webhook => "Webhook".to_string(),
            NotificationChannel::SMS => "SMS".to_string(),
            NotificationChannel::Push => "Push".to_string(),
            NotificationChannel::Log => "Log".to_string(),
            NotificationChannel::Console => "Console".to_string(),
        }
    }

    /// 우선순위를 문자열로 변환
    fn priority_to_string(priority: &NotificationPriority) -> String {
        match priority {
            NotificationPriority::Low => "Low".to_string(),
            NotificationPriority::Normal => "Normal".to_string(),
            NotificationPriority::High => "High".to_string(),
            NotificationPriority::Critical => "Critical".to_string(),
        }
    }

    /// 알림 타입을 문자열로 변환
    fn notification_type_to_string(notification_type: &NotificationType) -> String {
        match notification_type {
            NotificationType::HealthCheck => "HealthCheck".to_string(),
            NotificationType::PerformanceAlert => "PerformanceAlert".to_string(),
            NotificationType::ErrorAlert => "ErrorAlert".to_string(),
            NotificationType::StatusChange => "StatusChange".to_string(),
            NotificationType::Maintenance => "Maintenance".to_string(),
            NotificationType::SecurityAlert => "SecurityAlert".to_string(),
            NotificationType::Custom => "Custom".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notification_system_creation() {
        let config = NotificationConfig::default();
        let system = NotificationSystem::new(config);
        
        let stats = system.get_stats().await;
        assert_eq!(stats.total_sent, 0);
        assert_eq!(stats.total_failed, 0);
    }

    #[tokio::test]
    async fn test_notification_sending() {
        let config = NotificationConfig::default();
        let system = NotificationSystem::new(config);
        
        system.start().await;
        
        let result = system.send_notification(
            "테스트 알림".to_string(),
            "테스트 내용".to_string(),
            NotificationChannel::Console,
            NotificationPriority::Normal,
            NotificationType::Custom,
        ).await;
        
        assert!(result.is_ok());
        
        // 잠시 대기
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let stats = system.get_stats().await;
        assert!(stats.total_sent > 0);
        
        system.stop().await;
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = NotificationConfig {
            rate_limit_per_minute: 2,
            ..Default::default()
        };
        let system = NotificationSystem::new(config);
        
        // 첫 번째 알림
        let result1 = system.send_notification(
            "알림 1".to_string(),
            "내용 1".to_string(),
            NotificationChannel::Console,
            NotificationPriority::Normal,
            NotificationType::Custom,
        ).await;
        assert!(result1.is_ok());
        
        // 두 번째 알림
        let result2 = system.send_notification(
            "알림 2".to_string(),
            "내용 2".to_string(),
            NotificationChannel::Console,
            NotificationPriority::Normal,
            NotificationType::Custom,
        ).await;
        assert!(result2.is_ok());
        
        // 세 번째 알림 (속도 제한)
        let result3 = system.send_notification(
            "알림 3".to_string(),
            "내용 3".to_string(),
            NotificationChannel::Console,
            NotificationPriority::Normal,
            NotificationType::Custom,
        ).await;
        assert!(result3.is_err());
    }

    #[tokio::test]
    async fn test_deduplication() {
        let config = NotificationConfig {
            enable_deduplication: true,
            ..Default::default()
        };
        let system = NotificationSystem::new(config);
        
        // 첫 번째 알림
        let result1 = system.send_notification(
            "중복 테스트".to_string(),
            "동일한 내용".to_string(),
            NotificationChannel::Console,
            NotificationPriority::Normal,
            NotificationType::Custom,
        ).await;
        assert!(result1.is_ok());
        
        // 동일한 알림 (중복)
        let result2 = system.send_notification(
            "중복 테스트".to_string(),
            "동일한 내용".to_string(),
            NotificationChannel::Console,
            NotificationPriority::Normal,
            NotificationType::Custom,
        ).await;
        assert!(result2.is_err());
    }
}