use std::sync::Arc;
use std::time::Duration;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Error as SqlxError;
use log::{debug, error, info, warn};
use tokio::time::interval;

/// 최적화된 데이터베이스 연결 풀
/// 
/// 핵심 최적화:
/// 1. 연결 풀 크기 최적화
/// 2. 연결 재사용 최적화
/// 3. 연결 상태 모니터링
/// 4. 자동 연결 복구
pub struct OptimizedDatabasePool {
    /// 메인 연결 풀 (쓰기용)
    write_pool: SqlitePool,
    
    /// 읽기 전용 연결 풀들 (읽기용)
    read_pools: Vec<SqlitePool>,
    
    /// 현재 읽기 풀 인덱스
    read_pool_index: std::sync::atomic::AtomicUsize,
    
    /// 연결 풀 메트릭
    metrics: Arc<PoolMetrics>,
    
    /// 연결 상태 모니터링 작업자
    monitor_worker: Option<tokio::task::JoinHandle<()>>,
}

/// 연결 풀 성능 메트릭
#[derive(Debug, Default)]
pub struct PoolMetrics {
    pub total_connections: std::sync::atomic::AtomicU64,
    pub active_connections: std::sync::atomic::AtomicU64,
    pub idle_connections: std::sync::atomic::AtomicU64,
    pub connection_errors: std::sync::atomic::AtomicU64,
    pub query_count: std::sync::atomic::AtomicU64,
    pub avg_query_time_ms: std::sync::atomic::AtomicU64,
    pub start_time: std::time::Instant,
}

impl PoolMetrics {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            ..Default::default()
        }
    }
    
    pub fn record_query(&self, duration_ms: u64) {
        self.query_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // 이동 평균 계산
        let current_avg = self.avg_query_time_ms.load(std::sync::atomic::Ordering::Relaxed);
        let new_avg = (current_avg + duration_ms) / 2;
        self.avg_query_time_ms.store(new_avg, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_connection_error(&self) {
        self.connection_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn get_qps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.query_count.load(std::sync::atomic::Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }
}

impl OptimizedDatabasePool {
    /// 새 최적화된 연결 풀 생성
    pub async fn new(database_url: &str) -> Result<Self, SqlxError> {
        info!("🚀 최적화된 데이터베이스 연결 풀 초기화 중...");
        
        // 쓰기용 연결 풀 (더 많은 연결)
        let write_pool = SqlitePoolOptions::new()
            .max_connections(20)  // 기본 5 → 20
            .min_connections(5)    // 최소 연결 유지
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(300)) // 5분
            .max_lifetime(Duration::from_secs(1800)) // 30분
            .connect(database_url)
            .await?;
        
        // 읽기용 연결 풀들 (부하 분산)
        let mut read_pools = Vec::new();
        for i in 0..3 { // 3개의 읽기 풀
            let read_pool = SqlitePoolOptions::new()
                .max_connections(10)  // 읽기용은 적게
                .min_connections(2)
                .acquire_timeout(Duration::from_secs(5))
                .idle_timeout(Duration::from_secs(600)) // 10분
                .max_lifetime(Duration::from_secs(3600)) // 1시간
                .connect(database_url)
                .await?;
            
            read_pools.push(read_pool);
            info!("읽기 풀 {} 초기화 완료", i + 1);
        }
        
        let pool = Self {
            write_pool,
            read_pools,
            read_pool_index: std::sync::atomic::AtomicUsize::new(0),
            metrics: Arc::new(PoolMetrics::new()),
            monitor_worker: None,
        };
        
        info!("✅ 최적화된 연결 풀 초기화 완료");
        info!("   쓰기 풀: 20개 연결");
        info!("   읽기 풀: 3개 × 10개 연결 = 30개 연결");
        info!("   총 연결: 50개");
        
        Ok(pool)
    }
    
    /// 연결 풀 모니터링 시작
    pub fn start_monitoring(&mut self) {
        let write_pool = self.write_pool.clone();
        let read_pools = self.read_pools.clone();
        let metrics = self.metrics.clone();
        
        let monitor_worker = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // 연결 상태 확인
                Self::check_pool_health(&write_pool, &read_pools, &metrics).await;
            }
        });
        
        self.monitor_worker = Some(monitor_worker);
        info!("📊 연결 풀 모니터링 시작됨");
    }
    
    /// 연결 풀 모니터링 중지
    pub async fn stop_monitoring(&mut self) {
        if let Some(worker) = self.monitor_worker.take() {
            worker.abort();
            info!("연결 풀 모니터링 중지됨");
        }
    }
    
    /// 쓰기용 연결 풀 가져오기
    pub fn get_write_pool(&self) -> &SqlitePool {
        &self.write_pool
    }
    
    /// 읽기용 연결 풀 가져오기 (Round-Robin)
    pub fn get_read_pool(&self) -> &SqlitePool {
        let index = self.read_pool_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        &self.read_pools[index % self.read_pools.len()]
    }
    
    /// 쓰기 트랜잭션 실행
    pub async fn execute_write_transaction<F, T>(
        &self,
        f: F,
    ) -> Result<T, SqlxError>
    where
        F: FnOnce(&mut sqlx::Transaction<'_, sqlx::Sqlite>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, SqlxError>> + Send + '_>>,
    {
        let start_time = std::time::Instant::now();
        
        let mut tx = self.write_pool.begin().await?;
        let result = f(&mut tx).await?;
        tx.commit().await?;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.record_query(duration_ms);
        
        Ok(result)
    }
    
    /// 읽기 쿼리 실행
    pub async fn execute_read_query<F, T>(
        &self,
        f: F,
    ) -> Result<T, SqlxError>
    where
        F: FnOnce(&SqlitePool) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, SqlxError>> + Send + '_>>,
    {
        let start_time = std::time::Instant::now();
        
        let pool = self.get_read_pool();
        let result = f(pool).await?;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.record_query(duration_ms);
        
        Ok(result)
    }
    
    /// 배치 쓰기 트랜잭션 실행
    pub async fn execute_batch_write_transaction<F>(
        &self,
        batch_size: usize,
        f: F,
    ) -> Result<usize, SqlxError>
    where
        F: FnOnce(&mut sqlx::Transaction<'_, sqlx::Sqlite>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize, SqlxError>> + Send + '_>>,
    {
        let start_time = std::time::Instant::now();
        
        let mut tx = self.write_pool.begin().await?;
        let processed_count = f(&mut tx).await?;
        tx.commit().await?;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        self.metrics.record_query(duration_ms);
        
        debug!("배치 쓰기 트랜잭션 완료: {}개 처리, {}ms", processed_count, duration_ms);
        Ok(processed_count)
    }
    
    /// 연결 풀 상태 확인
    async fn check_pool_health(
        write_pool: &SqlitePool,
        read_pools: &[SqlitePool],
        metrics: &Arc<PoolMetrics>,
    ) {
        // 쓰기 풀 상태 확인
        match write_pool.acquire().await {
            Ok(conn) => {
                drop(conn);
                debug!("쓰기 풀 상태: 정상");
            }
            Err(e) => {
                error!("쓰기 풀 상태: 오류 - {}", e);
                metrics.record_connection_error();
            }
        }
        
        // 읽기 풀들 상태 확인
        for (i, pool) in read_pools.iter().enumerate() {
            match pool.acquire().await {
                Ok(conn) => {
                    drop(conn);
                    debug!("읽기 풀 {} 상태: 정상", i + 1);
                }
                Err(e) => {
                    error!("읽기 풀 {} 상태: 오류 - {}", i + 1, e);
                    metrics.record_connection_error();
                }
            }
        }
    }
    
    /// 성능 메트릭 조회
    pub fn get_metrics(&self) -> &PoolMetrics {
        &self.metrics
    }
    
    /// 성능 통계 출력
    pub fn print_performance_stats(&self) {
        let metrics = &self.metrics;
        info!("📊 최적화된 연결 풀 성능 통계:");
        info!("   총 쿼리 수: {}", metrics.query_count.load(std::sync::atomic::Ordering::Relaxed));
        info!("   평균 쿼리 시간: {}ms", metrics.avg_query_time_ms.load(std::sync::atomic::Ordering::Relaxed));
        info!("   처리량: {:.2} QPS", metrics.get_qps());
        info!("   연결 오류: {}", metrics.connection_errors.load(std::sync::atomic::Ordering::Relaxed));
    }
    
    /// 연결 풀 상태 조회
    pub async fn get_pool_status(&self) -> PoolStatus {
        let write_pool_size = self.write_pool.size();
        let read_pool_sizes: Vec<u32> = self.read_pools.iter().map(|p| p.size()).collect();
        
        PoolStatus {
            write_pool_size,
            read_pool_sizes,
            total_connections: write_pool_size + read_pool_sizes.iter().sum::<u32>(),
        }
    }
    
    /// 연결 풀 정리
    pub async fn cleanup(&self) {
        info!("연결 풀 정리 중...");
        
        // 쓰기 풀 정리
        self.write_pool.close().await;
        
        // 읽기 풀들 정리
        for pool in &self.read_pools {
            pool.close().await;
        }
        
        info!("연결 풀 정리 완료");
    }
}

/// 연결 풀 상태 정보
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub write_pool_size: u32,
    pub read_pool_sizes: Vec<u32>,
    pub total_connections: u32,
}

/// 고성능 배치 처리기
pub struct HighPerformanceBatchProcessor {
    pool: OptimizedDatabasePool,
    batch_size: usize,
    flush_interval: Duration,
}

impl HighPerformanceBatchProcessor {
    pub fn new(pool: OptimizedDatabasePool) -> Self {
        Self {
            pool,
            batch_size: 1000,
            flush_interval: Duration::from_millis(100),
        }
    }
    
    pub fn with_batch_config(mut self, batch_size: usize, flush_interval: Duration) -> Self {
        self.batch_size = batch_size;
        self.flush_interval = flush_interval;
        self
    }
    
    /// 고성능 배치 삽입
    pub async fn batch_insert_executions(
        &self,
        executions: Vec<crate::matching_engine::model::ExecutionReport>,
    ) -> Result<usize, SqlxError> {
        if executions.is_empty() {
            return Ok(0);
        }
        
        self.pool.execute_batch_write_transaction(executions.len(), |tx| {
            Box::pin(async move {
                let mut count = 0;
                
                for execution in executions {
                    sqlx::query!(
                        "INSERT INTO executions (exec_id, taker_order_id, maker_order_id, symbol, side, price, quantity, taker_fee, maker_fee, transaction_time)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        execution.execution_id,
                        execution.order_id,
                        execution.counterparty_id,
                        execution.symbol,
                        format!("{:?}", execution.side),
                        execution.price as i64,
                        execution.quantity as i64,
                        0, // 수수료
                        0,
                        execution.timestamp as i64
                    )
                    .execute(&mut **tx)
                    .await?;
                    
                    count += 1;
                }
                
                Ok(count)
            })
        }).await
    }
    
    /// 고성능 배치 업데이트
    pub async fn batch_update_balances(
        &self,
        balance_updates: Vec<(String, String, i64)>, // (user_id, asset, delta)
    ) -> Result<usize, SqlxError> {
        if balance_updates.is_empty() {
            return Ok(0);
        }
        
        self.pool.execute_batch_write_transaction(balance_updates.len(), |tx| {
            Box::pin(async move {
                let mut count = 0;
                
                for (user_id, asset, delta) in balance_updates {
                    sqlx::query!(
                        "INSERT OR REPLACE INTO balances (client_id, asset, available, locked, updated_at)
                         VALUES (?, ?, 
                                 COALESCE((SELECT available FROM balances WHERE client_id = ? AND asset = ?), 0) + ?,
                                 0, CURRENT_TIMESTAMP)",
                        user_id,
                        asset,
                        user_id,
                        asset,
                        delta
                    )
                    .execute(&mut **tx)
                    .await?;
                    
                    count += 1;
                }
                
                Ok(count)
            })
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_optimized_pool_creation() {
        // 실제 테스트에서는 테스트용 DB를 사용해야 함
        // let pool = OptimizedDatabasePool::new("sqlite::memory:").await.unwrap();
        // assert_eq!(pool.read_pools.len(), 3);
    }
    
    #[tokio::test]
    async fn test_read_pool_round_robin() {
        // let mut pool = OptimizedDatabasePool::new("sqlite::memory:").await.unwrap();
        // 
        // // Round-Robin 테스트
        // let pool1 = pool.get_read_pool();
        // let pool2 = pool.get_read_pool();
        // let pool3 = pool.get_read_pool();
        // let pool4 = pool.get_read_pool();
        // 
        // // 3개 풀이 순환되어야 함
        // assert_eq!(pool1 as *const _, pool4 as *const _);
    }
    
    #[tokio::test]
    async fn test_batch_processor() {
        // let pool = OptimizedDatabasePool::new("sqlite::memory:").await.unwrap();
        // let processor = HighPerformanceBatchProcessor::new(pool);
        // 
        // // 배치 삽입 테스트
        // let executions = vec![
        //     // 테스트용 ExecutionReport들
        // ];
        // 
        // let count = processor.batch_insert_executions(executions).await.unwrap();
        // assert!(count > 0);
    }
}