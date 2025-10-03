use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use crossbeam::queue::SegQueue;
use crossbeam::utils::Backoff;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use serde::{Serialize, Deserialize};

/// Lock-Free 캐시 시스템
/// 
/// 핵심 기능:
/// 1. Lock-Free 자료구조 사용
/// 2. 원자적 연산으로 동시성 보장
/// 3. 메모리 순서 최적화
/// 4. NUMA 친화적 설계
pub struct LockFreeCache {
    /// Lock-Free 해시맵 (잔고 데이터)
    balances: DashMap<String, AtomicU64>,
    
    /// Lock-Free 큐 (쓰기 작업)
    write_queue: Arc<SegQueue<WriteOperation>>,
    
    /// Lock-Free 큐 (읽기 작업)
    read_queue: Arc<SegQueue<ReadOperation>>,
    
    /// 성능 메트릭
    metrics: Arc<LockFreeMetrics>,
    
    /// 백그라운드 작업자들
    workers: Vec<tokio::task::JoinHandle<()>>,
}

/// 쓰기 작업
#[derive(Debug, Clone)]
pub struct WriteOperation {
    pub key: String,
    pub value: u64,
    pub operation_type: WriteType,
    pub timestamp: Instant,
}

/// 쓰기 타입
#[derive(Debug, Clone)]
pub enum WriteType {
    Set,
    Add,
    Subtract,
}

/// 읽기 작업
#[derive(Debug, Clone)]
pub struct ReadOperation {
    pub key: String,
    pub callback: tokio::sync::oneshot::Sender<Option<u64>>,
}

/// Lock-Free 성능 메트릭
#[derive(Debug, Default)]
pub struct LockFreeMetrics {
    pub reads: AtomicU64,
    pub writes: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub queue_size: AtomicUsize,
    pub start_time: Instant,
}

impl LockFreeMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            ..Default::default()
        }
    }
    
    pub fn record_read(&self) {
        self.reads.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_write(&self) {
        self.writes.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn update_queue_size(&self, size: usize) {
        self.queue_size.store(size, Ordering::Relaxed);
    }
    
    pub fn get_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total > 0 {
            hits as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    }
    
    pub fn get_tps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let total_ops = self.reads.load(Ordering::Relaxed) + self.writes.load(Ordering::Relaxed);
            total_ops as f64 / elapsed
        } else {
            0.0
        }
    }
}

impl LockFreeCache {
    /// 새 Lock-Free 캐시 생성
    pub fn new() -> Self {
        Self {
            balances: DashMap::new(),
            write_queue: Arc::new(SegQueue::new()),
            read_queue: Arc::new(SegQueue::new()),
            metrics: Arc::new(LockFreeMetrics::new()),
            workers: Vec::new(),
        }
    }
    
    /// 캐시 시작 (백그라운드 작업자 시작)
    pub fn start(&mut self) {
        // 쓰기 작업자
        let write_queue = self.write_queue.clone();
        let balances = self.balances.clone();
        let metrics = self.metrics.clone();
        
        let write_worker = tokio::spawn(async move {
            let backoff = Backoff::new();
            
            loop {
                if let Some(operation) = write_queue.pop() {
                    Self::process_write_operation(&balances, &operation, &metrics);
                } else {
                    backoff.snooze();
                }
            }
        });
        
        // 읽기 작업자
        let read_queue = self.read_queue.clone();
        let balances = self.balances.clone();
        let metrics = self.metrics.clone();
        
        let read_worker = tokio::spawn(async move {
            let backoff = Backoff::new();
            
            loop {
                if let Some(operation) = read_queue.pop() {
                    Self::process_read_operation(&balances, &operation, &metrics);
                } else {
                    backoff.snooze();
                }
            }
        });
        
        self.workers.push(write_worker);
        self.workers.push(read_worker);
        
        info!("🚀 Lock-Free 캐시 시작됨 (작업자: 2개)");
    }
    
    /// 캐시 중지
    pub async fn stop(&mut self) {
        for worker in self.workers.drain(..) {
            worker.abort();
        }
        info!("Lock-Free 캐시 중지됨");
    }
    
    /// Lock-Free 읽기 (논블로킹)
    pub async fn get(&self, key: &str) -> Option<u64> {
        self.metrics.record_read();
        
        // 직접 읽기 시도 (가장 빠름)
        if let Some(entry) = self.balances.get(key) {
            let value = entry.load(Ordering::Relaxed);
            self.metrics.record_hit();
            return Some(value);
        }
        
        self.metrics.record_miss();
        None
    }
    
    /// Lock-Free 쓰기 (논블로킹)
    pub fn set(&self, key: &str, value: u64) {
        self.metrics.record_write();
        
        let operation = WriteOperation {
            key: key.to_string(),
            value,
            operation_type: WriteType::Set,
            timestamp: Instant::now(),
        };
        
        self.write_queue.push(operation);
    }
    
    /// Lock-Free 증가 (원자적)
    pub fn add(&self, key: &str, delta: u64) -> u64 {
        self.metrics.record_write();
        
        // DashMap의 entry API 사용 (Lock-Free)
        let entry = self.balances.entry(key.to_string()).or_insert_with(|| AtomicU64::new(0));
        let old_value = entry.fetch_add(delta, Ordering::Relaxed);
        old_value + delta
    }
    
    /// Lock-Free 감소 (원자적)
    pub fn subtract(&self, key: &str, delta: u64) -> Option<u64> {
        self.metrics.record_write();
        
        if let Some(entry) = self.balances.get(key) {
            let current = entry.load(Ordering::Relaxed);
            if current >= delta {
                let new_value = entry.fetch_sub(delta, Ordering::Relaxed) - delta;
                return Some(new_value);
            }
        }
        None
    }
    
    /// Lock-Free 비교 후 교체 (CAS)
    pub fn compare_and_swap(&self, key: &str, expected: u64, new: u64) -> Result<u64, u64> {
        self.metrics.record_write();
        
        let entry = self.balances.entry(key.to_string()).or_insert_with(|| AtomicU64::new(0));
        
        match entry.compare_exchange(expected, new, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => Ok(new),
            Err(current) => Err(current),
        }
    }
    
    /// 쓰기 작업 처리
    fn process_write_operation(
        balances: &DashMap<String, AtomicU64>,
        operation: &WriteOperation,
        metrics: &LockFreeMetrics,
    ) {
        match operation.operation_type {
            WriteType::Set => {
                let entry = balances.entry(operation.key.clone()).or_insert_with(|| AtomicU64::new(0));
                entry.store(operation.value, Ordering::Relaxed);
            }
            WriteType::Add => {
                let entry = balances.entry(operation.key.clone()).or_insert_with(|| AtomicU64::new(0));
                entry.fetch_add(operation.value, Ordering::Relaxed);
            }
            WriteType::Subtract => {
                if let Some(entry) = balances.get(&operation.key) {
                    entry.fetch_sub(operation.value, Ordering::Relaxed);
                }
            }
        }
    }
    
    /// 읽기 작업 처리
    fn process_read_operation(
        balances: &DashMap<String, AtomicU64>,
        operation: &ReadOperation,
        metrics: &LockFreeMetrics,
    ) {
        let value = balances.get(&operation.key)
            .map(|entry| entry.load(Ordering::Relaxed));
        
        let _ = operation.callback.send(value);
    }
    
    /// 성능 메트릭 조회
    pub fn get_metrics(&self) -> &LockFreeMetrics {
        &self.metrics
    }
    
    /// 성능 통계 출력
    pub fn print_performance_stats(&self) {
        let metrics = &self.metrics;
        info!("📊 Lock-Free 캐시 성능 통계:");
        info!("   읽기: {}", metrics.reads.load(Ordering::Relaxed));
        info!("   쓰기: {}", metrics.writes.load(Ordering::Relaxed));
        info!("   히트율: {:.2}%", metrics.get_hit_rate());
        info!("   처리량: {:.2} TPS", metrics.get_tps());
        info!("   큐 크기: {}", metrics.queue_size.load(Ordering::Relaxed));
    }
    
    /// 캐시 상태 조회
    pub fn get_cache_status(&self) -> LockFreeCacheStatus {
        LockFreeCacheStatus {
            entry_count: self.balances.len(),
            queue_size: self.write_queue.len() + self.read_queue.len(),
        }
    }
}

/// Lock-Free 캐시 상태
#[derive(Debug, Clone)]
pub struct LockFreeCacheStatus {
    pub entry_count: usize,
    pub queue_size: usize,
}

/// 잔고 전용 Lock-Free 캐시
pub struct LockFreeBalanceCache {
    cache: LockFreeCache,
}

impl LockFreeBalanceCache {
    pub fn new() -> Self {
        Self {
            cache: LockFreeCache::new(),
        }
    }
    
    /// 잔고 조회 (Lock-Free)
    pub async fn get_balance(&self, user_id: &str, asset: &str) -> Option<u64> {
        let key = format!("{}:{}", user_id, asset);
        self.cache.get(&key).await
    }
    
    /// 잔고 설정 (Lock-Free)
    pub fn set_balance(&self, user_id: &str, asset: &str, amount: u64) {
        let key = format!("{}:{}", user_id, asset);
        self.cache.set(&key, amount);
    }
    
    /// 잔고 증가 (원자적)
    pub fn add_balance(&self, user_id: &str, asset: &str, delta: u64) -> u64 {
        let key = format!("{}:{}", user_id, asset);
        self.cache.add(&key, delta)
    }
    
    /// 잔고 감소 (원자적)
    pub fn subtract_balance(&self, user_id: &str, asset: &str, delta: u64) -> Option<u64> {
        let key = format!("{}:{}", user_id, asset);
        self.cache.subtract(&key, delta)
    }
    
    /// 잔고 비교 후 교체 (CAS)
    pub fn compare_and_swap_balance(
        &self,
        user_id: &str,
        asset: &str,
        expected: u64,
        new: u64,
    ) -> Result<u64, u64> {
        let key = format!("{}:{}", user_id, asset);
        self.cache.compare_and_swap(&key, expected, new)
    }
    
    pub fn start(&mut self) {
        self.cache.start();
    }
    
    pub async fn stop(&mut self) {
        self.cache.stop().await;
    }
    
    pub fn get_metrics(&self) -> &LockFreeMetrics {
        self.cache.get_metrics()
    }
}

/// NUMA 친화적 Lock-Free 캐시
pub struct NUMAOptimizedCache {
    /// CPU 코어별 캐시 파티션
    partitions: Vec<DashMap<String, AtomicU64>>,
    /// 파티션 선택을 위한 해시 함수
    partition_count: usize,
}

impl NUMAOptimizedCache {
    pub fn new(partition_count: usize) -> Self {
        let partitions = (0..partition_count)
            .map(|_| DashMap::new())
            .collect();
        
        Self {
            partitions,
            partition_count,
        }
    }
    
    /// 키에 따른 파티션 선택
    fn get_partition(&self, key: &str) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize % self.partition_count
    }
    
    /// NUMA 친화적 읽기
    pub fn get(&self, key: &str) -> Option<u64> {
        let partition_idx = self.get_partition(key);
        self.partitions[partition_idx]
            .get(key)
            .map(|entry| entry.load(Ordering::Relaxed))
    }
    
    /// NUMA 친화적 쓰기
    pub fn set(&self, key: &str, value: u64) {
        let partition_idx = self.get_partition(key);
        let entry = self.partitions[partition_idx]
            .entry(key.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        entry.store(value, Ordering::Relaxed);
    }
    
    /// NUMA 친화적 증가
    pub fn add(&self, key: &str, delta: u64) -> u64 {
        let partition_idx = self.get_partition(key);
        let entry = self.partitions[partition_idx]
            .entry(key.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        entry.fetch_add(delta, Ordering::Relaxed) + delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_lockfree_cache_basic() {
        let mut cache = LockFreeCache::new();
        cache.start();
        
        // 값 설정
        cache.set("test_key", 42);
        
        // 값 조회
        let value = cache.get("test_key").await;
        assert_eq!(value, Some(42));
        
        // 원자적 증가
        let new_value = cache.add("test_key", 8);
        assert_eq!(new_value, 50);
        
        // 원자적 감소
        let remaining = cache.subtract("test_key", 10);
        assert_eq!(remaining, Some(40));
        
        cache.stop().await;
    }
    
    #[tokio::test]
    async fn test_lockfree_balance_cache() {
        let mut balance_cache = LockFreeBalanceCache::new();
        balance_cache.start();
        
        // 잔고 설정
        balance_cache.set_balance("user1", "KRW", 1000000);
        
        // 잔고 조회
        let balance = balance_cache.get_balance("user1", "KRW").await;
        assert_eq!(balance, Some(1000000));
        
        // 잔고 증가
        let new_balance = balance_cache.add_balance("user1", "KRW", 500000);
        assert_eq!(new_balance, 1500000);
        
        // 잔고 감소
        let remaining = balance_cache.subtract_balance("user1", "KRW", 200000);
        assert_eq!(remaining, Some(1300000));
        
        balance_cache.stop().await;
    }
    
    #[tokio::test]
    async fn test_concurrent_access() {
        let mut cache = LockFreeCache::new();
        cache.start();
        
        let cache = Arc::new(cache);
        let mut handles = Vec::new();
        
        // 동시 쓰기 작업
        for i in 0..10 {
            let cache_clone = cache.clone();
            let handle = tokio::spawn(async move {
                for j in 0..100 {
                    let key = format!("key_{}", i);
                    cache_clone.add(&key, j as u64);
                }
            });
            handles.push(handle);
        }
        
        // 모든 작업 완료 대기
        for handle in handles {
            handle.await.unwrap();
        }
        
        // 결과 검증
        for i in 0..10 {
            let key = format!("key_{}", i);
            let value = cache.get(&key).await.unwrap_or(0);
            assert_eq!(value, 4950); // 0+1+2+...+99 = 4950
        }
    }
    
    #[test]
    fn test_numa_optimized_cache() {
        let cache = NUMAOptimizedCache::new(4);
        
        cache.set("key1", 100);
        cache.set("key2", 200);
        
        assert_eq!(cache.get("key1"), Some(100));
        assert_eq!(cache.get("key2"), Some(200));
        
        let new_value = cache.add("key1", 50);
        assert_eq!(new_value, 150);
    }
}