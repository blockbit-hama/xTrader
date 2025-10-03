use std::sync::Arc;
use std::sync::mpsc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use sqlx::sqlite::SqlitePool;

use crate::api::create_api_router;
use crate::matching_engine::engine::MatchingEngine;
use crate::matching_engine::model::{Order, ExecutionReport};
use crate::mdp::MarketDataPublisher;
use crate::sequencer::OrderSequencer;
use crate::api::models::WebSocketMessage;
use crate::db::AsyncCommitManager;

/// 사용자 잔고 정보
#[derive(Debug, Clone)]
pub struct UserBalance {
    pub krw: u64,
    pub btc: u64,
}

impl Default for UserBalance {
    fn default() -> Self {
        Self {
            krw: 50_000_000_000, // 500억원 (테스트용)
            btc: 100_000_000,    // 100 BTC (테스트용)
        }
    }
}

/// 서버 설정
#[derive(Clone)]
pub struct ServerConfig {
    pub rest_port: u16,
    pub ws_port: u16,
    pub symbols: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            rest_port: 7000,
            ws_port: 7001,
            symbols: vec!["BTC-KRW".into(), "ETH-KRW".into(), "AAPL".into()],
        }
    }
}

/// 서버 상태
#[derive(Clone)]
pub struct ServerState {
    pub engine: Arc<Mutex<MatchingEngine>>,
    pub execution_tx: broadcast::Sender<WebSocketMessage>,
    pub order_tx: mpsc::Sender<Order>,
    pub mdp: Arc<Mutex<MarketDataPublisher>>,
    pub db_pool: SqlitePool,
}

/// 서버 시작
pub async fn start_server(config: ServerConfig, db_pool: SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("xTrader 서버 시작 중...");

    // 채널 생성
    let (order_tx, order_rx) = mpsc::channel::<Order>();
    let (exec_tx, exec_rx) = mpsc::channel::<ExecutionReport>();
    
    // 브로드캐스트 채널 생성 (WebSocket용)
    let (broadcast_tx, _broadcast_rx) = broadcast::channel(1000);

    // 매칭 엔진 생성
    let mut engine = MatchingEngine::new(config.symbols.clone(), exec_tx);
    engine.set_broadcast_channel(broadcast_tx.clone());
    let engine = Arc::new(Mutex::new(engine));

    // MDP 생성
    let mut mdp = MarketDataPublisher::new(1000);
    mdp.set_broadcast_channel(broadcast_tx.clone());
    let mdp = Arc::new(Mutex::new(mdp));

    // 🚀 초고성능: 비동기 커밋 매니저 생성
    let async_commit_mgr = Arc::new(
        AsyncCommitManager::new(db_pool.clone())
            .with_config(100, 10) // 배치 크기: 100, 간격: 10ms
    );

    // 비동기 커밋 루프 시작 (백그라운드)
    let commit_mgr_clone = async_commit_mgr.clone();
    tokio::spawn(async move {
        commit_mgr_clone.run_batch_commit_loop().await;
    });

    // 시퀀서용 채널 생성
    let (sequencer_tx, sequencer_rx) = mpsc::channel::<Order>();

    // 시퀀서 생성 및 실행
    let mut sequencer = OrderSequencer::new(
        order_rx,
        sequencer_tx,
        exec_rx,
        broadcast_tx.clone(),
        mdp.clone(),
        async_commit_mgr.clone(),
    );

    // 시퀀서 실행 태스크
    tokio::spawn(async move {
        sequencer.run().await;
    });

    // 매칭 엔진 실행 태스크 (시퀀서에서 주문을 받음)
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        let mut engine_guard = engine_clone.lock().await;
        engine_guard.run(sequencer_rx);
    });

    // MDP는 이제 시퀀서에서 직접 처리됨

    // 서버 상태 생성
    let state = ServerState {
        engine: engine.clone(),
        execution_tx: broadcast_tx,
        order_tx: order_tx,
        mdp: mdp.clone(),
        db_pool: db_pool.clone(),
    };

    // REST API 라우터 생성
    let api_router = create_api_router()
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    // REST API 서버 시작
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.rest_port))
        .await
        .expect("Failed to bind REST server");
    
    println!("서버가 성공적으로 시작되었습니다!");
    println!("REST API: http://localhost:{}", config.rest_port);
    
    axum::serve(listener, api_router)
        .await
        .expect("REST server failed");

    Ok(())
}