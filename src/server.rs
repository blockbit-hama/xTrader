use std::sync::Arc;
use std::sync::mpsc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use sqlx::sqlite::SqlitePool;
use log::{info, warn, debug, error};

use crate::api::create_api_router;
use crate::matching_engine::engine::MatchingEngine;
use crate::matching_engine::model::{Order, ExecutionReport};
use crate::mdp::MarketDataPublisher;
use crate::sequencer::OrderSequencer;
use crate::api::models::WebSocketMessage;
use crate::db::AsyncCommitManager;
use crate::mq::{RedisStreamsProducer, RedisConsumerManager, ConsumerConfig, KafkaProducer, KafkaConsumerConfig, MDPConsumer, ExternalExchangeConsumer, RegulatoryConsumer, AnalyticsConsumer, RabbitMQProducer, RabbitMQConsumerConfig, WebSocketServerConsumer, LoadBalancerConsumer, DeadLetterQueueConsumer, LocalBackupQueue, MQHealthMonitor, RecoveryManager, HealthCheckConfig, RecoveryConfig};
use crate::mdp::{MDPConsumer as MDPConsumerType, MDPConsumerConfig, MDPApiServerBuilder, MDPCacheManager, CacheConfig};
use crate::external::{ExternalPriceSyncManager, PriceSyncConfig, RegulatoryReportingManager, RegulatoryReportingConfig, AnalyticsIntegrationManager, AnalyticsIntegrationConfig};
use crate::performance::{BatchProcessor, BatchProcessorConfig, WorkerPool, ParallelConsumerConfig, CacheOptimizer, CacheOptimizerConfig, MetricsCollector, MetricsCollectorConfig, PerformanceAnalyzer};
use crate::monitoring::{SystemHealthMonitor, HealthCheckConfig as MonitoringHealthCheckConfig, NotificationSystem, NotificationConfig, DashboardServer, DashboardConfig, DashboardDataProvider, LogAnalyzer, LogAnalyzerConfig};

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

    // 🚀 초고성능: 비동기 커밋 매니저 생성
    let async_commit_mgr = Arc::new(
        AsyncCommitManager::new(db_pool.clone())
            .with_config(100, 10) // 배치 크기: 100, 간격: 10ms
    );

    // 🚀 Redis Streams Producer 초기화
    let redis_producer = match RedisStreamsProducer::new("redis://localhost:6379", "executions").await {
        Ok(producer) => {
            println!("✅ Redis Streams Producer 초기화 완료");
            Some(Arc::new(producer))
        }
        Err(e) => {
            println!("⚠️ Redis Streams 연결 실패: {} (계속 실행)", e);
            None
        }
    };

    // 🚀 Kafka Producer 초기화
    let kafka_producer = match KafkaProducer::new(&["localhost:9092".to_string()], "market-data").await {
        Ok(producer) => {
            println!("✅ Kafka Producer 초기화 완료");
            Some(Arc::new(producer))
        }
        Err(e) => {
            println!("⚠️ Kafka 연결 실패: {} (계속 실행)", e);
            None
        }
    };

    // 🚀 RabbitMQ Producer 초기화
    let rabbitmq_producer = match RabbitMQProducer::new("amqp://localhost:5672", "websocket_notifications").await {
        Ok(producer) => {
            println!("✅ RabbitMQ Producer 초기화 완료");
            
            // Exchange 및 Dead Letter Queue 설정
            if let Err(e) = producer.setup_exchange().await {
                println!("⚠️ RabbitMQ Exchange 설정 실패: {}", e);
            }
            if let Err(e) = producer.setup_dead_letter_queue().await {
                println!("⚠️ RabbitMQ Dead Letter Queue 설정 실패: {}", e);
            }
            
            Some(Arc::new(producer))
        }
        Err(e) => {
            println!("⚠️ RabbitMQ 연결 실패: {} (계속 실행)", e);
            None
        }
    };

    // 🚀 장애 복구 시스템 초기화
    let backup_queue = Arc::new(LocalBackupQueue::new(
        "/tmp/mq_backup".to_string(),
        1000,  // 최대 메모리 큐 크기
        5000,  // 5초마다 디스크 백업
    ));

    let health_monitor = Arc::new(MQHealthMonitor::new(
        HealthCheckConfig::default(),
        backup_queue.clone(),
    ));

    let recovery_manager = Arc::new(RecoveryManager::new(
        backup_queue.clone(),
        health_monitor.clone(),
        RecoveryConfig::default(),
    ));

    // 헬스 모니터링 시작
    health_monitor.start_monitoring().await;
    println!("✅ MQ 헬스 모니터링 시작");

    // 복구 프로세스 시작
    recovery_manager.start_recovery().await;
    println!("✅ MQ 복구 프로세스 시작");

    // 주기적 리포트 생성 (백그라운드)
    let health_monitor_clone = health_monitor.clone();
    let recovery_manager_clone = recovery_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            // 헬스 리포트 출력
            let health_report = health_monitor_clone.generate_health_report().await;
            println!("{}", health_report);
            
            // 복구 리포트 출력
            let recovery_report = recovery_manager_clone.generate_recovery_report().await;
            println!("{}", recovery_report);
        }
    });

    // 🚀 MDP Consumer 및 API 서버 초기화
    let mdp_config = MDPConsumerConfig::default();
    let mdp_consumer = Arc::new(MDPConsumerType::new(mdp_config));
    
    // MDP Consumer 실행
    let mdp_consumer_clone = mdp_consumer.clone();
    tokio::spawn(async move {
        if let Err(e) = mdp_consumer_clone.run().await {
            error!("MDP Consumer 실행 실패: {}", e);
        }
    });
    println!("✅ MDP Consumer 시작");

    // 캐시 관리자 초기화
    let cache_config = CacheConfig::default();
    let cache_manager = Arc::new(MDPCacheManager::new(cache_config));
    cache_manager.start().await;
    println!("✅ MDP 캐시 관리자 시작");

    // MDP API 서버 시작
    let mdp_consumer_for_api = mdp_consumer.clone();
    tokio::spawn(async move {
        let builder = MDPApiServerBuilder::new()
            .consumer(mdp_consumer_for_api)
            .port(3001)
            .host("0.0.0.0".to_string());
        
        if let Err(e) = builder.run().await {
            error!("MDP API 서버 실행 실패: {}", e);
        }
    });
    println!("✅ MDP API 서버 시작 (포트: 3001)");

    // 🚀 외부 시스템 연동 초기화
    let price_sync_config = PriceSyncConfig::default();
    let price_sync_manager = Arc::new(ExternalPriceSyncManager::new(price_sync_config));
    
    // 가격 동기화 시작
    let price_sync_manager_clone = price_sync_manager.clone();
    tokio::spawn(async move {
        price_sync_manager_clone.start_sync().await;
    });
    println!("✅ 외부 거래소 가격 동기화 시작");

    // 규제 보고 시스템 초기화
    let regulatory_config = RegulatoryReportingConfig::default();
    let regulatory_manager = Arc::new(RegulatoryReportingManager::new(regulatory_config));
    
    // 규제 보고 시작
    let regulatory_manager_clone = regulatory_manager.clone();
    tokio::spawn(async move {
        regulatory_manager_clone.start_reporting().await;
    });
    println!("✅ 규제 기관 보고 시스템 시작");

    // 분석 시스템 연동 초기화
    let analytics_config = AnalyticsIntegrationConfig::default();
    let analytics_manager = Arc::new(AnalyticsIntegrationManager::new(analytics_config));
    
    // 분석 시스템 시작
    let analytics_manager_clone = analytics_manager.clone();
    tokio::spawn(async move {
        analytics_manager_clone.start_analysis().await;
    });
    println!("✅ 분석 시스템 연동 시작");

    // 주기적 외부 시스템 상태 리포트 (백그라운드)
    let price_sync_manager_report = price_sync_manager.clone();
    let regulatory_manager_report = regulatory_manager.clone();
    let analytics_manager_report = analytics_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // 가격 동기화 통계
            let price_stats = price_sync_manager_report.get_sync_stats().await;
            println!("📊 가격 동기화 통계: 심볼 {}개, 거래소 {}개, 차익거래 기회 {}개", 
                     price_stats.symbols_synced, price_stats.exchanges_monitored, price_stats.total_arbitrage_opportunities);
            
            // 규제 보고 통계
            let regulatory_stats = regulatory_manager_report.get_reporting_stats().await;
            println!("📋 규제 보고 통계: 거래 {}건, 의심활동 {}건, 보고서 {}건", 
                     regulatory_stats.total_transactions, regulatory_stats.total_suspicious_activities, regulatory_stats.total_reports);
            
            // 분석 시스템 통계
            let analytics_stats = analytics_manager_report.get_analysis_stats().await;
            println!("🔬 분석 시스템 통계: 대기 {}건, 활성 {}건, 완료 {}건, 평균 신뢰도 {:.2}", 
                     analytics_stats.pending_requests, analytics_stats.active_requests, analytics_stats.total_completed, analytics_stats.average_confidence);
        }
    });

    // 🚀 성능 최적화 시스템 초기화
    let batch_config = BatchProcessorConfig::default();
    let batch_processor = Arc::new(BatchProcessor::new(batch_config, |data| {
        // Mock 배치 처리 함수
        data.into_iter().map(|x| Ok(format!("processed_{}", x))).collect()
    }));
    
    // 배치 처리기 시작
    let batch_processor_clone = batch_processor.clone();
    tokio::spawn(async move {
        batch_processor_clone.start().await;
    });
    println!("✅ 배치 처리기 시작");

    // 병렬 소비 워커 풀 초기화
    let parallel_config = ParallelConsumerConfig::default();
    let worker_pool = Arc::new(WorkerPool::new(parallel_config, |data| {
        // Mock 병렬 처리 함수
        Ok(format!("processed_{}", data))
    }));
    
    // 워커 풀 시작
    let worker_pool_clone = worker_pool.clone();
    tokio::spawn(async move {
        worker_pool_clone.start().await;
    });
    println!("✅ 병렬 소비 워커 풀 시작");

    // 캐시 최적화기 초기화
    let cache_config = CacheOptimizerConfig::default();
    let cache_optimizer = Arc::new(CacheOptimizer::new(cache_config));
    
    // 캐시 최적화기 시작
    let cache_optimizer_clone = cache_optimizer.clone();
    tokio::spawn(async move {
        cache_optimizer_clone.start().await;
    });
    println!("✅ 캐시 최적화기 시작");

    // 메트릭 수집기 초기화
    let metrics_config = MetricsCollectorConfig::default();
    let metrics_collector = Arc::new(MetricsCollector::new(metrics_config));
    
    // 메트릭 수집기 시작
    let metrics_collector_clone = metrics_collector.clone();
    tokio::spawn(async move {
        metrics_collector_clone.start().await;
    });
    println!("✅ 메트릭 수집기 시작");

    // 성능 분석기 초기화
    let performance_analyzer = Arc::new(PerformanceAnalyzer::new(metrics_collector.clone()));

    // 주기적 성능 모니터링 (백그라운드)
    let batch_processor_monitor = batch_processor.clone();
    let worker_pool_monitor = worker_pool.clone();
    let cache_optimizer_monitor = cache_optimizer.clone();
    let metrics_collector_monitor = metrics_collector.clone();
    let performance_analyzer_monitor = performance_analyzer.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            // 배치 처리 통계
            let batch_stats = batch_processor_monitor.get_stats().await;
            println!("📊 배치 처리 통계: 총 배치 {}개, 메시지 {}개, 처리량 {:.1}/초", 
                     batch_stats.total_batches, batch_stats.total_messages, batch_stats.throughput_per_second);
            
            // 병렬 소비 통계
            let parallel_stats = worker_pool_monitor.get_stats().await;
            println!("⚡ 병렬 소비 통계: 워커 {}개, 처리 {}개, 실패 {}개", 
                     parallel_stats.total_workers, parallel_stats.total_processed, parallel_stats.total_failed);
            
            // 캐시 통계
            let cache_stats = cache_optimizer_monitor.get_stats().await;
            println!("💾 캐시 통계: 히트율 {:.1}%, L1 {}개, L2 {}개, L3 {}개", 
                     cache_stats.hit_rate * 100.0, cache_stats.l1_size, cache_stats.l2_size, cache_stats.l3_size);
            
            // 성능 분석 리포트
            let performance_report = performance_analyzer_monitor.generate_performance_report().await;
            println!("📈 성능 분석: CPU {:.1}%, 메모리 {:.1}%, 응답시간 {:.1}ms, 에러율 {:.2}%", 
                     performance_report.average_cpu_usage, performance_report.average_memory_usage, 
                     performance_report.average_response_time, performance_report.error_rate);
            
            // 추천사항 출력
            for recommendation in &performance_report.recommendations {
                println!("💡 추천: {}", recommendation);
            }
        }
    });

    // 🔍 모니터링 및 헬스체크 시스템 초기화
    let notification_config = NotificationConfig::default();
    let notification_system = Arc::new(NotificationSystem::new(notification_config));
    
    // 알림 시스템 시작
    let notification_system_clone = notification_system.clone();
    tokio::spawn(async move {
        notification_system_clone.start().await;
    });
    println!("✅ 알림 시스템 시작");

    // 헬스체크 모니터 초기화
    let health_config = MonitoringHealthCheckConfig::default();
    let health_monitor = Arc::new(SystemHealthMonitor::new(health_config, move |message, _type| {
        println!("🚨 헬스체크 알림: {}", message);
        Ok(())
    }));
    
    // 헬스체크 모니터 시작
    let health_monitor_clone = health_monitor.clone();
    tokio::spawn(async move {
        health_monitor_clone.start().await;
    });
    println!("✅ 헬스체크 모니터 시작");

    // 대시보드 데이터 제공자 초기화
    let dashboard_data_provider = Arc::new(DashboardDataProvider::new());
    
    // 대시보드 서버 초기화
    let dashboard_config = DashboardConfig::default();
    let dashboard_server = Arc::new(DashboardServer::new(dashboard_config));
    
    // 대시보드 서버 시작
    let dashboard_server_clone = dashboard_server.clone();
    let dashboard_data_provider_clone = dashboard_data_provider.clone();
    tokio::spawn(async move {
        dashboard_server_clone.start(dashboard_data_provider_clone).await;
    });
    println!("✅ 대시보드 서버 시작");

    // 로그 분석기 초기화
    let log_analyzer_config = LogAnalyzerConfig::default();
    let log_analyzer = Arc::new(LogAnalyzer::new(log_analyzer_config));
    
    // 로그 분석기 시작
    let log_analyzer_clone = log_analyzer.clone();
    tokio::spawn(async move {
        log_analyzer_clone.start().await;
    });
    println!("✅ 로그 분석기 시작");

    // 주기적 모니터링 리포트 (백그라운드)
    let health_monitor_report = health_monitor.clone();
    let notification_system_report = notification_system.clone();
    let dashboard_server_report = dashboard_server.clone();
    let log_analyzer_report = log_analyzer.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // 시스템 헬스체크 리포트
            let system_health = health_monitor_report.get_system_health().await;
            println!("🏥 시스템 헬스: 전체 상태 {}, 정상 {}개, 경고 {}개, 위험 {}개", 
                     match system_health.overall_status {
                         crate::monitoring::HealthStatus::Healthy => "정상",
                         crate::monitoring::HealthStatus::Warning => "경고",
                         crate::monitoring::HealthStatus::Critical => "위험",
                         crate::monitoring::HealthStatus::Unknown => "알수없음",
                     },
                     system_health.healthy_services, 
                     system_health.warning_services, 
                     system_health.critical_services);
            
            // 추천사항 출력
            for recommendation in &system_health.recommendations {
                println!("💡 추천: {}", recommendation);
            }
            
            // 알림 시스템 통계
            let notification_stats = notification_system_report.get_stats().await;
            println!("📢 알림 통계: 발송 {}개, 실패 {}개, 성공률 {:.1}%", 
                     notification_stats.total_sent, 
                     notification_stats.total_failed, 
                     notification_stats.success_rate * 100.0);
            
            // 대시보드 통계
            let dashboard_stats = dashboard_server_report.get_dashboard_stats().await;
            println!("📊 대시보드: 레이아웃 {}개, 위젯 {}개, 연결된 클라이언트 {}개", 
                     dashboard_stats.total_layouts, 
                     dashboard_stats.total_widgets, 
                     dashboard_stats.connected_clients);
            
            // 로그 분석 통계
            let log_stats = log_analyzer_report.get_stats().await;
            println!("📝 로그 분석: 총 {}개, 에러율 {:.2}%, 경고율 {:.2}%, 모듈 {}개", 
                     log_stats.total_entries, 
                     log_stats.error_rate, 
                     log_stats.warning_rate, 
                     log_stats.unique_modules);
        }
    });

    // 매칭 엔진 생성 (RabbitMQ Producer 초기화 후)
    let mut engine = MatchingEngine::new(config.symbols.clone(), exec_tx, rabbitmq_producer.clone());
    engine.set_broadcast_channel(broadcast_tx.clone());
    let engine = Arc::new(Mutex::new(engine));

    // MDP 생성
    let mut mdp = MarketDataPublisher::new(1000);
    mdp.set_broadcast_channel(broadcast_tx.clone());
    let mdp = Arc::new(Mutex::new(mdp));

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
        redis_producer.clone(),
        kafka_producer.clone(),
        rabbitmq_producer.clone(),
    );

    // 시퀀서 실행 태스크
    tokio::spawn(async move {
        sequencer.run().await;
    });

    // 🚀 Redis Consumer Manager 초기화 및 실행
    let redis_producer_clone = redis_producer.clone();
    if redis_producer_clone.is_some() {
        let consumer_configs = vec![
            ConsumerConfig {
                redis_url: "redis://localhost:6379".to_string(),
                stream_name: "executions".to_string(),
                consumer_group: "execution_processors".to_string(),
                worker_id: "worker-1".to_string(),
                batch_size: 100,
                processing_interval_ms: 100,
            },
            ConsumerConfig {
                redis_url: "redis://localhost:6379".to_string(),
                stream_name: "executions".to_string(),
                consumer_group: "execution_processors".to_string(),
                worker_id: "worker-2".to_string(),
                batch_size: 100,
                processing_interval_ms: 100,
            },
            ConsumerConfig {
                redis_url: "redis://localhost:6379".to_string(),
                stream_name: "executions".to_string(),
                consumer_group: "execution_processors".to_string(),
                worker_id: "worker-3".to_string(),
                batch_size: 100,
                processing_interval_ms: 100,
            },
        ];

        match RedisConsumerManager::new(consumer_configs, db_pool.clone()).await {
            Ok(consumer_manager) => {
                println!("✅ Redis Consumer Manager 초기화 완료 (3개 Worker)");
                
                // Consumer Manager 실행 (백그라운드)
                tokio::spawn(async move {
                    if let Err(e) = consumer_manager.start_all_workers().await {
                        println!("❌ Redis Consumer Manager 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ Redis Consumer Manager 초기화 실패: {}", e);
            }
        }
    }

    // 🚀 Kafka Consumer Manager 초기화 및 실행
    if let Some(_kafka_producer) = &kafka_producer {
        // MDP Consumer (Market Data Publisher)
        let mdp_config = KafkaConsumerConfig {
            kafka_brokers: vec!["localhost:9092".to_string()],
            topic_name: "market-data".to_string(),
            consumer_group: "mdp-group".to_string(),
            worker_id: "mdp-worker".to_string(),
            batch_size: 100,
            processing_interval_ms: 100,
        };

        match MDPConsumer::new(mdp_config).await {
            Ok(mdp_consumer) => {
                println!("✅ MDP Consumer 초기화 완료");
                
                tokio::spawn(async move {
                    if let Err(e) = mdp_consumer.run().await {
                        println!("❌ MDP Consumer 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ MDP Consumer 초기화 실패: {}", e);
            }
        }

        // 외부 거래소 Consumer
        let external_config = KafkaConsumerConfig {
            kafka_brokers: vec!["localhost:9092".to_string()],
            topic_name: "market-data".to_string(),
            consumer_group: "exchange-sync".to_string(),
            worker_id: "external-worker".to_string(),
            batch_size: 100,
            processing_interval_ms: 100,
        };

        match ExternalExchangeConsumer::new(external_config).await {
            Ok(external_consumer) => {
                println!("✅ 외부 거래소 Consumer 초기화 완료");
                
                tokio::spawn(async move {
                    if let Err(e) = external_consumer.run().await {
                        println!("❌ 외부 거래소 Consumer 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ 외부 거래소 Consumer 초기화 실패: {}", e);
            }
        }

        // 규제 기관 Consumer
        let regulatory_config = KafkaConsumerConfig {
            kafka_brokers: vec!["localhost:9092".to_string()],
            topic_name: "market-data".to_string(),
            consumer_group: "regulatory".to_string(),
            worker_id: "regulatory-worker".to_string(),
            batch_size: 100,
            processing_interval_ms: 100,
        };

        match RegulatoryConsumer::new(regulatory_config).await {
            Ok(regulatory_consumer) => {
                println!("✅ 규제 기관 Consumer 초기화 완료");
                
                tokio::spawn(async move {
                    if let Err(e) = regulatory_consumer.run().await {
                        println!("❌ 규제 기관 Consumer 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ 규제 기관 Consumer 초기화 실패: {}", e);
            }
        }

        // 분석 시스템 Consumer
        let analytics_config = KafkaConsumerConfig {
            kafka_brokers: vec!["localhost:9092".to_string()],
            topic_name: "market-data".to_string(),
            consumer_group: "analytics".to_string(),
            worker_id: "analytics-worker".to_string(),
            batch_size: 100,
            processing_interval_ms: 100,
        };

        match AnalyticsConsumer::new(analytics_config).await {
            Ok(analytics_consumer) => {
                println!("✅ 분석 시스템 Consumer 초기화 완료");
                
                tokio::spawn(async move {
                    if let Err(e) = analytics_consumer.run().await {
                        println!("❌ 분석 시스템 Consumer 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ 분석 시스템 Consumer 초기화 실패: {}", e);
            }
        }
    }

    // 🚀 RabbitMQ Consumer Manager 초기화 및 실행
    if let Some(_rabbitmq_producer) = &rabbitmq_producer {
        // WebSocket 서버 Consumer들 설정
        let server_configs = vec![
            (
                RabbitMQConsumerConfig {
                    rabbitmq_url: "amqp://localhost:5672".to_string(),
                    exchange_name: "websocket_notifications".to_string(),
                    queue_name: "ws-server-1".to_string(),
                    routing_patterns: vec!["execution.*".to_string(), "orderbook.*".to_string()],
                    worker_id: "ws-worker-1".to_string(),
                    batch_size: 100,
                    processing_interval_ms: 100,
                },
                "ws-server-1".to_string(),
                1000, // 최대 연결 수
            ),
            (
                RabbitMQConsumerConfig {
                    rabbitmq_url: "amqp://localhost:5672".to_string(),
                    exchange_name: "websocket_notifications".to_string(),
                    queue_name: "ws-server-2".to_string(),
                    routing_patterns: vec!["execution.*".to_string(), "orderbook.*".to_string()],
                    worker_id: "ws-worker-2".to_string(),
                    batch_size: 100,
                    processing_interval_ms: 100,
                },
                "ws-server-2".to_string(),
                1000, // 최대 연결 수
            ),
            (
                RabbitMQConsumerConfig {
                    rabbitmq_url: "amqp://localhost:5672".to_string(),
                    exchange_name: "websocket_notifications".to_string(),
                    queue_name: "ws-server-3".to_string(),
                    routing_patterns: vec!["execution.*".to_string(), "orderbook.*".to_string()],
                    worker_id: "ws-worker-3".to_string(),
                    batch_size: 100,
                    processing_interval_ms: 100,
                },
                "ws-server-3".to_string(),
                1000, // 최대 연결 수
            ),
        ];

        // 로드밸런서 Consumer 초기화
        match LoadBalancerConsumer::new(server_configs).await {
            Ok(load_balancer) => {
                println!("✅ RabbitMQ 로드밸런서 Consumer 초기화 완료");
                
                tokio::spawn(async move {
                    if let Err(e) = load_balancer.run().await {
                        println!("❌ RabbitMQ 로드밸런서 Consumer 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ RabbitMQ 로드밸런서 Consumer 초기화 실패: {}", e);
            }
        }

        // Dead Letter Queue Consumer 초기화
        let dlq_config = RabbitMQConsumerConfig {
            rabbitmq_url: "amqp://localhost:5672".to_string(),
            exchange_name: "websocket_notifications".to_string(),
            queue_name: "websocket_notifications.dlq".to_string(),
            routing_patterns: vec!["*".to_string()],
            worker_id: "dlq-worker".to_string(),
            batch_size: 50,
            processing_interval_ms: 5000, // 5초마다 처리
        };

        match DeadLetterQueueConsumer::new(dlq_config, 3).await {
            Ok(dlq_consumer) => {
                println!("✅ RabbitMQ Dead Letter Queue Consumer 초기화 완료");
                
                tokio::spawn(async move {
                    if let Err(e) = dlq_consumer.run().await {
                        println!("❌ RabbitMQ Dead Letter Queue Consumer 실행 오류: {}", e);
                    }
                });
            }
            Err(e) => {
                println!("⚠️ RabbitMQ Dead Letter Queue Consumer 초기화 실패: {}", e);
            }
        }
    }

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