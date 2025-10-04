//! 배치 처리 최적화 시스템
//!
//! 이 모듈은 메시지 배치 처리, 병렬 처리, 메모리 풀 관리를 통해
//! 고성능 메시지 처리를 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval, timeout};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 배치 처리 설정
#[derive(Debug, Clone)]
pub struct BatchProcessorConfig {
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub max_workers: usize,
    pub queue_capacity: usize,
    pub enable_compression: bool,
    pub enable_parallel_processing: bool,
    pub memory_pool_size: usize,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_timeout_ms: 100, // 100ms
            max_workers: 10,
            queue_capacity: 10000,
            enable_compression: true,
            enable_parallel_processing: true,
            memory_pool_size: 1000,
        }
    }
}

/// 배치 처리 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMessage<T> {
    pub id: String,
    pub data: T,
    pub priority: u8,
    pub timestamp: u64,
    pub retry_count: u32,
}

/// 배치 처리 결과
#[derive(Debug, Clone)]
pub struct BatchResult<T> {
    pub batch_id: String,
    pub processed_count: usize,
    pub failed_count: usize,
    pub processing_time_ms: u64,
    pub results: Vec<Result<T, String>>,
    pub timestamp: u64,
}

/// 배치 처리 통계
#[derive(Debug, Clone)]
pub struct BatchStats {
    pub total_batches: u64,
    pub total_messages: u64,
    pub total_failed: u64,
    pub average_batch_size: f64,
    pub average_processing_time_ms: f64,
    pub throughput_per_second: f64,
    pub active_workers: usize,
    pub queue_size: usize,
}

/// 메모리 풀 관리자
pub struct MemoryPool<T> {
    pool: Arc<Mutex<VecDeque<T>>>,
    max_size: usize,
    created_count: AtomicUsize,
    reused_count: AtomicUsize,
}

impl<T: Default> MemoryPool<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
            created_count: AtomicUsize::new(0),
            reused_count: AtomicUsize::new(0),
        }
    }

    pub async fn get(&self) -> T {
        let mut pool = self.pool.lock().await;
        if let Some(item) = pool.pop_front() {
            self.reused_count.fetch_add(1, Ordering::Relaxed);
            item
        } else {
            self.created_count.fetch_add(1, Ordering::Relaxed);
            T::default()
        }
    }

    pub async fn put(&self, item: T) {
        let mut pool = self.pool.lock().await;
        if pool.len() < self.max_size {
            pool.push_back(item);
        }
    }

    pub fn get_stats(&self) -> (usize, usize) {
        (
            self.created_count.load(Ordering::Relaxed),
            self.reused_count.load(Ordering::Relaxed),
        )
    }
}

/// 배치 처리기
pub struct BatchProcessor<T, R> {
    config: BatchProcessorConfig,
    message_queue: Arc<Mutex<VecDeque<BatchMessage<T>>>>,
    worker_semaphore: Arc<Semaphore>,
    memory_pool: Arc<MemoryPool<Vec<T>>>,
    stats: Arc<RwLock<BatchStats>>,
    is_running: Arc<Mutex<bool>>,
    processor_fn: Arc<dyn Fn(Vec<T>) -> Vec<Result<R, String>> + Send + Sync>,
}

impl<T: Clone + Send + Sync + 'static, R: Clone + Send + Sync + 'static> BatchProcessor<T, R> {
    /// 새 배치 처리기 생성
    pub fn new<F>(config: BatchProcessorConfig, processor_fn: F) -> Self
    where
        F: Fn(Vec<T>) -> Vec<Result<R, String>> + Send + Sync + 'static,
    {
        Self {
            config,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            worker_semaphore: Arc::new(Semaphore::new(config.max_workers)),
            memory_pool: Arc::new(MemoryPool::new(config.memory_pool_size)),
            stats: Arc::new(RwLock::new(BatchStats {
                total_batches: 0,
                total_messages: 0,
                total_failed: 0,
                average_batch_size: 0.0,
                average_processing_time_ms: 0.0,
                throughput_per_second: 0.0,
                active_workers: 0,
                queue_size: 0,
            })),
            is_running: Arc::new(Mutex::new(false)),
            processor_fn: Arc::new(processor_fn),
        }
    }

    /// 배치 처리기 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("배치 처리기가 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("배치 처리기 시작: 배치크기={}, 워커={}개", 
              self.config.batch_size, self.config.max_workers);

        let message_queue = self.message_queue.clone();
        let worker_semaphore = self.worker_semaphore.clone();
        let memory_pool = self.memory_pool.clone();
        let stats = self.stats.clone();
        let processor_fn = self.processor_fn.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        // 배치 처리 태스크
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

                // 배치 처리 시도
                if let Err(e) = Self::process_batch(
                    &message_queue,
                    &worker_semaphore,
                    &memory_pool,
                    &stats,
                    &processor_fn,
                    &config,
                ).await {
                    error!("배치 처리 실패: {}", e);
                }
            }

            info!("배치 처리기 종료");
        });
    }

    /// 배치 처리기 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("배치 처리기 중단 요청");
    }

    /// 메시지 추가
    pub async fn add_message(&self, message: BatchMessage<T>) -> Result<(), String> {
        let mut queue = self.message_queue.lock().await;
        
        if queue.len() >= self.config.queue_capacity {
            return Err("큐가 가득참".to_string());
        }

        queue.push_back(message);
        
        // 우선순위 기준으로 정렬
        queue.make_contiguous().sort_by(|a, b| b.priority.cmp(&a.priority));
        
        Ok(())
    }

    /// 배치 처리 실행
    async fn process_batch(
        message_queue: &Arc<Mutex<VecDeque<BatchMessage<T>>>>,
        worker_semaphore: &Arc<Semaphore>,
        memory_pool: &Arc<MemoryPool<Vec<T>>>,
        stats: &Arc<RwLock<BatchStats>>,
        processor_fn: &Arc<dyn Fn(Vec<T>) -> Vec<Result<R, String>> + Send + Sync>,
        config: &BatchProcessorConfig,
    ) -> Result<(), String> {
        // 배치 메시지 수집
        let batch_messages = {
            let mut queue = message_queue.lock().await;
            let mut batch = Vec::new();
            
            for _ in 0..config.batch_size {
                if let Some(message) = queue.pop_front() {
                    batch.push(message);
                } else {
                    break;
                }
            }
            
            batch
        };

        if batch_messages.is_empty() {
            return Ok(());
        }

        // 워커 세마포어 획득
        let _permit = worker_semaphore.acquire().await
            .map_err(|e| format!("워커 세마포어 획득 실패: {}", e))?;

        // 메모리 풀에서 벡터 가져오기
        let mut batch_data = memory_pool.get().await;
        batch_data.clear();

        // 배치 데이터 준비
        for message in &batch_messages {
            batch_data.push(message.data.clone());
        }

        let batch_id = uuid::Uuid::new_v4().to_string();
        let start_time = SystemTime::now();

        // 병렬 처리 활성화된 경우 별도 태스크에서 처리
        let result = if config.enable_parallel_processing {
            let processor_fn = processor_fn.clone();
            let batch_data = batch_data.clone();
            
            tokio::task::spawn_blocking(move || {
                processor_fn(batch_data)
            }).await.map_err(|e| format!("병렬 처리 실패: {}", e))?
        } else {
            processor_fn(batch_data)
        };

        let processing_time = start_time.elapsed().unwrap().as_millis() as u64;

        // 메모리 풀에 벡터 반환
        memory_pool.put(batch_data).await;

        // 통계 업데이트
        Self::update_stats(stats, &batch_messages, &result, processing_time).await;

        debug!("배치 처리 완료: {}개 메시지, {}ms", 
               batch_messages.len(), processing_time);

        Ok(())
    }

    /// 통계 업데이트
    async fn update_stats(
        stats: &Arc<RwLock<BatchStats>>,
        batch_messages: &[BatchMessage<T>],
        results: &[Result<R, String>],
        processing_time_ms: u64,
    ) {
        let mut stats_guard = stats.write().await;
        
        stats_guard.total_batches += 1;
        stats_guard.total_messages += batch_messages.len() as u64;
        
        let failed_count = results.iter().filter(|r| r.is_err()).count();
        stats_guard.total_failed += failed_count as u64;
        
        // 평균 배치 크기 업데이트
        stats_guard.average_batch_size = 
            (stats_guard.average_batch_size * (stats_guard.total_batches - 1) as f64 + 
             batch_messages.len() as f64) / stats_guard.total_batches as f64;
        
        // 평균 처리 시간 업데이트
        stats_guard.average_processing_time_ms = 
            (stats_guard.average_processing_time_ms * (stats_guard.total_batches - 1) as f64 + 
             processing_time_ms as f64) / stats_guard.total_batches as f64;
        
        // 처리량 계산 (초당 메시지 수)
        if processing_time_ms > 0 {
            let throughput = (batch_messages.len() as f64 * 1000.0) / processing_time_ms as f64;
            stats_guard.throughput_per_second = 
                (stats_guard.throughput_per_second * 0.9) + (throughput * 0.1); // 지수 이동 평균
        }
        
        // 활성 워커 수 업데이트
        stats_guard.active_workers = stats_guard.total_batches as usize % 10; // Mock
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> BatchStats {
        let mut stats = self.stats.read().await.clone();
        let queue = self.message_queue.lock().await;
        stats.queue_size = queue.len();
        stats
    }

    /// 큐 크기 조회
    pub async fn queue_size(&self) -> usize {
        let queue = self.message_queue.lock().await;
        queue.len()
    }

    /// 메모리 풀 통계 조회
    pub fn get_memory_pool_stats(&self) -> (usize, usize) {
        self.memory_pool.get_stats()
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

/// 성능 모니터링 유틸리티
pub struct PerformanceMonitor {
    start_time: SystemTime,
    operation_counts: AtomicUsize,
    total_time_ms: AtomicUsize,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: SystemTime::now(),
            operation_counts: AtomicUsize::new(0),
            total_time_ms: AtomicUsize::new(0),
        }
    }

    pub fn record_operation(&self, time_ms: u64) {
        self.operation_counts.fetch_add(1, Ordering::Relaxed);
        self.total_time_ms.fetch_add(time_ms as usize, Ordering::Relaxed);
    }

    pub fn get_metrics(&self) -> (usize, f64, f64) {
        let operations = self.operation_counts.load(Ordering::Relaxed);
        let total_time = self.total_time_ms.load(Ordering::Relaxed);
        let uptime_ms = self.start_time.elapsed().unwrap().as_millis() as u64;
        
        let avg_time_ms = if operations > 0 {
            total_time as f64 / operations as f64
        } else {
            0.0
        };
        
        let ops_per_second = if uptime_ms > 0 {
            (operations as f64 * 1000.0) / uptime_ms as f64
        } else {
            0.0
        };
        
        (operations, avg_time_ms, ops_per_second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_processor_creation() {
        let config = BatchProcessorConfig::default();
        let processor = BatchProcessor::new(config, |data| {
            data.into_iter().map(|x| Ok(format!("processed_{}", x))).collect()
        });
        
        let stats = processor.get_stats().await;
        assert_eq!(stats.total_batches, 0);
        assert_eq!(stats.total_messages, 0);
    }

    #[tokio::test]
    async fn test_memory_pool() {
        let pool = MemoryPool::<Vec<String>>::new(10);
        
        // 풀에서 가져오기
        let mut vec1 = pool.get().await;
        vec1.push("test1".to_string());
        pool.put(vec1).await;
        
        // 다시 가져오기 (재사용)
        let vec2 = pool.get().await;
        assert_eq!(vec2.len(), 1);
        assert_eq!(vec2[0], "test1");
        
        let (created, reused) = pool.get_stats();
        assert!(created > 0);
        assert!(reused > 0);
    }

    #[tokio::test]
    async fn test_message_processing() {
        let config = BatchProcessorConfig {
            batch_size: 5,
            batch_timeout_ms: 50,
            max_workers: 2,
            queue_capacity: 100,
            enable_compression: false,
            enable_parallel_processing: false,
            memory_pool_size: 10,
        };
        
        let processor = BatchProcessor::new(config, |data| {
            data.into_iter().map(|x| Ok(format!("processed_{}", x))).collect()
        });
        
        // 메시지 추가
        for i in 0..10 {
            let message = BatchMessage {
                id: format!("msg_{}", i),
                data: format!("data_{}", i),
                priority: 5,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                retry_count: 0,
            };
            
            processor.add_message(message).await.unwrap();
        }
        
        // 배치 처리기 시작
        processor.start().await;
        
        // 잠시 대기
        sleep(Duration::from_millis(200)).await;
        
        let stats = processor.get_stats().await;
        assert!(stats.total_batches > 0);
        assert!(stats.total_messages > 0);
        
        processor.stop().await;
    }

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new();
        
        monitor.record_operation(100);
        monitor.record_operation(200);
        monitor.record_operation(150);
        
        let (operations, avg_time, ops_per_second) = monitor.get_metrics();
        assert_eq!(operations, 3);
        assert_eq!(avg_time, 150.0);
        assert!(ops_per_second > 0.0);
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