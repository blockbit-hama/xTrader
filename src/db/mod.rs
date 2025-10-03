pub mod models;
pub mod repository;
pub mod async_commit;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Error as SqlxError;

pub use async_commit::{AsyncCommitManager, CommitStats};

/// SQLite 데이터베이스 초기화 및 연결
pub async fn init_database(database_url: &str) -> Result<SqlitePool, SqlxError> {
    println!("🗄️  SQLite 데이터베이스 초기화 중...");

    // 연결 풀 생성
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // 테이블 생성
    create_tables(&pool).await?;

    println!("✅ 데이터베이스 초기화 완료");

    Ok(pool)
}

/// 필요한 테이블 생성
async fn create_tables(pool: &SqlitePool) -> Result<(), SqlxError> {
    // 체결 내역 테이블
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS executions (
            exec_id TEXT PRIMARY KEY,
            taker_order_id TEXT NOT NULL,
            maker_order_id TEXT NOT NULL,
            symbol TEXT NOT NULL,
            side TEXT NOT NULL,
            price INTEGER NOT NULL,
            quantity INTEGER NOT NULL,
            taker_fee INTEGER NOT NULL,
            maker_fee INTEGER NOT NULL,
            transaction_time INTEGER NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(pool)
    .await?;

    // 주문 테이블
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS orders (
            order_id TEXT PRIMARY KEY,
            client_id TEXT NOT NULL,
            symbol TEXT NOT NULL,
            side TEXT NOT NULL,
            order_type TEXT NOT NULL,
            price INTEGER,
            quantity INTEGER NOT NULL,
            filled_quantity INTEGER DEFAULT 0,
            status TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(pool)
    .await?;

    // 잔고 테이블
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS balances (
            client_id TEXT PRIMARY KEY,
            asset TEXT NOT NULL,
            available INTEGER NOT NULL,
            locked INTEGER DEFAULT 0,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(pool)
    .await?;

    // 감사 로그 테이블
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            details TEXT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(pool)
    .await?;

    // 인덱스 생성
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_executions_symbol ON executions(symbol)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_executions_time ON executions(transaction_time)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_orders_client ON orders(client_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_orders_symbol ON orders(symbol)")
        .execute(pool)
        .await?;

    println!("📋 테이블 생성 완료");

    Ok(())
}
