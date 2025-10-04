//! 분석 시스템 연동
//!
//! 이 모듈은 머신러닝, 리스크 관리, 시장 분석 등
//! 다양한 분석 시스템과의 연동을 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};

/// 분석 시스템 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnalyticsSystem {
    MachineLearning,    // 머신러닝 시스템
    RiskManagement,     // 리스크 관리 시스템
    MarketAnalysis,     // 시장 분석 시스템
    FraudDetection,     // 사기 탐지 시스템
    SentimentAnalysis,  // 감정 분석 시스템
    PortfolioOptimization, // 포트폴리오 최적화
}

/// 분석 요청 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnalysisRequestType {
    PricePrediction,        // 가격 예측
    RiskAssessment,         // 리스크 평가
    MarketTrend,           // 시장 트렌드 분석
    FraudDetection,        // 사기 탐지
    SentimentAnalysis,     // 감정 분석
    PortfolioOptimization, // 포트폴리오 최적화
    AnomalyDetection,      // 이상 탐지
}

/// 분석 요청 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub request_id: String,
    pub system: AnalyticsSystem,
    pub request_type: AnalysisRequestType,
    pub symbol: Option<String>,
    pub data: serde_json::Value,
    pub timestamp: u64,
    pub priority: u8, // 1-10 (10이 최고 우선순위)
    pub timeout_ms: u64,
}

/// 분석 결과 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub request_id: String,
    pub system: AnalyticsSystem,
    pub request_type: AnalysisRequestType,
    pub result: serde_json::Value,
    pub confidence: f64, // 0.0-1.0
    pub processing_time_ms: u64,
    pub timestamp: u64,
    pub status: AnalysisStatus,
    pub error_message: Option<String>,
}

/// 분석 상태
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnalysisStatus {
    Pending,    // 대기 중
    Processing, // 처리 중
    Completed,  // 완료
    Failed,     // 실패
    Timeout,    // 타임아웃
}

/// 분석 시스템 설정
#[derive(Debug, Clone)]
pub struct AnalyticsIntegrationConfig {
    pub systems: Vec<AnalyticsSystem>,
    pub request_timeout_ms: u64,
    pub max_concurrent_requests: usize,
    pub enable_caching: bool,
    pub cache_ttl_ms: u64,
    pub retry_attempts: u32,
    pub retry_interval_ms: u64,
}

impl Default for AnalyticsIntegrationConfig {
    fn default() -> Self {
        Self {
            systems: vec![
                AnalyticsSystem::MachineLearning,
                AnalyticsSystem::RiskManagement,
                AnalyticsSystem::MarketAnalysis,
                AnalyticsSystem::FraudDetection,
                AnalyticsSystem::SentimentAnalysis,
            ],
            request_timeout_ms: 30000, // 30초
            max_concurrent_requests: 100,
            enable_caching: true,
            cache_ttl_ms: 300000, // 5분
            retry_attempts: 3,
            retry_interval_ms: 1000,
        }
    }
}

/// 분석 시스템 연동 관리자
pub struct AnalyticsIntegrationManager {
    /// 설정
    config: AnalyticsIntegrationConfig,
    /// 분석 요청 큐
    request_queue: Arc<RwLock<Vec<AnalysisRequest>>>,
    /// 분석 결과 저장소
    results: Arc<RwLock<HashMap<String, AnalysisResult>>>,
    /// 캐시 저장소
    cache: Arc<RwLock<HashMap<String, (AnalysisResult, u64)>>>,
    /// 처리 중인 요청 수
    active_requests: Arc<Mutex<usize>>,
    /// 분석 활성화 상태
    is_analyzing: Arc<Mutex<bool>>,
}

impl AnalyticsIntegrationManager {
    /// 새 분석 시스템 연동 관리자 생성
    pub fn new(config: AnalyticsIntegrationConfig) -> Self {
        Self {
            config,
            request_queue: Arc::new(RwLock::new(Vec::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            active_requests: Arc::new(Mutex::new(0)),
            is_analyzing: Arc::new(Mutex::new(false)),
        }
    }

    /// 분석 시스템 시작
    pub async fn start_analysis(&self) {
        let mut is_analyzing = self.is_analyzing.lock().await;
        if *is_analyzing {
            warn!("분석 시스템이 이미 실행 중입니다");
            return;
        }
        *is_analyzing = true;
        drop(is_analyzing);

        info!("분석 시스템 연동 시작");

        let request_queue = self.request_queue.clone();
        let results = self.results.clone();
        let cache = self.cache.clone();
        let active_requests = self.active_requests.clone();
        let config = self.config.clone();
        let is_analyzing = self.is_analyzing.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100)); // 100ms마다 체크

            loop {
                interval.tick().await;

                // 분석 중단 확인
                {
                    let analyzing = is_analyzing.lock().await;
                    if !*analyzing {
                        break;
                    }
                }

                // 활성 요청 수 확인
                let active_count = *active_requests.lock().await;
                if active_count >= config.max_concurrent_requests {
                    continue;
                }

                // 요청 큐에서 처리할 요청 가져오기
                let request = {
                    let mut queue = request_queue.write().await;
                    if queue.is_empty() {
                        continue;
                    }
                    
                    // 우선순위 기준으로 정렬
                    queue.sort_by(|a, b| b.priority.cmp(&a.priority));
                    queue.pop()
                };

                if let Some(request) = request {
                    // 캐시 확인
                    if config.enable_caching {
                        if let Some(cached_result) = Self::get_cached_result(&cache, &request).await {
                            let mut results_guard = results.write().await;
                            results_guard.insert(request.request_id.clone(), cached_result);
                            continue;
                        }
                    }

                    // 분석 요청 처리
                    let request_queue_clone = request_queue.clone();
                    let results_clone = results.clone();
                    let cache_clone = cache.clone();
                    let active_requests_clone = active_requests.clone();
                    let config_clone = config.clone();

                    tokio::spawn(async move {
                        *active_requests_clone.lock().await += 1;
                        
                        let result = Self::process_analysis_request(&request, &config_clone).await;
                        
                        // 결과 저장
                        {
                            let mut results_guard = results_clone.write().await;
                            results_guard.insert(request.request_id.clone(), result.clone());
                        }

                        // 캐시 저장
                        if config_clone.enable_caching {
                            let cache_key = Self::generate_cache_key(&request);
                            let cache_ttl = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64 + config_clone.cache_ttl_ms;
                            
                            let mut cache_guard = cache_clone.write().await;
                            cache_guard.insert(cache_key, (result, cache_ttl));
                        }

                        *active_requests_clone.lock().await -= 1;
                    });
                }
            }

            info!("분석 시스템 연동 종료");
        });
    }

    /// 분석 시스템 중단
    pub async fn stop_analysis(&self) {
        let mut is_analyzing = self.is_analyzing.lock().await;
        *is_analyzing = false;
        info!("분석 시스템 연동 중단 요청");
    }

    /// 분석 요청 제출
    pub async fn submit_analysis_request(&self, request: AnalysisRequest) -> Result<String, String> {
        // 요청 큐에 추가
        {
            let mut queue = self.request_queue.write().await;
            queue.push(request.clone());
        }

        info!("분석 요청 제출: {} ({})", request.request_id, request.request_type);
        Ok(request.request_id)
    }

    /// 분석 결과 조회
    pub async fn get_analysis_result(&self, request_id: &str) -> Option<AnalysisResult> {
        let results = self.results.read().await;
        results.get(request_id).cloned()
    }

    /// 분석 요청 처리
    async fn process_analysis_request(
        request: &AnalysisRequest,
        config: &AnalyticsIntegrationConfig,
    ) -> AnalysisResult {
        let start_time = SystemTime::now();
        
        // Mock: 실제로는 해당 분석 시스템에 요청 전송
        let processing_time = match request.system {
            AnalyticsSystem::MachineLearning => {
                sleep(Duration::from_millis(2000 + fastrand::u32(0..3000))).await; // 2-5초
                2000 + fastrand::u32(0..3000)
            }
            AnalyticsSystem::RiskManagement => {
                sleep(Duration::from_millis(1000 + fastrand::u32(0..2000))).await; // 1-3초
                1000 + fastrand::u32(0..2000)
            }
            AnalyticsSystem::MarketAnalysis => {
                sleep(Duration::from_millis(1500 + fastrand::u32(0..2500))).await; // 1.5-4초
                1500 + fastrand::u32(0..2500)
            }
            AnalyticsSystem::FraudDetection => {
                sleep(Duration::from_millis(500 + fastrand::u32(0..1000))).await; // 0.5-1.5초
                500 + fastrand::u32(0..1000)
            }
            AnalyticsSystem::SentimentAnalysis => {
                sleep(Duration::from_millis(3000 + fastrand::u32(0..4000))).await; // 3-7초
                3000 + fastrand::u32(0..4000)
            }
            AnalyticsSystem::PortfolioOptimization => {
                sleep(Duration::from_millis(5000 + fastrand::u32(0..5000))).await; // 5-10초
                5000 + fastrand::u32(0..5000)
            }
        };

        let processing_time_ms = processing_time as u64;

        // 타임아웃 확인
        if processing_time_ms > request.timeout_ms {
            return AnalysisResult {
                request_id: request.request_id.clone(),
                system: request.system.clone(),
                request_type: request.request_type.clone(),
                result: serde_json::Value::Null,
                confidence: 0.0,
                processing_time_ms,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                status: AnalysisStatus::Timeout,
                error_message: Some("Request timeout".to_string()),
            };
        }

        // 95% 성공률로 시뮬레이션
        if fastrand::f32() < 0.95 {
            let result = Self::generate_mock_result(&request);
            let confidence = 0.7 + (fastrand::f32() * 0.3); // 0.7-1.0

            AnalysisResult {
                request_id: request.request_id.clone(),
                system: request.system.clone(),
                request_type: request.request_type.clone(),
                result,
                confidence,
                processing_time_ms,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                status: AnalysisStatus::Completed,
                error_message: None,
            }
        } else {
            AnalysisResult {
                request_id: request.request_id.clone(),
                system: request.system.clone(),
                request_type: request.request_type.clone(),
                result: serde_json::Value::Null,
                confidence: 0.0,
                processing_time_ms,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                status: AnalysisStatus::Failed,
                error_message: Some("Analysis system error".to_string()),
            }
        }
    }

    /// Mock 결과 생성
    fn generate_mock_result(request: &AnalysisRequest) -> serde_json::Value {
        match request.request_type {
            AnalysisRequestType::PricePrediction => {
                serde_json::json!({
                    "predicted_price": 50000000.0 + (fastrand::f32() * 1000000.0),
                    "price_range": {
                        "min": 49000000.0,
                        "max": 51000000.0
                    },
                    "trend": "bullish",
                    "probability": 0.75
                })
            }
            AnalysisRequestType::RiskAssessment => {
                serde_json::json!({
                    "risk_score": fastrand::f32(),
                    "risk_level": "medium",
                    "risk_factors": ["volatility", "liquidity"],
                    "recommendation": "monitor"
                })
            }
            AnalysisRequestType::MarketTrend => {
                serde_json::json!({
                    "trend": "bullish",
                    "strength": fastrand::f32(),
                    "duration": "short_term",
                    "indicators": ["rsi", "macd", "bollinger"]
                })
            }
            AnalysisRequestType::FraudDetection => {
                serde_json::json!({
                    "fraud_probability": fastrand::f32() * 0.3, // 0-30%
                    "risk_level": "low",
                    "patterns": ["normal"],
                    "recommendation": "approve"
                })
            }
            AnalysisRequestType::SentimentAnalysis => {
                serde_json::json!({
                    "sentiment": "positive",
                    "confidence": fastrand::f32(),
                    "sources": ["news", "social_media", "forums"],
                    "keywords": ["bullish", "growth", "positive"]
                })
            }
            AnalysisRequestType::PortfolioOptimization => {
                serde_json::json!({
                    "optimal_allocation": {
                        "BTC": 0.4,
                        "ETH": 0.3,
                        "XRP": 0.2,
                        "CASH": 0.1
                    },
                    "expected_return": 0.15,
                    "risk_level": "medium"
                })
            }
            AnalysisRequestType::AnomalyDetection => {
                serde_json::json!({
                    "anomaly_detected": fastrand::f32() < 0.1, // 10% 확률
                    "anomaly_type": "price_spike",
                    "severity": "low",
                    "confidence": fastrand::f32()
                })
            }
        }
    }

    /// 캐시된 결과 조회
    async fn get_cached_result(
        cache: &Arc<RwLock<HashMap<String, (AnalysisResult, u64)>>>,
        request: &AnalysisRequest,
    ) -> Option<AnalysisResult> {
        let cache_key = Self::generate_cache_key(request);
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let cache_guard = cache.read().await;
        if let Some((result, expiry_time)) = cache_guard.get(&cache_key) {
            if current_time < *expiry_time {
                return Some(result.clone());
            }
        }
        None
    }

    /// 캐시 키 생성
    fn generate_cache_key(request: &AnalysisRequest) -> String {
        format!("{}_{}_{}", request.system, request.request_type, request.symbol.as_deref().unwrap_or(""))
    }

    /// 분석 통계 조회
    pub async fn get_analysis_stats(&self) -> AnalyticsStats {
        let request_queue = self.request_queue.read().await;
        let results = self.results.read().await;
        let cache = self.cache.read().await;
        let active_requests = *self.active_requests.lock().await;

        let mut status_counts = HashMap::new();
        let mut system_counts = HashMap::new();
        let mut type_counts = HashMap::new();

        for result in results.values() {
            *status_counts.entry(result.status.clone()).or_insert(0) += 1;
            *system_counts.entry(result.system.clone()).or_insert(0) += 1;
            *type_counts.entry(result.request_type.clone()).or_insert(0) += 1;
        }

        let mut total_confidence = 0.0;
        let mut completed_count = 0;
        for result in results.values() {
            if result.status == AnalysisStatus::Completed {
                total_confidence += result.confidence;
                completed_count += 1;
            }
        }

        let average_confidence = if completed_count > 0 {
            total_confidence / completed_count as f64
        } else {
            0.0
        };

        AnalyticsStats {
            pending_requests: request_queue.len(),
            active_requests,
            total_completed: results.len(),
            cached_results: cache.len(),
            status_counts,
            system_counts,
            type_counts,
            average_confidence,
            is_analyzing: *self.is_analyzing.lock().await,
        }
    }
}

/// 분석 통계
#[derive(Debug, Clone)]
pub struct AnalyticsStats {
    pub pending_requests: usize,
    pub active_requests: usize,
    pub total_completed: usize,
    pub cached_results: usize,
    pub status_counts: HashMap<AnalysisStatus, usize>,
    pub system_counts: HashMap<AnalyticsSystem, usize>,
    pub type_counts: HashMap<AnalysisRequestType, usize>,
    pub average_confidence: f64,
    pub is_analyzing: bool,
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

    pub fn u32(range: std::ops::Range<u32>) -> u32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        range.start + ((hash % (range.end - range.start) as u64) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_analytics_integration_manager_creation() {
        let config = AnalyticsIntegrationConfig::default();
        let manager = AnalyticsIntegrationManager::new(config);
        
        let stats = manager.get_analysis_stats().await;
        assert_eq!(stats.pending_requests, 0);
        assert_eq!(stats.active_requests, 0);
        assert_eq!(stats.total_completed, 0);
        assert!(!stats.is_analyzing);
    }

    #[tokio::test]
    async fn test_analysis_request_submission() {
        let config = AnalyticsIntegrationConfig::default();
        let manager = AnalyticsIntegrationManager::new(config);
        
        let request = AnalysisRequest {
            request_id: "test_request_1".to_string(),
            system: AnalyticsSystem::MachineLearning,
            request_type: AnalysisRequestType::PricePrediction,
            symbol: Some("BTC-KRW".to_string()),
            data: serde_json::json!({"price": 50000000}),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            priority: 5,
            timeout_ms: 30000,
        };
        
        let result = manager.submit_analysis_request(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_request_1");
        
        let stats = manager.get_analysis_stats().await;
        assert_eq!(stats.pending_requests, 1);
    }

    #[tokio::test]
    async fn test_mock_result_generation() {
        let request = AnalysisRequest {
            request_id: "test_request_1".to_string(),
            system: AnalyticsSystem::MachineLearning,
            request_type: AnalysisRequestType::PricePrediction,
            symbol: Some("BTC-KRW".to_string()),
            data: serde_json::json!({"price": 50000000}),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            priority: 5,
            timeout_ms: 30000,
        };
        
        let result = AnalyticsIntegrationManager::generate_mock_result(&request);
        assert!(result.is_object());
        assert!(result.get("predicted_price").is_some());
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let request = AnalysisRequest {
            request_id: "test_request_1".to_string(),
            system: AnalyticsSystem::MachineLearning,
            request_type: AnalysisRequestType::PricePrediction,
            symbol: Some("BTC-KRW".to_string()),
            data: serde_json::json!({"price": 50000000}),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            priority: 5,
            timeout_ms: 30000,
        };
        
        let cache_key = AnalyticsIntegrationManager::generate_cache_key(&request);
        assert_eq!(cache_key, "MachineLearning_PricePrediction_BTC-KRW");
    }

    #[tokio::test]
    async fn test_analytics_config() {
        let config = AnalyticsIntegrationConfig {
            systems: vec![AnalyticsSystem::MachineLearning],
            request_timeout_ms: 60000,
            max_concurrent_requests: 50,
            enable_caching: false,
            cache_ttl_ms: 600000,
            retry_attempts: 5,
            retry_interval_ms: 2000,
        };
        
        assert_eq!(config.systems.len(), 1);
        assert_eq!(config.request_timeout_ms, 60000);
        assert_eq!(config.max_concurrent_requests, 50);
        assert!(!config.enable_caching);
        assert_eq!(config.retry_attempts, 5);
    }
}