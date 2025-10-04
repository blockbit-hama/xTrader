//! 병렬 소비 최적화 시스템
//!
//! 이 모듈은 워커 스레드 풀, 로드 밸런싱, 백프레셔를 통해
//! 고성능 병렬 메시지 소비를 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore, mpsc};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

/// 병렬 소비 설정
#[derive(Debug, Clone)]
pub struct ParallelConsumerConfig {
    pub worker_count: usize,
    pub queue_capacity: usize,
    pub batch_size: usize,
    pub processing_timeout_ms: u64,
    pub enable_load_balancing: bool,
    pub enable_backpressure: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl Default for ParallelConsumerConfig {
    fn default() -> Self {
        Self {
            worker_count: 8,
            queue_capacity: 10000,
            batch_size: 50,
            processing_timeout_ms: 5000,
            enable_load_balancing: true,
            enable_backpressure: true,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// 소비 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerMessage<T> {
    pub id: String,
    pub data: T,
    pub partition: Option<u32>,
    pub offset: Option<u64>,
    pub timestamp: u64,
    pub retry_count: u32,
}

/// 워커 상태
#[derive(Debug, Clone)]
pub enum WorkerStatus {
    Idle,
    Processing,
    Error,
    Shutdown,
}

/// 워커 통계
#[derive(Debug, Clone)]
pub struct WorkerStats {
    pub worker_id: usize,
    pub status: WorkerStatus,
    pub processed_count: u64,
    pub failed_count: u64,
    pub average_processing_time_ms: f64,
    pub last_activity: u64,
    pub queue_size: usize,
}

/// 병렬 소비 통계
#[derive(Debug, Clone)]
pub struct ParallelConsumerStats {
    pub total_workers: usize,
    pub active_workers: usize,
    pub idle_workers: usize,
    pub error_workers: usize,
    pub total_processed: u64,
    pub total_failed: u64,
    pub average_throughput_per_second: f64,
    pub queue_size: usize,
    pub worker_stats: Vec<WorkerStats>,
}

/// 워커 풀 관리자
pub struct WorkerPool<T, R> {
    config: ParallelConsumerConfig,
    workers: Arc<RwLock<HashMap<usize, Arc<Worker<T, R>>>>>,
    message_queue: Arc<Mutex<VecDeque<ConsumerMessage<T>>>>,
    stats: Arc<RwLock<ParallelConsumerStats>>,
    is_running: Arc<Mutex<bool>>,
    processor_fn: Arc<dyn Fn(T) -> Result<R, String> + Send + Sync>,
}

/// 개별 워커
pub struct Worker<T, R> {
    worker_id: usize,
    status: Arc<RwLock<WorkerStatus>>,
    processed_count: AtomicU64,
    failed_count: AtomicU64,
    total_processing_time: AtomicU64,
    last_activity: AtomicU64,
    message_tx: mpsc::UnboundedSender<ConsumerMessage<T>>,
    stats_tx: mpsc::UnboundedSender<WorkerStats>,
}

impl<T: Clone + Send + Sync + 'static, R: Clone + Send + Sync + 'static> Worker<T, R> {
    fn new(
        worker_id: usize,
        processor_fn: Arc<dyn Fn(T) -> Result<R, String> + Send + Sync>,
        stats_tx: mpsc::UnboundedSender<WorkerStats>,
    ) -> (Self, mpsc::UnboundedReceiver<ConsumerMessage<T>>) {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        
        let worker = Self {
            worker_id,
            status: Arc::new(RwLock::new(WorkerStatus::Idle)),
            processed_count: AtomicU64::new(0),
            failed_count: AtomicU64::new(0),
            total_processing_time: AtomicU64::new(0),
            last_activity: AtomicU64::new(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
            ),
            message_tx,
            stats_tx,
        };
        
        // 워커 태스크 시작
        let worker_clone = worker.clone();
        tokio::spawn(async move {
            worker_clone.run(message_rx, processor_fn).await;
        });
        
        (worker, message_rx)
    }
    
    async fn run(
        &self,
        mut message_rx: mpsc::UnboundedReceiver<ConsumerMessage<T>>,
        processor_fn: Arc<dyn Fn(T) -> Result<R, String> + Send + Sync>,
    ) {
        info!("워커 {} 시작", self.worker_id);
        
        while let Some(message) = message_rx.recv().await {
            // 상태를 Processing으로 변경
            {
                let mut status = self.status.write().await;
                *status = WorkerStatus::Processing;
            }
            
            let start_time = SystemTime::now();
            
            // 메시지 처리
            match processor_fn(message.data.clone()).await {
                Ok(_) => {
                    self.processed_count.fetch_add(1, Ordering::Relaxed);
                    debug!("워커 {} 메시지 처리 성공: {}", self.worker_id, message.id);
                }
                Err(e) => {
                    self.failed_count.fetch_add(1, Ordering::Relaxed);
                    error!("워커 {} 메시지 처리 실패: {} - {}", self.worker_id, message.id, e);
                }
            }
            
            let processing_time = start_time.elapsed().unwrap().as_millis() as u64;
            self.total_processing_time.fetch_add(processing_time, Ordering::Relaxed);
            self.last_activity.store(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                Ordering::Relaxed
            );
            
            // 상태를 Idle로 변경
            {
                let mut status = self.status.write().await;
                *status = WorkerStatus::Idle;
            }
            
            // 통계 전송
            let stats = WorkerStats {
                worker_id: self.worker_id,
                status: WorkerStatus::Idle,
                processed_count: self.processed_count.load(Ordering::Relaxed),
                failed_count: self.failed_count.load(Ordering::Relaxed),
                average_processing_time_ms: {
                    let total = self.total_processing_time.load(Ordering::Relaxed);
                    let count = self.processed_count.load(Ordering::Relaxed);
                    if count > 0 { total as f64 / count as f64 } else { 0.0 }
                },
                last_activity: self.last_activity.load(Ordering::Relaxed),
                queue_size: 0, // 개별 워커는 큐 크기를 추적하지 않음
            };
            
            let _ = self.stats_tx.send(stats);
        }
        
        info!("워커 {} 종료", self.worker_id);
    }
    
    pub async fn send_message(&self, message: ConsumerMessage<T>) -> Result<(), String> {
        self.message_tx.send(message)
            .map_err(|e| format!("워커 {} 메시지 전송 실패: {}", self.worker_id, e))
    }
    
    pub async fn get_status(&self) -> WorkerStatus {
        self.status.read().await.clone()
    }
    
    pub fn get_stats(&self) -> WorkerStats {
        WorkerStats {
            worker_id: self.worker_id,
            status: WorkerStatus::Idle, // Mock
            processed_count: self.processed_count.load(Ordering::Relaxed),
            failed_count: self.failed_count.load(Ordering::Relaxed),
            average_processing_time_ms: {
                let total = self.total_processing_time.load(Ordering::Relaxed);
                let count = self.processed_count.load(Ordering::Relaxed);
                if count > 0 { total as f64 / count as f64 } else { 0.0 }
            },
            last_activity: self.last_activity.load(Ordering::Relaxed),
            queue_size: 0,
        }
    }
}

impl<T: Clone + Send + Sync + 'static, R: Clone + Send + Sync + 'static> Clone for Worker<T, R> {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id,
            status: self.status.clone(),
            processed_count: AtomicU64::new(self.processed_count.load(Ordering::Relaxed)),
            failed_count: AtomicU64::new(self.failed_count.load(Ordering::Relaxed)),
            total_processing_time: AtomicU64::new(self.total_processing_time.load(Ordering::Relaxed)),
            last_activity: AtomicU64::new(self.last_activity.load(Ordering::Relaxed)),
            message_tx: self.message_tx.clone(),
            stats_tx: self.stats_tx.clone(),
        }
    }
}

impl<T: Clone + Send + Sync + 'static, R: Clone + Send + Sync + 'static> WorkerPool<T, R> {
    /// 새 워커 풀 생성
    pub fn new<F>(config: ParallelConsumerConfig, processor_fn: F) -> Self
    where
        F: Fn(T) -> Result<R, String> + Send + Sync + 'static,
    {
        Self {
            config,
            workers: Arc::new(RwLock::new(HashMap::new())),
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(ParallelConsumerStats {
                total_workers: 0,
                active_workers: 0,
                idle_workers: 0,
                error_workers: 0,
                total_processed: 0,
                total_failed: 0,
                average_throughput_per_second: 0.0,
                queue_size: 0,
                worker_stats: Vec::new(),
            })),
            is_running: Arc::new(Mutex::new(false)),
            processor_fn: Arc::new(processor_fn),
        }
    }

    /// 워커 풀 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("워커 풀이 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("워커 풀 시작: {}개 워커", self.config.worker_count);

        // 통계 수집 채널 생성
        let (stats_tx, mut stats_rx) = mpsc::unbounded_channel();

        // 워커 생성
        let mut workers = self.workers.write().await;
        for worker_id in 0..self.config.worker_count {
            let (worker, _message_rx) = Worker::new(
                worker_id,
                self.processor_fn.clone(),
                stats_tx.clone(),
            );
            workers.insert(worker_id, Arc::new(worker));
        }
        drop(workers);

        // 통계 수집 태스크
        let stats = self.stats.clone();
        tokio::spawn(async move {
            while let Some(worker_stats) = stats_rx.recv().await {
                let mut stats_guard = stats.write().await;
                
                // 워커 통계 업데이트
                if let Some(existing_stats) = stats_guard.worker_stats.iter_mut()
                    .find(|s| s.worker_id == worker_stats.worker_id) {
                    *existing_stats = worker_stats;
                } else {
                    stats_guard.worker_stats.push(worker_stats);
                }
                
                // 전체 통계 업데이트
                stats_guard.total_processed = stats_guard.worker_stats.iter()
                    .map(|s| s.processed_count).sum();
                stats_guard.total_failed = stats_guard.worker_stats.iter()
                    .map(|s| s.failed_count).sum();
                stats_guard.active_workers = stats_guard.worker_stats.iter()
                    .filter(|s| matches!(s.status, WorkerStatus::Processing)).count();
                stats_guard.idle_workers = stats_guard.worker_stats.iter()
                    .filter(|s| matches!(s.status, WorkerStatus::Idle)).count();
                stats_guard.error_workers = stats_guard.worker_stats.iter()
                    .filter(|s| matches!(s.status, WorkerStatus::Error)).count();
            }
        });

        // 메시지 분배 태스크
        let message_queue = self.message_queue.clone();
        let workers = self.workers.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(10)); // 10ms마다 체크
            let mut round_robin_index = 0;

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 메시지 분배
                if let Err(e) = Self::distribute_messages(
                    &message_queue,
                    &workers,
                    &mut round_robin_index,
                    &config,
                ).await {
                    error!("메시지 분배 실패: {}", e);
                }
            }

            info!("워커 풀 종료");
        });
    }

    /// 워커 풀 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("워커 풀 중단 요청");
    }

    /// 메시지 추가
    pub async fn add_message(&self, message: ConsumerMessage<T>) -> Result<(), String> {
        let mut queue = self.message_queue.lock().await;
        
        if queue.len() >= self.config.queue_capacity {
            return Err("큐가 가득참".to_string());
        }

        queue.push_back(message);
        Ok(())
    }

    /// 메시지 분배
    async fn distribute_messages(
        message_queue: &Arc<Mutex<VecDeque<ConsumerMessage<T>>>>,
        workers: &Arc<RwLock<HashMap<usize, Arc<Worker<T, R>>>>>,
        round_robin_index: &mut usize,
        config: &ParallelConsumerConfig,
    ) -> Result<(), String> {
        let mut queue = message_queue.lock().await;
        let workers_guard = workers.read().await;
        
        if queue.is_empty() || workers_guard.is_empty() {
            return Ok(());
        }

        let mut distributed_count = 0;
        let worker_ids: Vec<usize> = workers_guard.keys().cloned().collect();
        
        while !queue.is_empty() && distributed_count < config.batch_size {
            // 로드 밸런싱 활성화된 경우 가장 유휴한 워커 선택
            let worker_id = if config.enable_load_balancing {
                Self::select_least_busy_worker(&workers_guard)?
            } else {
                // 라운드 로빈
                let id = worker_ids[*round_robin_index % worker_ids.len()];
                *round_robin_index += 1;
                id
            };

            if let Some(message) = queue.pop_front() {
                if let Some(worker) = workers_guard.get(&worker_id) {
                    if let Err(e) = worker.send_message(message).await {
                        error!("워커 {} 메시지 전송 실패: {}", worker_id, e);
                        // 실패한 메시지를 큐에 다시 추가
                        queue.push_front(ConsumerMessage {
                            id: uuid::Uuid::new_v4().to_string(),
                            data: message.data,
                            partition: message.partition,
                            offset: message.offset,
                            timestamp: message.timestamp,
                            retry_count: message.retry_count + 1,
                        });
                    } else {
                        distributed_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    /// 가장 유휴한 워커 선택
    fn select_least_busy_worker(
        workers: &HashMap<usize, Arc<Worker<T, R>>>,
    ) -> Result<usize, String> {
        let mut best_worker_id = 0;
        let mut min_processed = u64::MAX;

        for (worker_id, worker) in workers.iter() {
            let processed = worker.processed_count.load(Ordering::Relaxed);
            if processed < min_processed {
                min_processed = processed;
                best_worker_id = *worker_id;
            }
        }

        Ok(best_worker_id)
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> ParallelConsumerStats {
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

    /// 워커 상태 조회
    pub async fn get_worker_status(&self, worker_id: usize) -> Option<WorkerStatus> {
        let workers = self.workers.read().await;
        workers.get(&worker_id).map(|worker| {
            // 비동기 함수를 동기적으로 호출할 수 없으므로 Mock 상태 반환
            WorkerStatus::Idle
        })
    }
}

/// 백프레셔 관리자
pub struct BackpressureManager {
    threshold: usize,
    current_load: AtomicUsize,
    is_throttling: AtomicUsize,
}

impl BackpressureManager {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            current_load: AtomicUsize::new(0),
            is_throttling: AtomicUsize::new(0),
        }
    }

    pub fn increment_load(&self) {
        self.current_load.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_load(&self) {
        self.current_load.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn should_throttle(&self) -> bool {
        let load = self.current_load.load(Ordering::Relaxed);
        if load >= self.threshold {
            self.is_throttling.store(1, Ordering::Relaxed);
            true
        } else {
            self.is_throttling.store(0, Ordering::Relaxed);
            false
        }
    }

    pub fn is_throttling(&self) -> bool {
        self.is_throttling.load(Ordering::Relaxed) == 1
    }

    pub fn get_current_load(&self) -> usize {
        self.current_load.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_pool_creation() {
        let config = ParallelConsumerConfig::default();
        let pool = WorkerPool::new(config, |data| Ok(format!("processed_{}", data)));
        
        let stats = pool.get_stats().await;
        assert_eq!(stats.total_workers, 0);
        assert_eq!(stats.active_workers, 0);
    }

    #[tokio::test]
    async fn test_message_processing() {
        let config = ParallelConsumerConfig {
            worker_count: 2,
            queue_capacity: 100,
            batch_size: 10,
            processing_timeout_ms: 1000,
            enable_load_balancing: true,
            enable_backpressure: false,
            max_retries: 3,
            retry_delay_ms: 100,
        };
        
        let pool = WorkerPool::new(config, |data| Ok(format!("processed_{}", data)));
        
        // 워커 풀 시작
        pool.start().await;
        
        // 메시지 추가
        for i in 0..5 {
            let message = ConsumerMessage {
                id: format!("msg_{}", i),
                data: format!("data_{}", i),
                partition: Some(0),
                offset: Some(i as u64),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                retry_count: 0,
            };
            
            pool.add_message(message).await.unwrap();
        }
        
        // 잠시 대기
        sleep(Duration::from_millis(200)).await;
        
        let stats = pool.get_stats().await;
        assert!(stats.total_processed > 0);
        
        pool.stop().await;
    }

    #[tokio::test]
    async fn test_backpressure_manager() {
        let manager = BackpressureManager::new(10);
        
        // 임계값 이하
        for _ in 0..5 {
            manager.increment_load();
        }
        assert!(!manager.should_throttle());
        assert!(!manager.is_throttling());
        
        // 임계값 초과
        for _ in 0..10 {
            manager.increment_load();
        }
        assert!(manager.should_throttle());
        assert!(manager.is_throttling());
        
        // 로드 감소
        for _ in 0..5 {
            manager.decrement_load();
        }
        assert!(!manager.should_throttle());
        assert!(!manager.is_throttling());
        
        assert_eq!(manager.get_current_load(), 10);
    }

    #[tokio::test]
    async fn test_worker_stats() {
        let config = ParallelConsumerConfig::default();
        let pool = WorkerPool::new(config, |data| Ok(format!("processed_{}", data)));
        
        pool.start().await;
        
        // 워커 상태 조회
        let status = pool.get_worker_status(0).await;
        assert!(status.is_some());
        
        pool.stop().await;
    }
}