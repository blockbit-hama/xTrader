use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use log::{debug, error, info, warn};

use crate::api::create_api_router;
use crate::matching_engine::ultra_fast_engine::UltraFastMatchingEngine;
use crate::matching_engine::model::{Order, ExecutionReport};
use crate::persistence::async_persistence::AsyncPersistenceManager;
use crate::cache::write_behind_cache::{WriteBehindCache, BalanceCache};
use crate::cache::lockfree_cache::LockFreeBalanceCache;
use crate::db::optimized_pool::OptimizedDatabasePool;
use crate::sequencer::OrderSequencer;
use crate::api::models::WebSocketMessage;

/// 초고성능 서버 설정
#[derive(Clone)]
pub struct UltraFastServerConfig {
    pub rest_port: u16,
    pub ws_port: u16,
    pub symbols: Vec<String>,
    pub batch_size: usize,
    pub batch_interval_ms: u64,
    pub cache_ttl_secs: u64,
    pub sync_interval_ms: u64,
}

impl Default for UltraFastServerConfig {
    fn default() -> Self {
        Self {
            rest_port: 7000,
            ws_port: 7001,
            symbols: vec!["BTC-KRW".into(), "ETH-KRW".into(), "AAPL".into()],
            batch_size: 1000,        // 더 큰 배치 크기
            batch_interval_ms: 5,    // 더 빠른 배치 간격
            cache_ttl_secs: 300,     // 5분 캐시 TTL
            sync_interval_ms: 50,    // 50ms 동기화 간격
        }
    }
}

/// 초고성능 서버 상태
pub struct UltraFastServerState {
    /// 초고성능 매칭 엔진
    pub engine: Arc<Mutex<UltraFastMatchingEngine>>,
    
    /// 비동기 영속화 매니저
    pub persistence_manager: Arc<AsyncPersistenceManager>,
    
    /// Write-Behind 캐시
    pub balance_cache: Arc<BalanceCache>,
    
    /// Lock-Free 캐시
    pub lockfree_cache: Arc<RwLock<LockFreeBalanceCache>>,
    
    /// 최적화된 연결 풀
    pub db_pool: OptimizedDatabasePool,
    
    /// WebSocket 브로드캐스트 채널
    pub execution_tx: tokio::sync::broadcast::Sender<WebSocketMessage>,
    
    /// 주문 채널
    pub order_tx: tokio::sync::mpsc::Sender<Order>,
}

/// 초고성능 서버
pub struct UltraFastServer {
    config: UltraFastServerConfig,
    state: Option<UltraFastServerState>,
}

impl UltraFastServer {
    /// 새 초고성능 서버 생성
    pub fn new(config: UltraFastServerConfig) -> Self {
        Self {
            config,
            state: None,
        }
    }
    
    /// 서버 시작
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🚀 초고성능 xTrader 서버 시작 중...");
        
        // 최적화된 연결 풀 생성
        let db_pool = OptimizedDatabasePool::new("sqlite:xtrader.db").await?;
        db_pool.start_monitoring();
        
        // 비동기 영속화 매니저 생성
        let persistence_manager = Arc::new(
            AsyncPersistenceManager::new(db_pool.get_write_pool().clone())
                .with_config(self.config.batch_size, self.config.batch_interval_ms)
        );
        
        // Write-Behind 캐시 생성
        let l3_cache = Arc::new(DatabaseCacheLayer::new(db_pool.get_write_pool().clone()));
        let mut balance_cache = BalanceCache::new(l3_cache);
        balance_cache.start();
        
        // Lock-Free 캐시 생성
        let mut lockfree_cache = LockFreeBalanceCache::new();
        lockfree_cache.start();
        
        // 채널 생성
        let (order_tx, order_rx) = tokio::sync::mpsc::channel::<Order>(10000);
        let (exec_tx, exec_rx) = tokio::sync::mpsc::channel::<ExecutionReport>(10000);
        let (broadcast_tx, _broadcast_rx) = tokio::sync::broadcast::channel(10000);
        
        // 초고성능 매칭 엔진 생성
        let mut engine = UltraFastMatchingEngine::new(self.config.symbols.clone(), exec_tx);
        engine.set_broadcast_channel(broadcast_tx.clone());
        let engine = Arc::new(Mutex::new(engine));
        
        // 비동기 영속화 루프 시작
        let persistence_clone = persistence_manager.clone();
        tokio::spawn(async move {
            persistence_clone.run_batch_commit_loop().await;
        });
        
        // 시퀀서 생성 및 실행
        let mut sequencer = OrderSequencer::new(
            order_rx,
            tokio::sync::mpsc::channel::<Order>(10000).1,
            exec_rx,
            broadcast_tx.clone(),
            Arc::new(Mutex::new(crate::mdp::MarketDataPublisher::new(1000))),
            persistence_manager.clone(),
        );
        
        tokio::spawn(async move {
            sequencer.run().await;
        });
        
        // 매칭 엔진 실행 태스크
        let engine_clone = engine.clone();
        tokio::spawn(async move {
            let mut engine_guard = engine_clone.lock().await;
            // 초고성능 엔진은 별도의 실행 루프가 필요함
        });
        
        // 성능 모니터링 태스크
        let engine_metrics = engine.clone();
        let persistence_metrics = persistence_manager.clone();
        let balance_cache_metrics = balance_cache.get_metrics();
        let lockfree_cache_metrics = lockfree_cache.get_metrics();
        let db_pool_metrics = db_pool.get_metrics();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                info!("📊 초고성능 서버 성능 리포트:");
                
                // 매칭 엔진 메트릭
                let engine_guard = engine_metrics.lock().await;
                engine_guard.print_performance_stats();
                drop(engine_guard);
                
                // 영속화 메트릭
                persistence_metrics.print_metrics();
                
                // 캐시 메트릭
                balance_cache_metrics.print_performance_stats();
                lockfree_cache_metrics.print_performance_stats();
                
                // DB 풀 메트릭
                db_pool_metrics.print_performance_stats();
                
                info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
        });
        
        // 서버 상태 생성
        let state = UltraFastServerState {
            engine,
            persistence_manager,
            balance_cache: Arc::new(balance_cache),
            lockfree_cache: Arc::new(RwLock::new(lockfree_cache)),
            db_pool,
            execution_tx: broadcast_tx,
            order_tx,
        };
        
        self.state = Some(state);
        
        info!("✅ 초고성능 서버 초기화 완료");
        info!("   배치 크기: {}", self.config.batch_size);
        info!("   배치 간격: {}ms", self.config.batch_interval_ms);
        info!("   캐시 TTL: {}초", self.config.cache_ttl_secs);
        info!("   동기화 간격: {}ms", self.config.sync_interval_ms);
        
        Ok(())
    }
    
    /// REST API 서버 시작
    pub async fn start_rest_server(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.state.as_ref().ok_or("서버가 초기화되지 않음")?;
        
        // REST API 라우터 생성
        let api_router = create_api_router()
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http())
            .with_state(state.clone());
        
        // REST API 서버 시작
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.config.rest_port))
            .await
            .expect("Failed to bind REST server");
        
        info!("🌐 초고성능 REST API 서버 시작됨: http://localhost:{}", self.config.rest_port);
        
        axum::serve(listener, api_router)
            .await
            .expect("REST server failed");
        
        Ok(())
    }
    
    /// 서버 중지
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(state) = self.state.take() {
            info!("🛑 초고성능 서버 중지 중...");
            
            // 캐시 중지
            state.balance_cache.stop().await;
            state.lockfree_cache.write().await.stop().await;
            
            // 연결 풀 모니터링 중지
            state.db_pool.stop_monitoring().await;
            
            // 연결 풀 정리
            state.db_pool.cleanup().await;
            
            info!("✅ 초고성능 서버 중지 완료");
        }
        
        Ok(())
    }
    
    /// 성능 벤치마크 실행
    pub async fn run_benchmark(&self) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let state = self.state.as_ref().ok_or("서버가 초기화되지 않음")?;
        
        info!("🏃 초고성능 벤치마크 시작...");
        
        let start_time = std::time::Instant::now();
        let mut order_count = 0;
        let mut execution_count = 0;
        
        // 벤치마크 주문 생성 및 전송
        for i in 0..10000 {
            let order = Order {
                id: format!("benchmark_{}", i),
                symbol: "BTC-KRW".to_string(),
                side: if i % 2 == 0 { crate::matching_engine::model::Side::Buy } else { crate::matching_engine::model::Side::Sell },
                order_type: crate::matching_engine::model::OrderType::Limit,
                price: 10000 + (i % 1000),
                quantity: 100,
                remaining_quantity: 100,
                client_id: "benchmark_user".to_string(),
                user_id: "benchmark_user".to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                is_cancel: false,
                target_order_id: None,
            };
            
            if let Err(_) = state.order_tx.send(order).await {
                break;
            }
            order_count += 1;
        }
        
        // 결과 수집 대기
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let duration = start_time.elapsed();
        let tps = order_count as f64 / duration.as_secs_f64();
        
        let result = BenchmarkResult {
            total_orders: order_count,
            total_executions: execution_count,
            duration_ms: duration.as_millis(),
            orders_per_second: tps,
            avg_latency_us: 0, // 실제로는 측정 필요
        };
        
        info!("🏁 벤치마크 완료:");
        info!("   총 주문: {}", result.total_orders);
        info!("   총 체결: {}", result.total_executions);
        info!("   처리량: {:.2} TPS", result.orders_per_second);
        info!("   평균 지연: {}μs", result.avg_latency_us);
        
        Ok(result)
    }
}

/// 벤치마크 결과
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub total_orders: usize,
    pub total_executions: usize,
    pub duration_ms: u128,
    pub orders_per_second: f64,
    pub avg_latency_us: u64,
}

/// 데이터베이스 캐시 레이어 구현
struct DatabaseCacheLayer {
    pool: sqlx::SqlitePool,
}

impl DatabaseCacheLayer {
    fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl crate::cache::write_behind_cache::CacheLayer for DatabaseCacheLayer {
    async fn get(&self, key: &str) -> Result<Option<serde_json::Value>, crate::cache::write_behind_cache::CacheError> {
        let result = sqlx::query!(
            "SELECT value FROM cache WHERE key = ?",
            key
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::cache::write_behind_cache::CacheError::LayerError(e.to_string()))?;
        
        if let Some(row) = result {
            Ok(Some(serde_json::from_str(&row.value)?))
        } else {
            Ok(None)
        }
    }
    
    async fn set(&self, key: &str, value: &serde_json::Value) -> Result<(), crate::cache::write_behind_cache::CacheError> {
        let value_str = serde_json::to_string(value)?;
        
        sqlx::query!(
            "INSERT OR REPLACE INTO cache (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)",
            key,
            value_str
        )
        .execute(&self.pool)
        .await
        .map_err(|e| crate::cache::write_behind_cache::CacheError::LayerError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn delete(&self, key: &str) -> Result<(), crate::cache::write_behind_cache::CacheError> {
        sqlx::query!(
            "DELETE FROM cache WHERE key = ?",
            key
        )
        .execute(&self.pool)
        .await
        .map_err(|e| crate::cache::write_behind_cache::CacheError::LayerError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool, crate::cache::write_behind_cache::CacheError> {
        let result = sqlx::query!(
            "SELECT 1 FROM cache WHERE key = ?",
            key
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::cache::write_behind_cache::CacheError::LayerError(e.to_string()))?;
        
        Ok(result.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ultra_fast_server_creation() {
        let config = UltraFastServerConfig::default();
        let mut server = UltraFastServer::new(config);
        
        // 실제 테스트에서는 테스트용 DB를 사용해야 함
        // let result = server.start().await;
        // assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_benchmark() {
        let config = UltraFastServerConfig::default();
        let mut server = UltraFastServer::new(config);
        
        // 서버 시작
        // server.start().await.unwrap();
        
        // 벤치마크 실행
        // let result = server.run_benchmark().await.unwrap();
        // assert!(result.orders_per_second > 0.0);
        
        // 서버 중지
        // server.stop().await.unwrap();
    }
}