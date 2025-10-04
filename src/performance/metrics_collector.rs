//! 성능 모니터링 및 메트릭 수집 시스템
//!
//! 이 모듈은 시스템 성능 메트릭을 수집하고 분석하여
//! 실시간 모니터링과 성능 최적화를 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, AtomicBool, Ordering};

/// 메트릭 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,    // 카운터 (증가만)
    Gauge,      // 게이지 (증가/감소)
    Histogram,  // 히스토그램 (분포)
    Timer,      // 타이머 (시간 측정)
}

/// 메트릭 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricData {
    pub name: String,
    pub value: f64,
    pub timestamp: u64,
    pub tags: HashMap<String, String>,
    pub metric_type: MetricType,
}

/// 히스토그램 버킷
#[derive(Debug, Clone)]
pub struct HistogramBucket {
    pub upper_bound: f64,
    pub count: u64,
}

/// 히스토그램 메트릭
#[derive(Debug, Clone)]
pub struct HistogramMetric {
    pub name: String,
    pub buckets: Vec<HistogramBucket>,
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub timestamp: u64,
}

/// 타이머 메트릭
#[derive(Debug, Clone)]
pub struct TimerMetric {
    pub name: String,
    pub duration_ms: f64,
    pub timestamp: u64,
    pub tags: HashMap<String, String>,
}

/// 성능 메트릭
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_usage_percent: f64,
    pub disk_usage_bytes: u64,
    pub disk_usage_percent: f64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
    pub active_connections: usize,
    pub request_rate_per_second: f64,
    pub response_time_ms: f64,
    pub error_rate_percent: f64,
    pub timestamp: u64,
}

/// 메트릭 수집기 설정
#[derive(Debug, Clone)]
pub struct MetricsCollectorConfig {
    pub collection_interval_ms: u64,
    pub retention_days: u32,
    pub enable_system_metrics: bool,
    pub enable_application_metrics: bool,
    pub enable_custom_metrics: bool,
    pub max_metrics_per_type: usize,
    pub enable_aggregation: bool,
    pub aggregation_window_ms: u64,
}

impl Default for MetricsCollectorConfig {
    fn default() -> Self {
        Self {
            collection_interval_ms: 1000, // 1초
            retention_days: 7,
            enable_system_metrics: true,
            enable_application_metrics: true,
            enable_custom_metrics: true,
            max_metrics_per_type: 10000,
            enable_aggregation: true,
            aggregation_window_ms: 60000, // 1분
        }
    }
}

/// 메트릭 수집기
pub struct MetricsCollector {
    config: MetricsCollectorConfig,
    counters: Arc<RwLock<HashMap<String, AtomicU64>>>,
    gauges: Arc<RwLock<HashMap<String, AtomicU64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<HistogramMetric>>>>,
    timers: Arc<RwLock<HashMap<String, Vec<TimerMetric>>>>,
    custom_metrics: Arc<RwLock<HashMap<String, Vec<MetricData>>>>,
    performance_metrics: Arc<RwLock<Vec<PerformanceMetrics>>>,
    is_running: Arc<Mutex<bool>>,
}

impl MetricsCollector {
    /// 새 메트릭 수집기 생성
    pub fn new(config: MetricsCollectorConfig) -> Self {
        Self {
            config,
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            timers: Arc::new(RwLock::new(HashMap::new())),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
            performance_metrics: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// 메트릭 수집기 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("메트릭 수집기가 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("메트릭 수집기 시작: 수집간격={}ms", self.config.collection_interval_ms);

        let counters = self.counters.clone();
        let gauges = self.gauges.clone();
        let histograms = self.histograms.clone();
        let timers = self.timers.clone();
        let custom_metrics = self.custom_metrics.clone();
        let performance_metrics = self.performance_metrics.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        // 메트릭 수집 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.collection_interval_ms));

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 시스템 메트릭 수집
                if config.enable_system_metrics {
                    Self::collect_system_metrics(&performance_metrics).await;
                }

                // 메트릭 정리
                Self::cleanup_old_metrics(
                    &histograms,
                    &timers,
                    &custom_metrics,
                    &performance_metrics,
                    &config,
                ).await;
            }

            info!("메트릭 수집기 종료");
        });
    }

    /// 메트릭 수집기 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("메트릭 수집기 중단 요청");
    }

    /// 카운터 증가
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut counters = self.counters.write().await;
        let counter = counters.entry(name.to_string()).or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(value, Ordering::Relaxed);
    }

    /// 카운터 값 조회
    pub async fn get_counter(&self, name: &str) -> Option<u64> {
        let counters = self.counters.read().await;
        counters.get(name).map(|c| c.load(Ordering::Relaxed))
    }

    /// 게이지 설정
    pub async fn set_gauge(&self, name: &str, value: u64) {
        let mut gauges = self.gauges.write().await;
        let gauge = gauges.entry(name.to_string()).or_insert_with(|| AtomicU64::new(0));
        gauge.store(value, Ordering::Relaxed);
    }

    /// 게이지 값 조회
    pub async fn get_gauge(&self, name: &str) -> Option<u64> {
        let gauges = self.gauges.read().await;
        gauges.get(name).map(|g| g.load(Ordering::Relaxed))
    }

    /// 히스토그램 기록
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        let histogram_list = histograms.entry(name.to_string()).or_insert_with(Vec::new);
        
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 기존 히스토그램 업데이트 또는 새로 생성
        if let Some(last_histogram) = histogram_list.last_mut() {
            if current_time - last_histogram.timestamp < self.config.aggregation_window_ms {
                // 기존 히스토그램 업데이트
                last_histogram.count += 1;
                last_histogram.sum += value;
                last_histogram.min = last_histogram.min.min(value);
                last_histogram.max = last_histogram.max.max(value);
                
                // 버킷 업데이트
                for bucket in &mut last_histogram.buckets {
                    if value <= bucket.upper_bound {
                        bucket.count += 1;
                    }
                }
                return;
            }
        }

        // 새 히스토그램 생성
        let buckets = vec![
            HistogramBucket { upper_bound: 0.1, count: 0 },
            HistogramBucket { upper_bound: 0.5, count: 0 },
            HistogramBucket { upper_bound: 1.0, count: 0 },
            HistogramBucket { upper_bound: 5.0, count: 0 },
            HistogramBucket { upper_bound: 10.0, count: 0 },
            HistogramBucket { upper_bound: f64::INFINITY, count: 0 },
        ];

        let mut new_histogram = HistogramMetric {
            name: name.to_string(),
            buckets,
            count: 1,
            sum: value,
            min: value,
            max: value,
            timestamp: current_time,
        };

        // 버킷에 값 추가
        for bucket in &mut new_histogram.buckets {
            if value <= bucket.upper_bound {
                bucket.count = 1;
            }
        }

        histogram_list.push(new_histogram);

        // 최대 개수 제한
        if histogram_list.len() > self.config.max_metrics_per_type {
            histogram_list.drain(0..histogram_list.len() - self.config.max_metrics_per_type);
        }
    }

    /// 타이머 기록
    pub async fn record_timer(&self, name: &str, duration_ms: f64, tags: HashMap<String, String>) {
        let mut timers = self.timers.write().await;
        let timer_list = timers.entry(name.to_string()).or_insert_with(Vec::new);
        
        let timer_metric = TimerMetric {
            name: name.to_string(),
            duration_ms,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            tags,
        };

        timer_list.push(timer_metric);

        // 최대 개수 제한
        if timer_list.len() > self.config.max_metrics_per_type {
            timer_list.drain(0..timer_list.len() - self.config.max_metrics_per_type);
        }
    }

    /// 커스텀 메트릭 추가
    pub async fn add_custom_metric(&self, metric: MetricData) {
        let mut custom_metrics = self.custom_metrics.write().await;
        let metric_list = custom_metrics.entry(metric.name.clone()).or_insert_with(Vec::new);
        
        metric_list.push(metric);

        // 최대 개수 제한
        if metric_list.len() > self.config.max_metrics_per_type {
            metric_list.drain(0..metric_list.len() - self.config.max_metrics_per_type);
        }
    }

    /// 시스템 메트릭 수집
    async fn collect_system_metrics(performance_metrics: &Arc<RwLock<Vec<PerformanceMetrics>>>) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Mock 시스템 메트릭
        let metrics = PerformanceMetrics {
            cpu_usage_percent: fastrand::f32() * 100.0,
            memory_usage_bytes: fastrand::u32(0..8_000_000_000) as u64, // 0-8GB
            memory_usage_percent: fastrand::f32() * 100.0,
            disk_usage_bytes: fastrand::u32(0..1_000_000_000_000) as u64, // 0-1TB
            disk_usage_percent: fastrand::f32() * 100.0,
            network_bytes_sent: fastrand::u32(0..1_000_000) as u64,
            network_bytes_received: fastrand::u32(0..1_000_000) as u64,
            active_connections: fastrand::usize(0..1000),
            request_rate_per_second: fastrand::f32() * 1000.0,
            response_time_ms: fastrand::f32() * 100.0,
            error_rate_percent: fastrand::f32() * 10.0,
            timestamp: current_time,
        };

        let mut perf_metrics = performance_metrics.write().await;
        perf_metrics.push(metrics);

        // 최대 개수 제한 (1시간치 데이터)
        if perf_metrics.len() > 3600 {
            perf_metrics.drain(0..perf_metrics.len() - 3600);
        }
    }

    /// 오래된 메트릭 정리
    async fn cleanup_old_metrics(
        histograms: &Arc<RwLock<HashMap<String, Vec<HistogramMetric>>>>,
        timers: &Arc<RwLock<HashMap<String, Vec<TimerMetric>>>>,
        custom_metrics: &Arc<RwLock<HashMap<String, Vec<MetricData>>>>,
        performance_metrics: &Arc<RwLock<Vec<PerformanceMetrics>>>,
        config: &MetricsCollectorConfig,
    ) {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - (config.retention_days as u64 * 24 * 60 * 60);

        // 히스토그램 정리
        {
            let mut histograms_guard = histograms.write().await;
            for histogram_list in histograms_guard.values_mut() {
                histogram_list.retain(|h| h.timestamp > cutoff_time);
            }
        }

        // 타이머 정리
        {
            let mut timers_guard = timers.write().await;
            for timer_list in timers_guard.values_mut() {
                timer_list.retain(|t| t.timestamp > cutoff_time);
            }
        }

        // 커스텀 메트릭 정리
        {
            let mut custom_metrics_guard = custom_metrics.write().await;
            for metric_list in custom_metrics_guard.values_mut() {
                metric_list.retain(|m| m.timestamp > cutoff_time);
            }
        }

        // 성능 메트릭 정리
        {
            let mut perf_metrics = performance_metrics.write().await;
            perf_metrics.retain(|m| m.timestamp > cutoff_time);
        }
    }

    /// 모든 카운터 조회
    pub async fn get_all_counters(&self) -> HashMap<String, u64> {
        let counters = self.counters.read().await;
        counters.iter().map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed))).collect()
    }

    /// 모든 게이지 조회
    pub async fn get_all_gauges(&self) -> HashMap<String, u64> {
        let gauges = self.gauges.read().await;
        gauges.iter().map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed))).collect()
    }

    /// 히스토그램 조회
    pub async fn get_histogram(&self, name: &str) -> Option<HistogramMetric> {
        let histograms = self.histograms.read().await;
        histograms.get(name).and_then(|list| list.last()).cloned()
    }

    /// 타이머 통계 조회
    pub async fn get_timer_stats(&self, name: &str) -> Option<(f64, f64, f64, usize)> {
        let timers = self.timers.read().await;
        if let Some(timer_list) = timers.get(name) {
            if timer_list.is_empty() {
                return None;
            }

            let durations: Vec<f64> = timer_list.iter().map(|t| t.duration_ms).collect();
            let sum: f64 = durations.iter().sum();
            let count = durations.len();
            let avg = sum / count as f64;
            let min = durations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max = durations.iter().fold(0.0, |a, &b| a.max(b));

            Some((avg, min, max, count))
        } else {
            None
        }
    }

    /// 성능 메트릭 조회
    pub async fn get_performance_metrics(&self) -> Vec<PerformanceMetrics> {
        self.performance_metrics.read().await.clone()
    }

    /// 최신 성능 메트릭 조회
    pub async fn get_latest_performance_metrics(&self) -> Option<PerformanceMetrics> {
        let perf_metrics = self.performance_metrics.read().await;
        perf_metrics.last().cloned()
    }

    /// 메트릭 요약 조회
    pub async fn get_metrics_summary(&self) -> MetricsSummary {
        let counters = self.get_all_counters().await;
        let gauges = self.get_all_gauges().await;
        let perf_metrics = self.get_latest_performance_metrics().await;

        MetricsSummary {
            total_counters: counters.len(),
            total_gauges: gauges.len(),
            total_histograms: self.histograms.read().await.len(),
            total_timers: self.timers.read().await.len(),
            total_custom_metrics: self.custom_metrics.read().await.len(),
            performance_metrics: perf_metrics,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

/// 메트릭 요약
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub total_counters: usize,
    pub total_gauges: usize,
    pub total_histograms: usize,
    pub total_timers: usize,
    pub total_custom_metrics: usize,
    pub performance_metrics: Option<PerformanceMetrics>,
    pub timestamp: u64,
}

/// 성능 분석기
pub struct PerformanceAnalyzer {
    metrics_collector: Arc<MetricsCollector>,
}

impl PerformanceAnalyzer {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self { metrics_collector }
    }

    /// 성능 분석 리포트 생성
    pub async fn generate_performance_report(&self) -> PerformanceReport {
        let perf_metrics = self.metrics_collector.get_performance_metrics().await;
        let counters = self.metrics_collector.get_all_counters().await;
        let gauges = self.metrics_collector.get_all_gauges().await;

        let mut avg_cpu = 0.0;
        let mut avg_memory = 0.0;
        let mut avg_response_time = 0.0;
        let mut max_cpu = 0.0;
        let mut max_memory = 0.0;
        let mut max_response_time = 0.0;

        if !perf_metrics.is_empty() {
            let cpu_values: Vec<f64> = perf_metrics.iter().map(|m| m.cpu_usage_percent).collect();
            let memory_values: Vec<f64> = perf_metrics.iter().map(|m| m.memory_usage_percent).collect();
            let response_values: Vec<f64> = perf_metrics.iter().map(|m| m.response_time_ms).collect();

            avg_cpu = cpu_values.iter().sum::<f64>() / cpu_values.len() as f64;
            avg_memory = memory_values.iter().sum::<f64>() / memory_values.len() as f64;
            avg_response_time = response_values.iter().sum::<f64>() / response_values.len() as f64;

            max_cpu = cpu_values.iter().fold(0.0, |a, &b| a.max(b));
            max_memory = memory_values.iter().fold(0.0, |a, &b| a.max(b));
            max_response_time = response_values.iter().fold(0.0, |a, &b| a.max(b));
        }

        PerformanceReport {
            analysis_period_ms: perf_metrics.len() as u64 * 1000,
            average_cpu_usage: avg_cpu,
            average_memory_usage: avg_memory,
            average_response_time: avg_response_time,
            peak_cpu_usage: max_cpu,
            peak_memory_usage: max_memory,
            peak_response_time: max_response_time,
            total_requests: counters.get("requests").copied().unwrap_or(0),
            total_errors: counters.get("errors").copied().unwrap_or(0),
            error_rate: if let Some(req_count) = counters.get("requests") {
                if *req_count > 0 {
                    counters.get("errors").copied().unwrap_or(0) as f64 / *req_count as f64 * 100.0
                } else {
                    0.0
                }
            } else {
                0.0
            },
            recommendations: self.generate_recommendations(avg_cpu, avg_memory, avg_response_time),
        }
    }

    fn generate_recommendations(&self, avg_cpu: f64, avg_memory: f64, avg_response_time: f64) -> Vec<String> {
        let mut recommendations = Vec::new();

        if avg_cpu > 80.0 {
            recommendations.push("CPU 사용률이 높습니다. 워커 수를 늘리거나 배치 크기를 줄이세요.".to_string());
        }

        if avg_memory > 80.0 {
            recommendations.push("메모리 사용률이 높습니다. 캐시 크기를 줄이거나 메모리 풀을 최적화하세요.".to_string());
        }

        if avg_response_time > 100.0 {
            recommendations.push("응답 시간이 느립니다. 데이터베이스 쿼리를 최적화하거나 캐시를 활용하세요.".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("시스템 성능이 양호합니다.".to_string());
        }

        recommendations
    }
}

/// 성능 리포트
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub analysis_period_ms: u64,
    pub average_cpu_usage: f64,
    pub average_memory_usage: f64,
    pub average_response_time: f64,
    pub peak_cpu_usage: f64,
    pub peak_memory_usage: f64,
    pub peak_response_time: f64,
    pub total_requests: u64,
    pub total_errors: u64,
    pub error_rate: f64,
    pub recommendations: Vec<String>,
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

    pub fn usize(range: std::ops::Range<usize>) -> usize {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        range.start + ((hash % (range.end - range.start) as u64) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let config = MetricsCollectorConfig::default();
        let collector = MetricsCollector::new(config);
        
        let summary = collector.get_metrics_summary().await;
        assert_eq!(summary.total_counters, 0);
        assert_eq!(summary.total_gauges, 0);
    }

    #[tokio::test]
    async fn test_counter_operations() {
        let config = MetricsCollectorConfig::default();
        let collector = MetricsCollector::new(config);
        
        collector.increment_counter("test_counter", 10).await;
        collector.increment_counter("test_counter", 5).await;
        
        assert_eq!(collector.get_counter("test_counter").await, Some(15));
        assert_eq!(collector.get_counter("nonexistent").await, None);
    }

    #[tokio::test]
    async fn test_gauge_operations() {
        let config = MetricsCollectorConfig::default();
        let collector = MetricsCollector::new(config);
        
        collector.set_gauge("test_gauge", 100).await;
        collector.set_gauge("test_gauge", 200).await;
        
        assert_eq!(collector.get_gauge("test_gauge").await, Some(200));
        assert_eq!(collector.get_gauge("nonexistent").await, None);
    }

    #[tokio::test]
    async fn test_histogram_recording() {
        let config = MetricsCollectorConfig::default();
        let collector = MetricsCollector::new(config);
        
        collector.record_histogram("test_histogram", 1.5).await;
        collector.record_histogram("test_histogram", 2.5).await;
        
        let histogram = collector.get_histogram("test_histogram").await;
        assert!(histogram.is_some());
        
        let histogram = histogram.unwrap();
        assert_eq!(histogram.count, 2);
        assert_eq!(histogram.sum, 4.0);
        assert_eq!(histogram.min, 1.5);
        assert_eq!(histogram.max, 2.5);
    }

    #[tokio::test]
    async fn test_timer_recording() {
        let config = MetricsCollectorConfig::default();
        let collector = MetricsCollector::new(config);
        
        let mut tags = HashMap::new();
        tags.insert("endpoint".to_string(), "/api/test".to_string());
        
        collector.record_timer("test_timer", 100.0, tags).await;
        
        let stats = collector.get_timer_stats("test_timer").await;
        assert!(stats.is_some());
        
        let (avg, min, max, count) = stats.unwrap();
        assert_eq!(avg, 100.0);
        assert_eq!(min, 100.0);
        assert_eq!(max, 100.0);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_performance_analyzer() {
        let config = MetricsCollectorConfig::default();
        let collector = Arc::new(MetricsCollector::new(config));
        
        collector.start().await;
        
        // 메트릭 수집 대기
        sleep(Duration::from_millis(2000)).await;
        
        let analyzer = PerformanceAnalyzer::new(collector.clone());
        let report = analyzer.generate_performance_report().await;
        
        assert!(report.analysis_period_ms > 0);
        assert!(report.recommendations.len() > 0);
        
        collector.stop().await;
    }
}