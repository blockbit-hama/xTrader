use super::models::{ExecutionRecord, OrderRecord, BalanceRecord, AuditLog};
use sqlx::sqlite::SqlitePool;
use sqlx::Error as SqlxError;

/// 체결 내역 저장소
pub struct ExecutionRepository {
    pool: SqlitePool,
}

impl ExecutionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 체결 내역 저장
    pub async fn save(&self, execution: &ExecutionRecord) -> Result<(), SqlxError> {
        sqlx::query(
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 심볼별 체결 내역 조회
    pub async fn find_by_symbol(&self, symbol: &str, limit: i64) -> Result<Vec<ExecutionRecord>, SqlxError> {
        let executions = sqlx::query_as::<_, ExecutionRecord>(
            "SELECT exec_id, taker_order_id, maker_order_id, symbol, side, price, quantity, taker_fee, maker_fee, transaction_time
             FROM executions
             WHERE symbol = ?
             ORDER BY transaction_time DESC
             LIMIT ?"
        )
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(executions)
    }

    /// 모든 체결 내역 조회 (복구용)
    pub async fn find_all(&self) -> Result<Vec<ExecutionRecord>, SqlxError> {
        let executions = sqlx::query_as::<_, ExecutionRecord>(
            "SELECT exec_id, taker_order_id, maker_order_id, symbol, side, price, quantity, taker_fee, maker_fee, transaction_time
             FROM executions
             ORDER BY transaction_time ASC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(executions)
    }
}

/// 주문 저장소
pub struct OrderRepository {
    pool: SqlitePool,
}

impl OrderRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 주문 저장
    pub async fn save(&self, order: &OrderRecord) -> Result<(), SqlxError> {
        sqlx::query(
            "INSERT INTO orders
             (order_id, client_id, symbol, side, order_type, price, quantity, filled_quantity, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&order.order_id)
        .bind(&order.client_id)
        .bind(&order.symbol)
        .bind(&order.side)
        .bind(&order.order_type)
        .bind(order.price)
        .bind(order.quantity)
        .bind(order.filled_quantity)
        .bind(&order.status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 주문 상태 업데이트
    pub async fn update_status(
        &self,
        order_id: &str,
        filled_quantity: i64,
        status: &str,
    ) -> Result<(), SqlxError> {
        sqlx::query(
            "UPDATE orders
             SET filled_quantity = ?, status = ?, updated_at = CURRENT_TIMESTAMP
             WHERE order_id = ?"
        )
        .bind(filled_quantity)
        .bind(status)
        .bind(order_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 클라이언트별 주문 조회
    pub async fn find_by_client(&self, client_id: &str) -> Result<Vec<OrderRecord>, SqlxError> {
        let orders = sqlx::query_as::<_, OrderRecord>(
            "SELECT order_id, client_id, symbol, side, order_type, price, quantity, filled_quantity, status
             FROM orders
             WHERE client_id = ?
             ORDER BY created_at DESC"
        )
        .bind(client_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }
}

/// 잔고 저장소
pub struct BalanceRepository {
    pool: SqlitePool,
}

impl BalanceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 잔고 업데이트 (없으면 생성)
    pub async fn upsert(&self, balance: &BalanceRecord) -> Result<(), SqlxError> {
        sqlx::query(
            "INSERT INTO balances (client_id, asset, available, locked)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(client_id) DO UPDATE SET
                available = excluded.available,
                locked = excluded.locked,
                updated_at = CURRENT_TIMESTAMP"
        )
        .bind(&balance.client_id)
        .bind(&balance.asset)
        .bind(balance.available)
        .bind(balance.locked)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 잔고 조회
    pub async fn find_by_client(&self, client_id: &str) -> Result<Vec<BalanceRecord>, SqlxError> {
        let balances = sqlx::query_as::<_, BalanceRecord>(
            "SELECT client_id, asset, available, locked
             FROM balances
             WHERE client_id = ?"
        )
        .bind(client_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(balances)
    }
}

/// 감사 로그 저장소
pub struct AuditLogRepository {
    pool: SqlitePool,
}

impl AuditLogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 로그 기록
    pub async fn log(
        &self,
        event_type: &str,
        entity_type: &str,
        entity_id: &str,
        details: Option<&str>,
    ) -> Result<(), SqlxError> {
        sqlx::query(
            "INSERT INTO audit_logs (event_type, entity_type, entity_id, details)
             VALUES (?, ?, ?, ?)"
        )
        .bind(event_type)
        .bind(entity_type)
        .bind(entity_id)
        .bind(details)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 엔티티별 로그 조회
    pub async fn find_by_entity(&self, entity_id: &str) -> Result<Vec<AuditLog>, SqlxError> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT id, event_type, entity_type, entity_id, details
             FROM audit_logs
             WHERE entity_id = ?
             ORDER BY timestamp DESC"
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }
}
