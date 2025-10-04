//! MDP 캐싱 시스템
//!
//! 이 모듈은 봉차트와 시장 통계 데이터를 Redis에 캐싱하여
//! 고성능 데이터 조회를 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use crate::mdp::consumer::{CandlestickData, MarketStatistics};

/// 캐시 설정
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub redis_url: String,
    pub candlestick_ttl_seconds: u64,
    pub statistics_ttl_seconds: u64,
    pub update_interval_ms: u64,
    pub max_cache_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            candlestick_ttl_seconds: 300,    // 5분
            statistics_ttl_seconds: 60,       // 1분
            update_interval_ms: 1000,         // 1초마다 업데이트
            max_cache_size: 10000,
        }
    }
}

/// 캐시 키 타입
#[derive(Debug, Clone)]
pub enum CacheKey {
    Candlestick(String, String), // symbol, timeframe
    Statistics(String),          // symbol
    AllStatistics,
    AllCandlesticks(String),     // symbol
}

impl CacheKey {
    /// 캐시 키를 문자열로 변환
    pub fn to_string(&self) -> String {
        match self {
            CacheKey::Candlestick(symbol, timeframe) => {
                format!("candlestick:{}:{}", symbol, timeframe)
            }
            CacheKey::Statistics(symbol) => {
                format!("statistics:{}", symbol)
            }
            CacheKey::AllStatistics => {
                "statistics:all".to_string()
            }
            CacheKey::AllCandlesticks(symbol) => {
                format!("candlesticks:{}", symbol)
            }
        }
    }
}

/// 캐시된 봉차트 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCandlestickData {
    pub data: CandlestickData,
    pub cached_at: u64,
    pub ttl: u64,
}

/// 캐시된 시장 통계 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMarketStatistics {
    pub data: MarketStatistics,
    pub cached_at: u64,
    pub ttl: u64,
}

/// 캐시된 전체 통계 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAllStatistics {
    pub data: HashMap<String, MarketStatistics>,
    pub cached_at: u64,
    pub ttl: u64,
}

/// MDP 캐시 관리자
pub struct MDPCacheManager {
    /// Redis 연결 (Mock)
    redis_client: Arc<Mutex<MockRedisClient>>,
    /// 설정
    config: CacheConfig,
    /// 메모리 캐시 (로컬 백업)
    memory_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// 캐시 통계
    cache_stats: Arc<RwLock<CacheStats>>,
}

/// Mock Redis 클라이언트
struct MockRedisClient {
    data: HashMap<String, (Vec<u8>, u64)>, // key -> (value, expiry)
}

impl MockRedisClient {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    async fn set(&mut self, key: String, value: Vec<u8>, ttl_seconds: u64) -> Result<(), String> {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + ttl_seconds;
        
        self.data.insert(key, (value, expiry));
        Ok(())
    }

    async fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if let Some((value, expiry)) = self.data.get(key) {
            if current_time < *expiry {
                Ok(Some(value.clone()))
            } else {
                // 만료된 데이터 제거
                self.data.remove(key);
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn exists(&mut self, key: &str) -> Result<bool, String> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if let Some((_, expiry)) = self.data.get(key) {
            if current_time < *expiry {
                Ok(true)
            } else {
                self.data.remove(key);
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    async fn del(&mut self, key: &str) -> Result<usize, String> {
        if self.data.remove(key).is_some() {
            Ok(1)
        } else {
            Ok(0)
        }
    }

    async fn keys(&mut self, pattern: &str) -> Result<Vec<String>, String> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut keys = Vec::new();
        let mut expired_keys = Vec::new();
        
        for (key, (_, expiry)) in &self.data {
            if current_time < *expiry {
                if pattern == "*" || key.contains(pattern) {
                    keys.push(key.clone());
                }
            } else {
                expired_keys.push(key.clone());
            }
        }
        
        // 만료된 키 제거
        for key in expired_keys {
            self.data.remove(&key);
        }
        
        Ok(keys)
    }
}

/// 캐시 통계
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub deletes: u64,
    pub memory_cache_size: usize,
    pub redis_cache_size: usize,
}

impl MDPCacheManager {
    /// 새 캐시 관리자 생성
    pub fn new(config: CacheConfig) -> Self {
        Self {
            redis_client: Arc::new(Mutex::new(MockRedisClient::new())),
            config,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStats {
                hits: 0,
                misses: 0,
                sets: 0,
                deletes: 0,
                memory_cache_size: 0,
                redis_cache_size: 0,
            })),
        }
    }

    /// 캐시 관리자 시작
    pub async fn start(&self) {
        info!("MDP 캐시 관리자 시작");
        
        let redis_client = self.redis_client.clone();
        let memory_cache = self.memory_cache.clone();
        let cache_stats = self.cache_stats.clone();
        let config = self.config.clone();
        
        // 캐시 정리 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.update_interval_ms));
            
            loop {
                interval.tick().await;
                
                // 만료된 캐시 정리
                if let Err(e) = Self::cleanup_expired_cache(&redis_client, &memory_cache, &cache_stats).await {
                    error!("캐시 정리 실패: {}", e);
                }
                
                // 캐시 크기 제한
                if let Err(e) = Self::limit_cache_size(&memory_cache, config.max_cache_size).await {
                    error!("캐시 크기 제한 실패: {}", e);
                }
            }
        });
    }

    /// 봉차트 데이터 캐시
    pub async fn cache_candlestick(&self, symbol: &str, timeframe: &str, data: &CandlestickData) -> Result<(), String> {
        let key = CacheKey::Candlestick(symbol.to_string(), timeframe.to_string());
        let cached_data = CachedCandlestickData {
            data: data.clone(),
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: self.config.candlestick_ttl_seconds,
        };
        
        let serialized = serde_json::to_vec(&cached_data)
            .map_err(|e| format!("직렬화 실패: {}", e))?;
        
        // Redis에 저장
        let mut redis = self.redis_client.lock().await;
        redis.set(key.to_string(), serialized.clone(), self.config.candlestick_ttl_seconds).await?;
        
        // 메모리 캐시에도 저장
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.insert(key.to_string(), serialized);
        
        // 통계 업데이트
        self.update_cache_stats(|stats| stats.sets += 1).await;
        
        debug!("봉차트 캐시 저장: {} {}", symbol, timeframe);
        Ok(())
    }

    /// 시장 통계 캐시
    pub async fn cache_statistics(&self, symbol: &str, data: &MarketStatistics) -> Result<(), String> {
        let key = CacheKey::Statistics(symbol.to_string());
        let cached_data = CachedMarketStatistics {
            data: data.clone(),
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: self.config.statistics_ttl_seconds,
        };
        
        let serialized = serde_json::to_vec(&cached_data)
            .map_err(|e| format!("직렬화 실패: {}", e))?;
        
        // Redis에 저장
        let mut redis = self.redis_client.lock().await;
        redis.set(key.to_string(), serialized.clone(), self.config.statistics_ttl_seconds).await?;
        
        // 메모리 캐시에도 저장
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.insert(key.to_string(), serialized);
        
        // 통계 업데이트
        self.update_cache_stats(|stats| stats.sets += 1).await;
        
        debug!("시장 통계 캐시 저장: {}", symbol);
        Ok(())
    }

    /// 전체 시장 통계 캐시
    pub async fn cache_all_statistics(&self, data: &HashMap<String, MarketStatistics>) -> Result<(), String> {
        let key = CacheKey::AllStatistics;
        let cached_data = CachedAllStatistics {
            data: data.clone(),
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: self.config.statistics_ttl_seconds,
        };
        
        let serialized = serde_json::to_vec(&cached_data)
            .map_err(|e| format!("직렬화 실패: {}", e))?;
        
        // Redis에 저장
        let mut redis = self.redis_client.lock().await;
        redis.set(key.to_string(), serialized.clone(), self.config.statistics_ttl_seconds).await?;
        
        // 메모리 캐시에도 저장
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.insert(key.to_string(), serialized);
        
        // 통계 업데이트
        self.update_cache_stats(|stats| stats.sets += 1).await;
        
        debug!("전체 시장 통계 캐시 저장: {}개 심볼", data.len());
        Ok(())
    }

    /// 봉차트 데이터 조회
    pub async fn get_candlestick(&self, symbol: &str, timeframe: &str) -> Result<Option<CandlestickData>, String> {
        let key = CacheKey::Candlestick(symbol.to_string(), timeframe.to_string());
        
        // 메모리 캐시에서 먼저 확인
        if let Some(data) = self.get_from_memory_cache(&key.to_string()).await? {
            self.update_cache_stats(|stats| stats.hits += 1).await;
            return Ok(Some(data.data));
        }
        
        // Redis에서 확인
        if let Some(data) = self.get_from_redis_cache(&key.to_string()).await? {
            // 메모리 캐시에 저장
            self.set_memory_cache(&key.to_string(), &data).await?;
            self.update_cache_stats(|stats| stats.hits += 1).await;
            return Ok(Some(data.data));
        }
        
        self.update_cache_stats(|stats| stats.misses += 1).await;
        Ok(None)
    }

    /// 시장 통계 조회
    pub async fn get_statistics(&self, symbol: &str) -> Result<Option<MarketStatistics>, String> {
        let key = CacheKey::Statistics(symbol.to_string());
        
        // 메모리 캐시에서 먼저 확인
        if let Some(data) = self.get_from_memory_cache_statistics(&key.to_string()).await? {
            self.update_cache_stats(|stats| stats.hits += 1).await;
            return Ok(Some(data.data));
        }
        
        // Redis에서 확인
        if let Some(data) = self.get_from_redis_cache_statistics(&key.to_string()).await? {
            // 메모리 캐시에 저장
            self.set_memory_cache_statistics(&key.to_string(), &data).await?;
            self.update_cache_stats(|stats| stats.hits += 1).await;
            return Ok(Some(data.data));
        }
        
        self.update_cache_stats(|stats| stats.misses += 1).await;
        Ok(None)
    }

    /// 전체 시장 통계 조회
    pub async fn get_all_statistics(&self) -> Result<Option<HashMap<String, MarketStatistics>>, String> {
        let key = CacheKey::AllStatistics;
        
        // 메모리 캐시에서 먼저 확인
        if let Some(data) = self.get_from_memory_cache_all_statistics(&key.to_string()).await? {
            self.update_cache_stats(|stats| stats.hits += 1).await;
            return Ok(Some(data.data));
        }
        
        // Redis에서 확인
        if let Some(data) = self.get_from_redis_cache_all_statistics(&key.to_string()).await? {
            // 메모리 캐시에 저장
            self.set_memory_cache_all_statistics(&key.to_string(), &data).await?;
            self.update_cache_stats(|stats| stats.hits += 1).await;
            return Ok(Some(data.data));
        }
        
        self.update_cache_stats(|stats| stats.misses += 1).await;
        Ok(None)
    }

    /// 캐시에서 데이터 제거
    pub async fn invalidate(&self, key: &CacheKey) -> Result<(), String> {
        let key_str = key.to_string();
        
        // Redis에서 제거
        let mut redis = self.redis_client.lock().await;
        redis.del(&key_str).await?;
        
        // 메모리 캐시에서도 제거
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.remove(&key_str);
        
        // 통계 업데이트
        self.update_cache_stats(|stats| stats.deletes += 1).await;
        
        debug!("캐시 무효화: {}", key_str);
        Ok(())
    }

    /// 캐시 통계 조회
    pub async fn get_cache_stats(&self) -> CacheStats {
        let stats = self.cache_stats.read().await;
        let memory_cache_size = self.memory_cache.read().await.len();
        
        CacheStats {
            hits: stats.hits,
            misses: stats.misses,
            sets: stats.sets,
            deletes: stats.deletes,
            memory_cache_size,
            redis_cache_size: stats.redis_cache_size,
        }
    }

    /// 메모리 캐시에서 데이터 조회
    async fn get_from_memory_cache(&self, key: &str) -> Result<Option<CachedCandlestickData>, String> {
        let memory_cache = self.memory_cache.read().await;
        if let Some(data) = memory_cache.get(key) {
            let cached: CachedCandlestickData = serde_json::from_slice(data)
                .map_err(|e| format!("역직렬화 실패: {}", e))?;
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    /// Redis 캐시에서 데이터 조회
    async fn get_from_redis_cache(&self, key: &str) -> Result<Option<CachedCandlestickData>, String> {
        let mut redis = self.redis_client.lock().await;
        if let Some(data) = redis.get(key).await? {
            let cached: CachedCandlestickData = serde_json::from_slice(&data)
                .map_err(|e| format!("역직렬화 실패: {}", e))?;
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    /// 메모리 캐시에 데이터 저장
    async fn set_memory_cache(&self, key: &str, data: &CachedCandlestickData) -> Result<(), String> {
        let serialized = serde_json::to_vec(data)
            .map_err(|e| format!("직렬화 실패: {}", e))?;
        
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.insert(key.to_string(), serialized);
        
        Ok(())
    }

    /// 메모리 캐시에서 통계 데이터 조회
    async fn get_from_memory_cache_statistics(&self, key: &str) -> Result<Option<CachedMarketStatistics>, String> {
        let memory_cache = self.memory_cache.read().await;
        if let Some(data) = memory_cache.get(key) {
            let cached: CachedMarketStatistics = serde_json::from_slice(data)
                .map_err(|e| format!("역직렬화 실패: {}", e))?;
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    /// Redis 캐시에서 통계 데이터 조회
    async fn get_from_redis_cache_statistics(&self, key: &str) -> Result<Option<CachedMarketStatistics>, String> {
        let mut redis = self.redis_client.lock().await;
        if let Some(data) = redis.get(key).await? {
            let cached: CachedMarketStatistics = serde_json::from_slice(&data)
                .map_err(|e| format!("역직렬화 실패: {}", e))?;
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    /// 메모리 캐시에 통계 데이터 저장
    async fn set_memory_cache_statistics(&self, key: &str, data: &CachedMarketStatistics) -> Result<(), String> {
        let serialized = serde_json::to_vec(data)
            .map_err(|e| format!("직렬화 실패: {}", e))?;
        
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.insert(key.to_string(), serialized);
        
        Ok(())
    }

    /// 메모리 캐시에서 전체 통계 데이터 조회
    async fn get_from_memory_cache_all_statistics(&self, key: &str) -> Result<Option<CachedAllStatistics>, String> {
        let memory_cache = self.memory_cache.read().await;
        if let Some(data) = memory_cache.get(key) {
            let cached: CachedAllStatistics = serde_json::from_slice(data)
                .map_err(|e| format!("역직렬화 실패: {}", e))?;
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    /// Redis 캐시에서 전체 통계 데이터 조회
    async fn get_from_redis_cache_all_statistics(&self, key: &str) -> Result<Option<CachedAllStatistics>, String> {
        let mut redis = self.redis_client.lock().await;
        if let Some(data) = redis.get(key).await? {
            let cached: CachedAllStatistics = serde_json::from_slice(&data)
                .map_err(|e| format!("역직렬화 실패: {}", e))?;
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    /// 메모리 캐시에 전체 통계 데이터 저장
    async fn set_memory_cache_all_statistics(&self, key: &str, data: &CachedAllStatistics) -> Result<(), String> {
        let serialized = serde_json::to_vec(data)
            .map_err(|e| format!("직렬화 실패: {}", e))?;
        
        let mut memory_cache = self.memory_cache.write().await;
        memory_cache.insert(key.to_string(), serialized);
        
        Ok(())
    }

    /// 만료된 캐시 정리
    async fn cleanup_expired_cache(
        redis_client: &Arc<Mutex<MockRedisClient>>,
        memory_cache: &Arc<RwLock<HashMap<String, Vec<u8>>>>,
        cache_stats: &Arc<RwLock<CacheStats>>,
    ) -> Result<(), String> {
        let mut redis = redis_client.lock().await;
        let keys = redis.keys("*").await?;
        
        for key in keys {
            if !redis.exists(&key).await? {
                let mut memory_cache = memory_cache.write().await;
                memory_cache.remove(&key);
            }
        }
        
        Ok(())
    }

    /// 캐시 크기 제한
    async fn limit_cache_size(
        memory_cache: &Arc<RwLock<HashMap<String, Vec<u8>>>>,
        max_size: usize,
    ) -> Result<(), String> {
        let mut cache = memory_cache.write().await;
        
        if cache.len() > max_size {
            let excess = cache.len() - max_size;
            let keys_to_remove: Vec<String> = cache.keys().take(excess).cloned().collect();
            
            for key in keys_to_remove {
                cache.remove(&key);
            }
            
            debug!("캐시 크기 제한: {}개 항목 제거", excess);
        }
        
        Ok(())
    }

    /// 캐시 통계 업데이트
    async fn update_cache_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut CacheStats),
    {
        let mut stats = self.cache_stats.write().await;
        updater(&mut *stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_key_conversion() {
        let key = CacheKey::Candlestick("BTC-KRW".to_string(), "1m".to_string());
        assert_eq!(key.to_string(), "candlestick:BTC-KRW:1m");
        
        let key = CacheKey::Statistics("BTC-KRW".to_string());
        assert_eq!(key.to_string(), "statistics:BTC-KRW");
        
        let key = CacheKey::AllStatistics;
        assert_eq!(key.to_string(), "statistics:all");
    }

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let config = CacheConfig::default();
        let manager = MDPCacheManager::new(config);
        
        let stats = manager.get_cache_stats().await;
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.sets, 0);
    }

    #[tokio::test]
    async fn test_candlestick_caching() {
        let config = CacheConfig::default();
        let manager = MDPCacheManager::new(config);
        
        let candlestick = CandlestickData {
            symbol: "BTC-KRW".to_string(),
            timeframe: "1m".to_string(),
            open: 50000000,
            high: 51000000,
            low: 49000000,
            close: 50500000,
            volume: 1000,
            timestamp: 1234567890,
            trade_count: 10,
        };
        
        // 캐시 저장
        let result = manager.cache_candlestick("BTC-KRW", "1m", &candlestick).await;
        assert!(result.is_ok());
        
        // 캐시 조회
        let cached = manager.get_candlestick("BTC-KRW", "1m").await.unwrap();
        assert!(cached.is_some());
        
        let cached_data = cached.unwrap();
        assert_eq!(cached_data.symbol, "BTC-KRW");
        assert_eq!(cached_data.timeframe, "1m");
        assert_eq!(cached_data.open, 50000000);
        assert_eq!(cached_data.close, 50500000);
    }

    #[tokio::test]
    async fn test_statistics_caching() {
        let config = CacheConfig::default();
        let manager = MDPCacheManager::new(config);
        
        let statistics = MarketStatistics {
            symbol: "BTC-KRW".to_string(),
            price_change_24h: 5.5,
            volume_24h: 1000000,
            high_24h: 52000000,
            low_24h: 48000000,
            last_price: 50500000,
            bid_price: 50400000,
            ask_price: 50600000,
            spread: 200000,
            timestamp: 1234567890,
        };
        
        // 캐시 저장
        let result = manager.cache_statistics("BTC-KRW", &statistics).await;
        assert!(result.is_ok());
        
        // 캐시 조회
        let cached = manager.get_statistics("BTC-KRW").await.unwrap();
        assert!(cached.is_some());
        
        let cached_data = cached.unwrap();
        assert_eq!(cached_data.symbol, "BTC-KRW");
        assert_eq!(cached_data.price_change_24h, 5.5);
        assert_eq!(cached_data.volume_24h, 1000000);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let config = CacheConfig::default();
        let manager = MDPCacheManager::new(config);
        
        let candlestick = CandlestickData {
            symbol: "BTC-KRW".to_string(),
            timeframe: "1m".to_string(),
            open: 50000000,
            high: 51000000,
            low: 49000000,
            close: 50500000,
            volume: 1000,
            timestamp: 1234567890,
            trade_count: 10,
        };
        
        // 캐시 저장
        manager.cache_candlestick("BTC-KRW", "1m", &candlestick).await.unwrap();
        
        // 캐시 조회 (성공)
        let cached = manager.get_candlestick("BTC-KRW", "1m").await.unwrap();
        assert!(cached.is_some());
        
        // 캐시 무효화
        let key = CacheKey::Candlestick("BTC-KRW".to_string(), "1m".to_string());
        manager.invalidate(&key).await.unwrap();
        
        // 캐시 조회 (실패)
        let cached = manager.get_candlestick("BTC-KRW", "1m").await.unwrap();
        assert!(cached.is_none());
    }
}