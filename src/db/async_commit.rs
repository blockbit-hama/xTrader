//! 비동기 배치 커밋 매니저
//!
//! 초고성능을 위한 비차단 DB 커밋 시스템
//! - 메모리 우선: 체결은 즉시 메모리에서 완료
//! - 비동기 저장: DB 저장은 백그라운드에서 배치 처리
//! - 배치 최적화: 여러 체결을 하나의 트랜잭션으로 묶어 처리

use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;
use tokio::time::{Duration, interval};
use sqlx::sqlite::SqlitePool;
use log::{debug, error, info, warn};

use crate::db::models::ExecutionRecord;
use crate::db::repository::ExecutionRepository;

/// 비동기 커밋 매니저
///
/// DB 저장을 비차단 방식으로 처리하여 메인 체결 로직의 지연을 최소화합니다.
/// 배치 커밋을 통해 DB 트랜잭션 오버헤드를 줄입니다.
pub struct AsyncCommitManager {
    /// 커밋 대기 큐
    commit_queue: Arc<Mutex<VecDeque<ExecutionRecord>>>,
    /// 데이터베이스 풀
    db_pool: SqlitePool,
    /// 배치 크기 (한 번에 커밋할 최대 개수)
    batch_size: usize,
    /// 배치 간격 (밀리초)
    batch_interval_ms: u64,
    /// 통계: 총 커밋 수
    total_commits: Arc<Mutex<u64>>,
    /// 통계: 총 배치 수
    total_batches: Arc<Mutex<u64>>,
    /// 통계: 실패 수
    failed_commits: Arc<Mutex<u64>>,
}

impl AsyncCommitManager {
    /// 새 비동기 커밋 매니저 생성
    pub fn new(db_pool: SqlitePool) -> Self {
        Self {
            commit_queue: Arc::new(Mutex::new(VecDeque::new())),
            db_pool,
            batch_size: 100,           // 한 번에 최대 100개 커밋
            batch_interval_ms: 10,     // 10ms마다 배치 처리
            total_commits: Arc::new(Mutex::new(0)),
            total_batches: Arc::new(Mutex::new(0)),
            failed_commits: Arc::new(Mutex::new(0)),
        }
    }

    /// 설정 커스터마이징
    pub fn with_config(mut self, batch_size: usize, batch_interval_ms: u64) -> Self {
        self.batch_size = batch_size;
        self.batch_interval_ms = batch_interval_ms;
        self
    }

    /// 체결 내역을 큐에 추가 (비차단)
    pub async fn enqueue(&self, execution: ExecutionRecord) {
        let mut queue = self.commit_queue.lock().await;
        queue.push_back(execution);
        debug!("체결 내역 큐에 추가 (큐 크기: {})", queue.len());
    }

    /// 배치 커밋 루프 실행 (백그라운드 태스크)
    pub async fn run_batch_commit_loop(self: Arc<Self>) {
        info!("🚀 비동기 배치 커밋 루프 시작 (배치 크기: {}, 간격: {}ms)",
              self.batch_size, self.batch_interval_ms);

        let mut interval_timer = interval(Duration::from_millis(self.batch_interval_ms));

        loop {
            interval_timer.tick().await;

            // 큐에서 배치 가져오기
            let batch = {
                let mut queue = self.commit_queue.lock().await;
                let batch_size = std::cmp::min(self.batch_size, queue.len());
                queue.drain(..batch_size).collect::<Vec<_>>()
            };

            if batch.is_empty() {
                continue;
            }

            // 배치 커밋 실행
            if let Err(e) = self.commit_batch(batch).await {
                error!("배치 커밋 실패: {}", e);
            }
        }
    }

    /// 배치 커밋 실행
    async fn commit_batch(&self, batch: Vec<ExecutionRecord>) -> Result<(), sqlx::Error> {
        let batch_size = batch.len();
        debug!("배치 커밋 시작: {} 건", batch_size);

        let exec_repo = ExecutionRepository::new(self.db_pool.clone());

        // 단일 트랜잭션으로 모든 체결 저장
        let mut tx = self.db_pool.begin().await?;

        let mut success_count = 0;
        let mut fail_count = 0;

        for execution in batch {
            // INSERT OR REPLACE 사용으로 중복 처리
            let result = sqlx::query(
                "INSERT OR REPLACE INTO executions
                 (exec_id, taker_order_id, maker_order_id, symbol, side, price, quantity, taker_fee, maker_fee, transaction_time)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&execution.exec_id)
            .bind(&execution.taker_order_id)
            .bind(&execution.maker_order_id)
            .bind(&execution.symbol)
            .bind(&execution.side)
            .bind(execution.price)
            .bind(execution.quantity)
            .bind(execution.taker_fee)
            .bind(execution.maker_fee)
            .bind(execution.transaction_time)
            .execute(&mut *tx)
            .await;

            match result {
                Ok(_) => success_count += 1,
                Err(e) => {
                    warn!("체결 내역 저장 실패 - {}: {}", execution.exec_id, e);
                    fail_count += 1;
                }
            }
        }

        // 트랜잭션 커밋
        tx.commit().await?;

        // 통계 업데이트
        {
            let mut total_commits = self.total_commits.lock().await;
            *total_commits += success_count;
        }
        {
            let mut total_batches = self.total_batches.lock().await;
            *total_batches += 1;
        }
        {
            let mut failed_commits = self.failed_commits.lock().await;
            *failed_commits += fail_count;
        }

        info!("✅ 배치 커밋 완료: 성공 {} / 실패 {} (배치 크기: {})",
              success_count, fail_count, batch_size);

        Ok(())
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> CommitStats {
        CommitStats {
            total_commits: *self.total_commits.lock().await,
            total_batches: *self.total_batches.lock().await,
            failed_commits: *self.failed_commits.lock().await,
            queue_size: self.commit_queue.lock().await.len(),
        }
    }

    /// 큐 플러시 (모든 대기 중인 커밋 강제 실행)
    pub async fn flush(&self) -> Result<(), sqlx::Error> {
        info!("큐 플러시 시작...");

        loop {
            let batch = {
                let mut queue = self.commit_queue.lock().await;
                if queue.is_empty() {
                    break;
                }
                let batch_size = std::cmp::min(self.batch_size, queue.len());
                queue.drain(..batch_size).collect::<Vec<_>>()
            };

            self.commit_batch(batch).await?;
        }

        info!("✅ 큐 플러시 완료");
        Ok(())
    }
}

/// 커밋 통계
#[derive(Debug, Clone)]
pub struct CommitStats {
    pub total_commits: u64,
    pub total_batches: u64,
    pub failed_commits: u64,
    pub queue_size: usize,
}

impl std::fmt::Display for CommitStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CommitStats {{ 총 커밋: {}, 총 배치: {}, 실패: {}, 큐 크기: {} }}",
            self.total_commits, self.total_batches, self.failed_commits, self.queue_size
        )
    }
}
