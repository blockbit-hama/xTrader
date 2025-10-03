use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;
use log::{debug, error, info, warn};
use serde::{Serialize, Deserialize};

/// Write-Behind 캐시 시스템
/// 
/// 핵심 기능:
/// 1. 메모리에서 즉시 읽기/쓰기
/// 2. 백그라운드에서 DB 동기화
/// 3. 다층 캐싱 (L1: 메모리, L2: Redis, L3: DB)
/// 4. 캐시 무효화 및 일관성 보장
pub struct WriteBehindCache {
    /// L1 캐시: 메모리 (가장 빠름)
    l1_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    
    /// L2 캐시: Redis (중간 속도)
    l2_cache: Option<Arc<dyn CacheLayer>>,
    
    /// L3 캐시: DB (가장 느림)
    l3_cache: Arc<dyn CacheLayer>,
    
    /// 변경된 키들 추적
    dirty_keys: Arc<Mutex<HashSet<String>>>,
    
    /// 동기화 간격
    sync_interval: Duration,
    
    /// 캐시 TTL
    cache_ttl: Duration,
    
    /// 성능 메트릭
    metrics: Arc<CacheMetrics>,
    
    /// 동기화 작업자 핸들
    sync_worker: Option<tokio::task::JoinHandle<()>>,
}

/// 캐시 엔트리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub value: serde_json::Value,
    pub timestamp: Instant,
    pub version: u64,
    pub ttl: Duration,
}

impl CacheEntry {
    pub fn new(value: serde_json::Value, ttl: Duration) -> Self {
        Self {
            value,
            timestamp: Instant::now(),
            version: 0,
            ttl,
        }
    }
    
    pub fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
    
    pub fn update(&mut self, value: serde_json::Value) {
        self.value = value;
        self.timestamp = Instant::now();
        self.version += 1;
    }
}

/// 캐시 레이어 트레이트
#[async_trait::async_trait]
pub trait CacheLayer: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<serde_json::Value>, CacheError>;
    async fn set(&self, key: &str, value: &serde_json::Value) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;
}

/// 캐시 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("캐시 키를 찾을 수 없음: {0}")]
    KeyNotFound(String),
    #[error("캐시 만료됨: {0}")]
    Expired(String),
    #[error("캐시 레이어 오류: {0}")]
    LayerError(String),
    #[error("직렬화 오류: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// 캐시 성능 메트릭
#[derive(Debug, Default)]
pub struct CacheMetrics {
    pub l1_hits: std::sync::atomic::AtomicU64,
    pub l1_misses: std::sync::atomic::AtomicU64,
    pub l2_hits: std::sync::atomic::AtomicU64,
    pub l2_misses: std::sync::atomic::AtomicU64,
    pub l3_hits: std::sync::atomic::AtomicU64,
    pub l3_misses: std::sync::atomic::AtomicU64,
    pub sync_operations: std::sync::atomic::AtomicU64,
    pub sync_errors: std::sync::atomic::AtomicU64,
    pub start_time: Instant,
}

impl CacheMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            ..Default::default()
        }
    }
    
    pub fn record_l1_hit(&self) {
        self.l1_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_l1_miss(&self) {
        self.l1_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_l2_hit(&self) {
        self.l2_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_l2_miss(&self) {
        self.l2_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_l3_hit(&self) {
        self.l3_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_l3_miss(&self) {
        self.l3_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_sync_success(&self) {
        self.sync_operations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_sync_error(&self) {
        self.sync_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn get_l1_hit_rate(&self) -> f64 {
        let hits = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.l1_misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;
        if total > 0 {
            hits as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    }
    
    pub fn get_overall_hit_rate(&self) -> f64 {
        let total_hits = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed) +
                        self.l2_hits.load(std::sync::atomic::Ordering::Relaxed) +
                        self.l3_hits.load(std::sync::atomic::Ordering::Relaxed);
        let total_misses = self.l1_misses.load(std::sync::atomic::Ordering::Relaxed) +
                          self.l2_misses.load(std::sync::atomic::Ordering::Relaxed) +
                          self.l3_misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = total_hits + total_misses;
        if total > 0 {
            total_hits as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    }
}

impl WriteBehindCache {
    /// 새 Write-Behind 캐시 생성
    pub fn new(
        l3_cache: Arc<dyn CacheLayer>,
        sync_interval: Duration,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            l2_cache: None,
            l3_cache,
            dirty_keys: Arc::new(Mutex::new(HashSet::new())),
            sync_interval,
            cache_ttl,
            metrics: Arc::new(CacheMetrics::new()),
            sync_worker: None,
        }
    }
    
    /// L2 캐시 설정 (Redis 등)
    pub fn with_l2_cache(mut self, l2_cache: Arc<dyn CacheLayer>) -> Self {
        self.l2_cache = Some(l2_cache);
        self
    }
    
    /// 캐시 시작 (백그라운드 동기화 시작)
    pub fn start(&mut self) {
        let l1_cache = self.l1_cache.clone();
        let l2_cache = self.l2_cache.clone();
        let l3_cache = self.l3_cache.clone();
        let dirty_keys = self.dirty_keys.clone();
        let metrics = self.metrics.clone();
        let sync_interval = self.sync_interval;
        
        let sync_worker = tokio::spawn(async move {
            let mut interval = interval(sync_interval);
            
            loop {
                interval.tick().await;
                
                // 만료된 캐시 엔트리 정리
                Self::cleanup_expired_entries(&l1_cache).await;
                
                // 변경된 키들 동기화
                Self::sync_dirty_keys(&l1_cache, &l2_cache, &l3_cache, &dirty_keys, &metrics).await;
            }
        });
        
        self.sync_worker = Some(sync_worker);
        info!("🚀 Write-Behind 캐시 시작됨 (동기화 간격: {:?})", sync_interval);
    }
    
    /// 캐시 중지
    pub async fn stop(&mut self) {
        if let Some(worker) = self.sync_worker.take() {
            worker.abort();
            info!("Write-Behind 캐시 중지됨");
        }
    }
    
    /// 캐시에서 값 조회 (다층 캐시)
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, CacheError>
    where
        T: serde::de::DeserializeOwned,
    {
        // L1 캐시 확인 (메모리)
        if let Some(entry) = self.l1_cache.read().await.get(key) {
            if !entry.is_expired() {
                self.metrics.record_l1_hit();
                return Ok(Some(serde_json::from_value(entry.value.clone())?));
            } else {
                // 만료된 엔트리 제거
                self.l1_cache.write().await.remove(key);
            }
        }
        self.metrics.record_l1_miss();
        
        // L2 캐시 확인 (Redis)
        if let Some(ref l2_cache) = self.l2_cache {
            if let Ok(Some(value)) = l2_cache.get(key).await {
                self.metrics.record_l2_hit();
                
                // L1 캐시에 저장
                let entry = CacheEntry::new(value.clone(), self.cache_ttl);
                self.l1_cache.write().await.insert(key.to_string(), entry);
                
                return Ok(Some(serde_json::from_value(value)?));
            }
            self.metrics.record_l2_miss();
        }
        
        // L3 캐시 확인 (DB)
        if let Ok(Some(value)) = self.l3_cache.get(key).await {
            self.metrics.record_l3_hit();
            
            // L1, L2 캐시에 저장
            let entry = CacheEntry::new(value.clone(), self.cache_ttl);
            self.l1_cache.write().await.insert(key.to_string(), entry);
            
            if let Some(ref l2_cache) = self.l2_cache {
                let _ = l2_cache.set(key, &value).await;
            }
            
            return Ok(Some(serde_json::from_value(value)?));
        }
        self.metrics.record_l3_miss();
        
        Ok(None)
    }
    
    /// 캐시에 값 저장 (Write-Behind)
    pub async fn set<T>(&self, key: &str, value: &T) -> Result<(), CacheError>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        
        // L1 캐시에 즉시 저장 (메모리)
        let mut cache = self.l1_cache.write().await;
        if let Some(entry) = cache.get_mut(key) {
            entry.update(json_value.clone());
        } else {
            cache.insert(key.to_string(), CacheEntry::new(json_value.clone(), self.cache_ttl));
        }
        drop(cache);
        
        // 변경된 키로 마킹 (나중에 DB 동기화)
        self.dirty_keys.lock().await.insert(key.to_string());
        
        debug!("캐시 저장됨: {} (Write-Behind)", key);
        Ok(())
    }
    
    /// 캐시에서 값 삭제
    pub async fn delete(&self, key: &str) -> Result<(), CacheError> {
        // L1 캐시에서 삭제
        self.l1_cache.write().await.remove(key);
        
        // L2 캐시에서 삭제
        if let Some(ref l2_cache) = self.l2_cache {
            let _ = l2_cache.delete(key).await;
        }
        
        // L3 캐시에서 삭제
        self.l3_cache.delete(key).await?;
        
        // 변경된 키에서 제거
        self.dirty_keys.lock().await.remove(key);
        
        debug!("캐시 삭제됨: {}", key);
        Ok(())
    }
    
    /// 캐시 무효화
    pub async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        self.l1_cache.write().await.remove(key);
        
        if let Some(ref l2_cache) = self.l2_cache {
            let _ = l2_cache.delete(key).await;
        }
        
        debug!("캐시 무효화됨: {}", key);
        Ok(())
    }
    
    /// 만료된 캐시 엔트리 정리
    async fn cleanup_expired_entries(l1_cache: &Arc<RwLock<HashMap<String, CacheEntry>>>) {
        let mut cache = l1_cache.write().await;
        cache.retain(|_, entry| !entry.is_expired());
    }
    
    /// 변경된 키들 동기화
    async fn sync_dirty_keys(
        l1_cache: &Arc<RwLock<HashMap<String, CacheEntry>>>,
        l2_cache: &Option<Arc<dyn CacheLayer>>,
        l3_cache: &Arc<dyn CacheLayer>,
        dirty_keys: &Arc<Mutex<HashSet<String>>>,
        metrics: &Arc<CacheMetrics>,
    ) {
        let keys_to_sync = {
            let mut dirty_keys = dirty_keys.lock().await;
            dirty_keys.drain().collect::<Vec<_>>()
        };
        
        if keys_to_sync.is_empty() {
            return;
        }
        
        let cache = l1_cache.read().await;
        
        for key in keys_to_sync {
            if let Some(entry) = cache.get(&key) {
                // L2 캐시에 동기화
                if let Some(ref l2_cache) = l2_cache {
                    if let Err(e) = l2_cache.set(&key, &entry.value).await {
                        warn!("L2 캐시 동기화 실패: {} - {}", key, e);
                    }
                }
                
                // L3 캐시에 동기화
                if let Err(e) = l3_cache.set(&key, &entry.value).await {
                    warn!("L3 캐시 동기화 실패: {} - {}", key, e);
                    metrics.record_sync_error();
                } else {
                    metrics.record_sync_success();
                }
            }
        }
        
        debug!("캐시 동기화 완료: {}개 키", keys_to_sync.len());
    }
    
    /// 성능 메트릭 조회
    pub fn get_metrics(&self) -> &CacheMetrics {
        &self.metrics
    }
    
    /// 성능 통계 출력
    pub fn print_performance_stats(&self) {
        let metrics = &self.metrics;
        info!("📊 Write-Behind 캐시 성능 통계:");
        info!("   L1 히트율: {:.2}%", metrics.get_l1_hit_rate());
        info!("   전체 히트율: {:.2}%", metrics.get_overall_hit_rate());
        info!("   L1 히트: {}", metrics.l1_hits.load(std::sync::atomic::Ordering::Relaxed));
        info!("   L1 미스: {}", metrics.l1_misses.load(std::sync::atomic::Ordering::Relaxed));
        info!("   L2 히트: {}", metrics.l2_hits.load(std::sync::atomic::Ordering::Relaxed));
        info!("   L2 미스: {}", metrics.l2_misses.load(std::sync::atomic::Ordering::Relaxed));
        info!("   L3 히트: {}", metrics.l3_hits.load(std::sync::atomic::Ordering::Relaxed));
        info!("   L3 미스: {}", metrics.l3_misses.load(std::sync::atomic::Ordering::Relaxed));
        info!("   동기화 성공: {}", metrics.sync_operations.load(std::sync::atomic::Ordering::Relaxed));
        info!("   동기화 실패: {}", metrics.sync_errors.load(std::sync::atomic::Ordering::Relaxed));
    }
    
    /// 캐시 상태 조회
    pub async fn get_cache_status(&self) -> CacheStatus {
        let l1_size = self.l1_cache.read().await.len();
        let dirty_count = self.dirty_keys.lock().await.len();
        
        CacheStatus {
            l1_size,
            dirty_count,
            l2_enabled: self.l2_cache.is_some(),
        }
    }
}

/// 캐시 상태 정보
#[derive(Debug, Clone)]
pub struct CacheStatus {
    pub l1_size: usize,
    pub dirty_count: usize,
    pub l2_enabled: bool,
}

/// 잔고 전용 Write-Behind 캐시
pub struct BalanceCache {
    cache: WriteBehindCache,
}

impl BalanceCache {
    pub fn new(l3_cache: Arc<dyn CacheLayer>) -> Self {
        let cache = WriteBehindCache::new(
            l3_cache,
            Duration::from_millis(100), // 100ms마다 동기화
            Duration::from_secs(300),   // 5분 TTL
        );
        
        Self { cache }
    }
    
    /// 잔고 조회 (메모리 우선)
    pub async fn get_balance(&self, user_id: &str, asset: &str) -> Result<Option<u64>, CacheError> {
        let key = format!("balance:{}:{}", user_id, asset);
        self.cache.get::<u64>(&key).await
    }
    
    /// 잔고 업데이트 (Write-Behind)
    pub async fn update_balance(&self, user_id: &str, asset: &str, amount: u64) -> Result<(), CacheError> {
        let key = format!("balance:{}:{}", user_id, asset);
        self.cache.set(&key, &amount).await
    }
    
    /// 잔고 증가
    pub async fn add_balance(&self, user_id: &str, asset: &str, delta: u64) -> Result<u64, CacheError> {
        let current = self.get_balance(user_id, asset).await?.unwrap_or(0);
        let new_amount = current + delta;
        self.update_balance(user_id, asset, new_amount).await?;
        Ok(new_amount)
    }
    
    /// 잔고 감소
    pub async fn subtract_balance(&self, user_id: &str, asset: &str, delta: u64) -> Result<Option<u64>, CacheError> {
        let current = self.get_balance(user_id, asset).await?.unwrap_or(0);
        if current >= delta {
            let new_amount = current - delta;
            self.update_balance(user_id, asset, new_amount).await?;
            Ok(Some(new_amount))
        } else {
            Ok(None) // 잔고 부족
        }
    }
    
    pub fn start(&mut self) {
        self.cache.start();
    }
    
    pub async fn stop(&mut self) {
        self.cache.stop().await;
    }
    
    pub fn get_metrics(&self) -> &CacheMetrics {
        self.cache.get_metrics()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    // Mock 캐시 레이어
    struct MockCacheLayer {
        data: std::sync::Mutex<std::collections::HashMap<String, serde_json::Value>>,
    }
    
    impl MockCacheLayer {
        fn new() -> Self {
            Self {
                data: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }
    
    #[async_trait::async_trait]
    impl CacheLayer for MockCacheLayer {
        async fn get(&self, key: &str) -> Result<Option<serde_json::Value>, CacheError> {
            Ok(self.data.lock().unwrap().get(key).cloned())
        }
        
        async fn set(&self, key: &str, value: &serde_json::Value) -> Result<(), CacheError> {
            self.data.lock().unwrap().insert(key.to_string(), value.clone());
            Ok(())
        }
        
        async fn delete(&self, key: &str) -> Result<(), CacheError> {
            self.data.lock().unwrap().remove(key);
            Ok(())
        }
        
        async fn exists(&self, key: &str) -> Result<bool, CacheError> {
            Ok(self.data.lock().unwrap().contains_key(key))
        }
    }
    
    #[tokio::test]
    async fn test_write_behind_cache_basic() {
        let l3_cache = Arc::new(MockCacheLayer::new());
        let mut cache = WriteBehindCache::new(
            l3_cache,
            Duration::from_millis(10),
            Duration::from_secs(1),
        );
        
        cache.start();
        
        // 값 저장
        cache.set("test_key", &42u64).await.unwrap();
        
        // 값 조회
        let value: Option<u64> = cache.get("test_key").await.unwrap();
        assert_eq!(value, Some(42));
        
        // L1 히트 확인
        let metrics = cache.get_metrics();
        assert!(metrics.l1_hits.load(std::sync::atomic::Ordering::Relaxed) > 0);
        
        cache.stop().await;
    }
    
    #[tokio::test]
    async fn test_balance_cache() {
        let l3_cache = Arc::new(MockCacheLayer::new());
        let mut balance_cache = BalanceCache::new(l3_cache);
        
        balance_cache.start();
        
        // 잔고 설정
        balance_cache.update_balance("user1", "KRW", 1000000).await.unwrap();
        
        // 잔고 조회
        let balance = balance_cache.get_balance("user1", "KRW").await.unwrap();
        assert_eq!(balance, Some(1000000));
        
        // 잔고 증가
        let new_balance = balance_cache.add_balance("user1", "KRW", 500000).await.unwrap();
        assert_eq!(new_balance, 1500000);
        
        // 잔고 감소
        let remaining = balance_cache.subtract_balance("user1", "KRW", 200000).await.unwrap();
        assert_eq!(remaining, Some(1300000));
        
        balance_cache.stop().await;
    }
}