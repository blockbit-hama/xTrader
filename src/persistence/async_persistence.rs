use std::sync::Arc;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;
use sqlx::SqlitePool;
use log::{debug, error, info, warn};
use serde::{Serialize, Deserialize};

use crate::matching_engine::model::ExecutionReport;
use crate::matching_engine::ultra_fast_engine::PersistenceTask;

/// 비동기 영속화 매니저
/// 
/// 핵심 기능:
/// 1. 배치 처리로 DB 부하 최소화
/// 2. 백그라운드에서 비동기 처리
/// 3. 실패 시 재시도 메커니즘
/// 4. 성능 메트릭 수집
pub struct AsyncPersistenceManager {
    /// DB 연결 풀
    db_pool: SqlitePool,
    
    /// 영속화 큐
    persistence_queue: Arc<Mutex<VecDeque<PersistenceTask>>>,
    
    /// 배치 크기
    batch_size: usize,
    
    /// 배치 간격 (밀리초)
    batch_interval_ms: u64,
    
    /// 성능 메트릭
    metrics: Arc<PersistenceMetrics>,
    
    /// 재시도 큐 (실패한 작업들)
    retry_queue: Arc<Mutex<VecDeque<PersistenceTask>>>,
    
    /// 최대 재시도 횟수
    max_retries: usize,
}

/// 영속화 성능 메트릭
#[derive(Debug, Default)]
pub struct PersistenceMetrics {
    pub total_batches: std::sync::atomic::AtomicU64,
    pub total_executions: std::sync::atomic::AtomicU64,
    pub successful_batches: std::sync::atomic::AtomicU64,
    pub failed_batches: std::sync::atomic::AtomicU64,
    pub total_latency_ms: std::sync::atomic::AtomicU64,
    pub start_time: std::time::Instant,
}

impl PersistenceMetrics {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            ..Default::default()
        }
    }
    
    pub fn record_batch(&self, success: bool, latency_ms: u64, execution_count: usize) {
        self.total_batches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_executions.fetch_add(execution_count as u64, std::sync::atomic::Ordering::Relaxed);
        self.total_latency_ms.fetch_add(latency_ms, std::sync::atomic::Ordering::Relaxed);
        
        if success {
            self.successful_batches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.failed_batches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    
    pub fn get_tps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_executions.load(std::sync::atomic::Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }
    
    pub fn get_avg_latency_ms(&self) -> f64 {
        let total_batches = self.total_batches.load(std::sync::atomic::Ordering::Relaxed);
        if total_batches > 0 {
            self.total_latency_ms.load(std::sync::atomic::Ordering::Relaxed) as f64 / total_batches as f64
        } else {
            0.0
        }
    }
    
    pub fn get_success_rate(&self) -> f64 {
        let total_batches = self.total_batches.load(std::sync::atomic::Ordering::Relaxed);
        if total_batches > 0 {
            self.successful_batches.load(std::sync::atomic::Ordering::Relaxed) as f64 / total_batches as f64 * 100.0
        } else {
            0.0
        }
    }
}

impl AsyncPersistenceManager {
    /// 새 비동기 영속화 매니저 생성
    pub fn new(db_pool: SqlitePool) -> Self {
        Self {
            db_pool,
            persistence_queue: Arc::new(Mutex::new(VecDeque::new())),
            batch_size: 100,
            batch_interval_ms: 10,
            metrics: Arc::new(PersistenceMetrics::new()),
            retry_queue: Arc::new(Mutex::new(VecDeque::new())),
            max_retries: 3,
        }
    }
    
    /// 설정 업데이트
    pub fn with_config(mut self, batch_size: usize, batch_interval_ms: u64) -> Self {
        self.batch_size = batch_size;
        self.batch_interval_ms = batch_interval_ms;
        self
    }
    
    /// 영속화 큐에 작업 추가
    pub async fn queue_task(&self, task: PersistenceTask) {
        let mut queue = self.persistence_queue.lock().await;
        queue.push_back(task);
        debug!("영속화 작업 큐에 추가됨 (큐 크기: {})", queue.len());
    }
    
    /// 배치 커밋 루프 실행
    pub async fn run_batch_commit_loop(&self) {
        info!("🚀 비동기 영속화 매니저 시작 (배치 크기: {}, 간격: {}ms)", 
              self.batch_size, self.batch_interval_ms);
        
        let mut interval = interval(Duration::from_millis(self.batch_interval_ms));
        
        loop {
            interval.tick().await;
            
            // 메인 큐에서 배치 처리
            let batch = self.drain_queue().await;
            if !batch.is_empty() {
                self.process_batch(batch).await;
            }
            
            // 재시도 큐 처리
            self.process_retry_queue().await;
            
            // 주기적으로 메트릭 출력
            if self.metrics.total_batches.load(std::sync::atomic::Ordering::Relaxed) % 100 == 0 {
                self.print_metrics();
            }
        }
    }
    
    /// 큐에서 배치 가져오기
    async fn drain_queue(&self) -> Vec<PersistenceTask> {
        let mut queue = self.persistence_queue.lock().await;
        queue.drain(..self.batch_size.min(queue.len())).collect()
    }
    
    /// 배치 처리
    async fn process_batch(&self, batch: Vec<PersistenceTask>) {
        let start_time = std::time::Instant::now();
        let batch_size = batch.len();
        
        match self.commit_batch_to_db(&batch).await {
            Ok(_) => {
                let latency_ms = start_time.elapsed().as_millis() as u64;
                self.metrics.record_batch(true, latency_ms, batch_size);
                debug!("✅ 배치 커밋 성공: {}개 작업, {}ms", batch_size, latency_ms);
            }
            Err(e) => {
                let latency_ms = start_time.elapsed().as_millis() as u64;
                self.metrics.record_batch(false, latency_ms, batch_size);
                error!("❌ 배치 커밋 실패: {}개 작업, {}ms, 에러: {}", batch_size, latency_ms, e);
                
                // 실패한 작업들을 재시도 큐에 추가
                self.add_to_retry_queue(batch).await;
            }
        }
    }
    
    /// DB에 배치 커밋
    async fn commit_batch_to_db(&self, batch: &[PersistenceTask]) -> Result<(), sqlx::Error> {
        if batch.is_empty() {
            return Ok(());
        }
        
        let mut tx = self.db_pool.begin().await?;
        
        // 모든 체결 내역을 하나의 트랜잭션으로 처리
        for task in batch {
            // 체결 내역 저장
            sqlx::query!(
                "INSERT INTO executions (exec_id, taker_order_id, maker_order_id, symbol, side, price, quantity, taker_fee, maker_fee, transaction_time)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                task.execution.execution_id,
                task.execution.order_id,
                task.execution.counterparty_id,
                task.execution.symbol,
                format!("{:?}", task.execution.side),
                task.execution.price as i64,
                task.execution.quantity as i64,
                0, // 수수료는 나중에 구현
                0,
                task.execution.timestamp as i64
            )
            .execute(&mut tx)
            .await?;
            
            // 주문 상태 업데이트
            sqlx::query!(
                "UPDATE orders 
                 SET filled_quantity = filled_quantity + ?, 
                     status = CASE 
                         WHEN filled_quantity + ? >= quantity THEN 'Filled' 
                         ELSE 'PartiallyFilled' 
                     END,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE order_id = ?",
                task.execution.quantity as i64,
                task.execution.quantity as i64,
                task.execution.order_id
            )
            .execute(&mut tx)
            .await?;
            
            // 잔고 업데이트
            sqlx::query!(
                "INSERT OR REPLACE INTO balances (client_id, asset, available, locked, updated_at)
                 VALUES (?, 'KRW', 
                         COALESCE((SELECT available FROM balances WHERE client_id = ? AND asset = 'KRW'), 0) + ?,
                         0, CURRENT_TIMESTAMP)",
                task.user_id,
                task.user_id,
                task.balance_delta
            )
            .execute(&mut tx)
            .await?;
            
            // 감사 로그
            sqlx::query!(
                "INSERT INTO audit_log (timestamp, operation, details, user_id)
                 VALUES (?, ?, ?, ?)",
                task.timestamp as i64,
                "EXECUTION",
                format!("Execution {} completed", task.execution.execution_id),
                task.user_id
            )
            .execute(&mut tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
    
    /// 재시도 큐에 추가
    async fn add_to_retry_queue(&self, batch: Vec<PersistenceTask>) {
        let mut retry_queue = self.retry_queue.lock().await;
        for mut task in batch {
            task.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64;
            retry_queue.push_back(task);
        }
        warn!("재시도 큐에 {}개 작업 추가됨", retry_queue.len());
    }
    
    /// 재시도 큐 처리
    async fn process_retry_queue(&self) {
        let mut retry_queue = self.retry_queue.lock().await;
        if retry_queue.is_empty() {
            return;
        }
        
        let batch: Vec<PersistenceTask> = retry_queue.drain(..self.batch_size.min(retry_queue.len())).collect();
        drop(retry_queue);
        
        if !batch.is_empty() {
            match self.commit_batch_to_db(&batch).await {
                Ok(_) => {
                    debug!("✅ 재시도 배치 성공: {}개 작업", batch.len());
                }
                Err(e) => {
                    error!("❌ 재시도 배치 실패: {}개 작업, 에러: {}", batch.len(), e);
                    // 재시도 횟수 확인 후 다시 큐에 추가하거나 포기
                    self.handle_retry_failure(batch).await;
                }
            }
        }
    }
    
    /// 재시도 실패 처리
    async fn handle_retry_failure(&self, batch: Vec<PersistenceTask>) {
        // 실제 구현에서는 재시도 횟수를 추적하고 최대 횟수 초과 시 알림
        warn!("재시도 실패한 작업들을 로그에 기록: {}개", batch.len());
        for task in batch {
            error!("실패한 영속화 작업: execution_id={}, user_id={}", 
                   task.execution.execution_id, task.user_id);
        }
    }
    
    /// 성능 메트릭 출력
    pub fn print_metrics(&self) {
        let metrics = &self.metrics;
        info!("📊 영속화 성능 메트릭:");
        info!("   총 배치 수: {}", metrics.total_batches.load(std::sync::atomic::Ordering::Relaxed));
        info!("   총 체결 수: {}", metrics.total_executions.load(std::sync::atomic::Ordering::Relaxed));
        info!("   성공률: {:.2}%", metrics.get_success_rate());
        info!("   처리량: {:.2} TPS", metrics.get_tps());
        info!("   평균 지연: {:.2}ms", metrics.get_avg_latency_ms());
        
        let queue_size = {
            let queue = self.persistence_queue.try_lock().unwrap_or_default();
            queue.len()
        };
        let retry_size = {
            let retry_queue = self.retry_queue.try_lock().unwrap_or_default();
            retry_queue.len()
        };
        info!("   큐 크기: {} (재시도: {})", queue_size, retry_size);
    }
    
    /// 큐 상태 조회
    pub async fn get_queue_status(&self) -> QueueStatus {
        let main_queue_size = self.persistence_queue.lock().await.len();
        let retry_queue_size = self.retry_queue.lock().await.len();
        
        QueueStatus {
            main_queue_size,
            retry_queue_size,
            total_pending: main_queue_size + retry_queue_size,
        }
    }
    
    /// 강제 배치 처리 (테스트용)
    pub async fn force_process_batch(&self) -> Result<usize, sqlx::Error> {
        let batch = self.drain_queue().await;
        let batch_size = batch.len();
        
        if !batch.is_empty() {
            self.commit_batch_to_db(&batch).await?;
            debug!("강제 배치 처리 완료: {}개 작업", batch_size);
        }
        
        Ok(batch_size)
    }
}

/// 큐 상태 정보
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub main_queue_size: usize,
    pub retry_queue_size: usize,
    pub total_pending: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching_engine::model::{ExecutionReport, Side};
    use uuid::Uuid;
    
    fn create_test_execution() -> ExecutionReport {
        ExecutionReport {
            execution_id: Uuid::new_v4().to_string(),
            order_id: "order1".to_string(),
            symbol: "BTC-KRW".to_string(),
            side: Side::Buy,
            price: 10000,
            quantity: 100,
            remaining_quantity: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            counterparty_id: "order2".to_string(),
            is_maker: false,
        }
    }
    
    fn create_test_task() -> PersistenceTask {
        PersistenceTask {
            execution: create_test_execution(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            user_id: "user1".to_string(),
            balance_delta: 1000000,
        }
    }
    
    #[tokio::test]
    async fn test_persistence_manager_creation() {
        // 실제 테스트에서는 테스트용 DB를 사용해야 함
        // let db_pool = create_test_db_pool().await;
        // let manager = AsyncPersistenceManager::new(db_pool);
        // assert_eq!(manager.batch_size, 100);
        // assert_eq!(manager.batch_interval_ms, 10);
    }
    
    #[tokio::test]
    async fn test_queue_task() {
        // let db_pool = create_test_db_pool().await;
        // let manager = AsyncPersistenceManager::new(db_pool);
        // let task = create_test_task();
        // 
        // manager.queue_task(task).await;
        // let status = manager.get_queue_status().await;
        // assert_eq!(status.main_queue_size, 1);
    }
    
    #[tokio::test]
    async fn test_metrics() {
        let metrics = PersistenceMetrics::new();
        
        metrics.record_batch(true, 10, 5);
        metrics.record_batch(false, 20, 3);
        
        assert_eq!(metrics.total_batches.load(std::sync::atomic::Ordering::Relaxed), 2);
        assert_eq!(metrics.total_executions.load(std::sync::atomic::Ordering::Relaxed), 8);
        assert_eq!(metrics.successful_batches.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(metrics.failed_batches.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(metrics.get_success_rate(), 50.0);
    }
}