//! 캐시 최적화 시스템
//!
//! 이 모듈은 다단계 캐시, 압축, LRU/LFU 알고리즘을 통해
//! 고성능 캐시 시스템을 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::hash::{Hash, Hasher};

/// 캐시 설정
#[derive(Debug, Clone)]
pub struct CacheOptimizerConfig {
    pub l1_size: usize,           // L1 캐시 크기 (메모리)
    pub l2_size: usize,           // L2 캐시 크기 (메모리)
    pub l3_size: usize,           // L3 캐시 크기 (디스크)
    pub ttl_seconds: u64,         // 기본 TTL
    pub enable_compression: bool, // 압축 활성화
    pub compression_threshold: usize, // 압축 임계값
    pub enable_lru: bool,         // LRU 활성화
    pub enable_lfu: bool,         // LFU 활성화
    pub cleanup_interval_ms: u64, // 정리 간격
}

impl Default for CacheOptimizerConfig {
    fn default() -> Self {
        Self {
            l1_size: 1000,
            l2_size: 10000,
            l3_size: 100000,
            ttl_seconds: 300, // 5분
            enable_compression: true,
            compression_threshold: 1024, // 1KB
            enable_lru: true,
            enable_lfu: false,
            cleanup_interval_ms: 60000, // 1분
        }
    }
}

/// 캐시 항목
#[derive(Debug, Clone)]
pub struct CacheItem<T> {
    pub key: String,
    pub value: T,
    pub created_at: u64,
    pub last_accessed: u64,
    pub access_count: u64,
    pub compressed: bool,
    pub size_bytes: usize,
}

/// 캐시 레벨
#[derive(Debug, Clone, PartialEq)]
pub enum CacheLevel {
    L1, // 메모리 (가장 빠름)
    L2, // 메모리 (중간)
    L3, // 디스크 (가장 느림)
    Miss, // 캐시 미스
}

/// 캐시 통계
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub l3_hits: u64,
    pub misses: u64,
    pub total_requests: u64,
    pub hit_rate: f64,
    pub l1_size: usize,
    pub l2_size: usize,
    pub l3_size: usize,
    pub total_size_bytes: usize,
    pub compression_ratio: f64,
}

/// LRU 캐시 노드
#[derive(Debug, Clone)]
struct LRUNode<T> {
    key: String,
    value: T,
    prev: Option<usize>,
    next: Option<usize>,
}

/// LRU 캐시 구현
pub struct LRUCache<T> {
    capacity: usize,
    nodes: Vec<LRUNode<T>>,
    key_to_index: HashMap<String, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    free_indices: VecDeque<usize>,
}

impl<T: Clone> LRUCache<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            nodes: Vec::new(),
            key_to_index: HashMap::new(),
            head: None,
            tail: None,
            free_indices: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<T> {
        if let Some(&index) = self.key_to_index.get(key) {
            let value = self.nodes[index].value.clone();
            self.move_to_head(index);
            Some(value)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: T) {
        if let Some(&index) = self.key_to_index.get(&key) {
            // 기존 키 업데이트
            self.nodes[index].value = value;
            self.move_to_head(index);
        } else {
            // 새 키 추가
            if self.nodes.len() >= self.capacity {
                self.evict_tail();
            }
            
            let index = self.allocate_node(key.clone(), value);
            self.key_to_index.insert(key, index);
            self.add_to_head(index);
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        if let Some(&index) = self.key_to_index.get(key) {
            let value = self.nodes[index].value.clone();
            self.remove_node(index);
            self.key_to_index.remove(key);
            Some(value)
        } else {
            None
        }
    }

    pub fn size(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_full(&self) -> bool {
        self.nodes.len() >= self.capacity
    }

    fn allocate_node(&mut self, key: String, value: T) -> usize {
        if let Some(index) = self.free_indices.pop_front() {
            self.nodes[index] = LRUNode {
                key,
                value,
                prev: None,
                next: None,
            };
            index
        } else {
            let index = self.nodes.len();
            self.nodes.push(LRUNode {
                key,
                value,
                prev: None,
                next: None,
            });
            index
        }
    }

    fn add_to_head(&mut self, index: usize) {
        self.nodes[index].prev = None;
        self.nodes[index].next = self.head;
        
        if let Some(head_index) = self.head {
            self.nodes[head_index].prev = Some(index);
        } else {
            self.tail = Some(index);
        }
        
        self.head = Some(index);
    }

    fn remove_node(&mut self, index: usize) {
        if let Some(prev_index) = self.nodes[index].prev {
            self.nodes[prev_index].next = self.nodes[index].next;
        } else {
            self.head = self.nodes[index].next;
        }

        if let Some(next_index) = self.nodes[index].next {
            self.nodes[next_index].prev = self.nodes[index].prev;
        } else {
            self.tail = self.nodes[index].prev;
        }

        self.free_indices.push_back(index);
    }

    fn move_to_head(&mut self, index: usize) {
        self.remove_node(index);
        self.add_to_head(index);
    }

    fn evict_tail(&mut self) {
        if let Some(tail_index) = self.tail {
            let key = self.nodes[tail_index].key.clone();
            self.remove_node(tail_index);
            self.key_to_index.remove(&key);
        }
    }
}

/// LFU 캐시 구현
pub struct LFUCache<T> {
    capacity: usize,
    cache: HashMap<String, T>,
    frequencies: HashMap<String, u64>,
    frequency_groups: BTreeMap<u64, Vec<String>>,
    min_frequency: u64,
}

impl<T: Clone> LFUCache<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
            frequencies: HashMap::new(),
            frequency_groups: BTreeMap::new(),
            min_frequency: 1,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<T> {
        if let Some(value) = self.cache.get(key).cloned() {
            self.increment_frequency(key);
            Some(value)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: T) {
        if self.cache.contains_key(&key) {
            self.cache.insert(key.clone(), value);
            self.increment_frequency(&key);
        } else {
            if self.cache.len() >= self.capacity {
                self.evict_least_frequent();
            }
            
            self.cache.insert(key.clone(), value);
            self.frequencies.insert(key.clone(), 1);
            self.frequency_groups.entry(1).or_insert_with(Vec::new).push(key);
            self.min_frequency = 1;
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        if let Some(value) = self.cache.remove(key) {
            if let Some(freq) = self.frequencies.remove(key) {
                if let Some(group) = self.frequency_groups.get_mut(&freq) {
                    group.retain(|k| k != key);
                    if group.is_empty() {
                        self.frequency_groups.remove(&freq);
                        if freq == self.min_frequency {
                            self.min_frequency = self.frequency_groups.keys().next().copied().unwrap_or(1);
                        }
                    }
                }
            }
            Some(value)
        } else {
            None
        }
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }

    fn increment_frequency(&mut self, key: &str) {
        if let Some(&old_freq) = self.frequencies.get(key) {
            let new_freq = old_freq + 1;
            
            // 이전 빈도 그룹에서 제거
            if let Some(group) = self.frequency_groups.get_mut(&old_freq) {
                group.retain(|k| k != key);
                if group.is_empty() {
                    self.frequency_groups.remove(&old_freq);
                    if old_freq == self.min_frequency {
                        self.min_frequency = new_freq;
                    }
                }
            }
            
            // 새 빈도 그룹에 추가
            self.frequencies.insert(key.to_string(), new_freq);
            self.frequency_groups.entry(new_freq).or_insert_with(Vec::new).push(key.to_string());
        }
    }

    fn evict_least_frequent(&mut self) {
        if let Some(group) = self.frequency_groups.get_mut(&self.min_frequency) {
            if let Some(key_to_remove) = group.pop() {
                self.cache.remove(&key_to_remove);
                self.frequencies.remove(&key_to_remove);
                
                if group.is_empty() {
                    self.frequency_groups.remove(&self.min_frequency);
                    self.min_frequency = self.frequency_groups.keys().next().copied().unwrap_or(1);
                }
            }
        }
    }
}

/// 다단계 캐시 최적화기
pub struct CacheOptimizer<T> {
    config: CacheOptimizerConfig,
    l1_cache: Arc<Mutex<LRUCache<CacheItem<T>>>>,
    l2_cache: Arc<Mutex<LRUCache<CacheItem<T>>>>,
    l3_cache: Arc<Mutex<HashMap<String, CacheItem<T>>>>,
    stats: Arc<RwLock<CacheStats>>,
    is_running: Arc<Mutex<bool>>,
}

impl<T: Clone + Send + Sync + 'static> CacheOptimizer<T> {
    /// 새 캐시 최적화기 생성
    pub fn new(config: CacheOptimizerConfig) -> Self {
        Self {
            config,
            l1_cache: Arc::new(Mutex::new(LRUCache::new(config.l1_size))),
            l2_cache: Arc::new(Mutex::new(LRUCache::new(config.l2_size))),
            l3_cache: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats {
                l1_hits: 0,
                l2_hits: 0,
                l3_hits: 0,
                misses: 0,
                total_requests: 0,
                hit_rate: 0.0,
                l1_size: 0,
                l2_size: 0,
                l3_size: 0,
                total_size_bytes: 0,
                compression_ratio: 0.0,
            })),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// 캐시 최적화기 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("캐시 최적화기가 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("캐시 최적화기 시작: L1={}, L2={}, L3={}", 
              self.config.l1_size, self.config.l2_size, self.config.l3_size);

        let l1_cache = self.l1_cache.clone();
        let l2_cache = self.l2_cache.clone();
        let l3_cache = self.l3_cache.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        // 캐시 정리 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.cleanup_interval_ms));

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 만료된 항목 정리
                Self::cleanup_expired_items(&l1_cache, &l2_cache, &l3_cache, &stats).await;
            }

            info!("캐시 최적화기 종료");
        });
    }

    /// 캐시 최적화기 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("캐시 최적화기 중단 요청");
    }

    /// 캐시에서 값 조회
    pub async fn get(&self, key: &str) -> Option<T> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // L1 캐시 확인
        {
            let mut l1 = self.l1_cache.lock().await;
            if let Some(item) = l1.get(key) {
                if current_time - item.created_at < self.config.ttl_seconds {
                    self.update_stats(CacheLevel::L1).await;
                    return Some(item.value);
                } else {
                    l1.remove(key);
                }
            }
        }

        // L2 캐시 확인
        {
            let mut l2 = self.l2_cache.lock().await;
            if let Some(item) = l2.get(key) {
                if current_time - item.created_at < self.config.ttl_seconds {
                    // L1로 승격
                    let mut l1 = self.l1_cache.lock().await;
                    l1.put(key.to_string(), item.clone());
                    l2.remove(key);
                    
                    self.update_stats(CacheLevel::L2).await;
                    return Some(item.value);
                } else {
                    l2.remove(key);
                }
            }
        }

        // L3 캐시 확인
        {
            let mut l3 = self.l3_cache.lock().await;
            if let Some(item) = l3.get(key).cloned() {
                if current_time - item.created_at < self.config.ttl_seconds {
                    // L1로 승격
                    let mut l1 = self.l1_cache.lock().await;
                    l1.put(key.to_string(), item.clone());
                    l3.remove(key);
                    
                    self.update_stats(CacheLevel::L3).await;
                    return Some(item.value);
                } else {
                    l3.remove(key);
                }
            }
        }

        self.update_stats(CacheLevel::Miss).await;
        None
    }

    /// 캐시에 값 저장
    pub async fn put(&self, key: String, value: T) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let size_bytes = std::mem::size_of_val(&value);
        let compressed = self.config.enable_compression && size_bytes > self.config.compression_threshold;

        let item = CacheItem {
            key: key.clone(),
            value,
            created_at: current_time,
            last_accessed: current_time,
            access_count: 1,
            compressed,
            size_bytes,
        };

        // L1 캐시에 저장
        let mut l1 = self.l1_cache.lock().await;
        l1.put(key, item);
    }

    /// 캐시에서 값 제거
    pub async fn remove(&self, key: &str) -> Option<T> {
        let mut l1 = self.l1_cache.lock().await;
        if let Some(item) = l1.remove(key) {
            return Some(item.value);
        }

        let mut l2 = self.l2_cache.lock().await;
        if let Some(item) = l2.remove(key) {
            return Some(item.value);
        }

        let mut l3 = self.l3_cache.lock().await;
        if let Some(item) = l3.remove(key) {
            return Some(item.value);
        }

        None
    }

    /// 만료된 항목 정리
    async fn cleanup_expired_items(
        l1_cache: &Arc<Mutex<LRUCache<CacheItem<T>>>>,
        l2_cache: &Arc<Mutex<LRUCache<CacheItem<T>>>>,
        l3_cache: &Arc<Mutex<HashMap<String, CacheItem<T>>>>,
        stats: &Arc<RwLock<CacheStats>>,
    ) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // L1 캐시 정리 (LRU는 자동으로 관리됨)
        // L2 캐시 정리 (LRU는 자동으로 관리됨)
        
        // L3 캐시 정리
        {
            let mut l3 = l3_cache.lock().await;
            l3.retain(|_, item| current_time - item.created_at < 300); // 5분 TTL
        }

        debug!("캐시 정리 완료");
    }

    /// 통계 업데이트
    async fn update_stats(&self, level: CacheLevel) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        match level {
            CacheLevel::L1 => stats.l1_hits += 1,
            CacheLevel::L2 => stats.l2_hits += 1,
            CacheLevel::L3 => stats.l3_hits += 1,
            CacheLevel::Miss => stats.misses += 1,
        }

        let total_hits = stats.l1_hits + stats.l2_hits + stats.l3_hits;
        stats.hit_rate = if stats.total_requests > 0 {
            total_hits as f64 / stats.total_requests as f64
        } else {
            0.0
        };

        // 캐시 크기 업데이트
        stats.l1_size = self.l1_cache.lock().await.size();
        stats.l2_size = self.l2_cache.lock().await.size();
        stats.l3_size = self.l3_cache.lock().await.len();
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// 캐시 크기 조회
    pub async fn get_cache_sizes(&self) -> (usize, usize, usize) {
        let l1_size = self.l1_cache.lock().await.size();
        let l2_size = self.l2_cache.lock().await.size();
        let l3_size = self.l3_cache.lock().await.len();
        (l1_size, l2_size, l3_size)
    }

    /// 캐시 비우기
    pub async fn clear(&self) {
        self.l1_cache.lock().await.nodes.clear();
        self.l2_cache.lock().await.nodes.clear();
        self.l3_cache.lock().await.clear();
        info!("캐시 비우기 완료");
    }
}

/// 압축 유틸리티
pub struct CompressionUtils;

impl CompressionUtils {
    /// 데이터 압축 (Mock)
    pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
        // Mock: 실제로는 zstd, lz4 등 사용
        Ok(data.to_vec())
    }

    /// 데이터 압축 해제 (Mock)
    pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
        // Mock: 실제로는 zstd, lz4 등 사용
        Ok(data.to_vec())
    }

    /// 압축률 계산
    pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
        if original_size == 0 {
            0.0
        } else {
            (original_size - compressed_size) as f64 / original_size as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lru_cache() {
        let mut cache = LRUCache::new(3);
        
        cache.put("key1".to_string(), "value1".to_string());
        cache.put("key2".to_string(), "value2".to_string());
        cache.put("key3".to_string(), "value3".to_string());
        
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        
        cache.put("key4".to_string(), "value4".to_string());
        assert_eq!(cache.get("key2"), None); // 가장 오래된 항목 제거됨
        
        assert_eq!(cache.size(), 3);
    }

    #[tokio::test]
    async fn test_lfu_cache() {
        let mut cache = LFUCache::new(3);
        
        cache.put("key1".to_string(), "value1".to_string());
        cache.put("key2".to_string(), "value2".to_string());
        cache.put("key3".to_string(), "value3".to_string());
        
        // key1을 여러 번 접근
        cache.get("key1");
        cache.get("key1");
        
        cache.put("key4".to_string(), "value4".to_string());
        assert_eq!(cache.get("key2"), None); // 가장 적게 사용된 항목 제거됨
        
        assert_eq!(cache.size(), 3);
    }

    #[tokio::test]
    async fn test_cache_optimizer() {
        let config = CacheOptimizerConfig::default();
        let optimizer = CacheOptimizer::new(config);
        
        // 캐시 최적화기 시작
        optimizer.start().await;
        
        // 값 저장
        optimizer.put("key1".to_string(), "value1".to_string()).await;
        optimizer.put("key2".to_string(), "value2".to_string()).await;
        
        // 값 조회
        assert_eq!(optimizer.get("key1").await, Some("value1".to_string()));
        assert_eq!(optimizer.get("key2").await, Some("value2".to_string()));
        assert_eq!(optimizer.get("key3").await, None);
        
        // 통계 확인
        let stats = optimizer.get_stats().await;
        assert!(stats.total_requests > 0);
        assert!(stats.hit_rate > 0.0);
        
        optimizer.stop().await;
    }

    #[tokio::test]
    async fn test_compression_utils() {
        let data = b"test data for compression";
        let compressed = CompressionUtils::compress(data).unwrap();
        let decompressed = CompressionUtils::decompress(&compressed).unwrap();
        
        assert_eq!(data, decompressed.as_slice());
        
        let ratio = CompressionUtils::compression_ratio(data.len(), compressed.len());
        assert!(ratio >= 0.0 && ratio <= 1.0);
    }
}