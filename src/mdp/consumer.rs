//! MDP (Market Data Provider) Consumer
//!
//! 이 모듈은 Kafka에서 시장 데이터를 소비하여
//! 봉차트와 시장 통계를 실시간으로 계산합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use crate::mq::kafka_consumer::{KafkaConsumerWorker, KafkaConsumerConfig};
use crate::mq::kafka_producer::{MarketDataMessage, MarketStatisticsMessage, OrderBookUpdateMessage};

/// 체결 데이터 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub symbol: String,
    pub price: u64,
    pub quantity: u64,
    pub timestamp: u64,
    pub side: String, // "buy" or "sell"
    pub trade_id: String,
}

/// 봉차트 데이터 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickData {
    pub symbol: String,
    pub timeframe: String, // "1m", "5m", "15m", "1h", "4h", "1d"
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
    pub timestamp: u64,
    pub trade_count: u32,
}

/// 시장 통계 데이터 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStatistics {
    pub symbol: String,
    pub price_change_24h: f64,      // 24시간 가격 변동률 (%)
    pub volume_24h: u64,            // 24시간 거래량
    pub high_24h: u64,              // 24시간 최고가
    pub low_24h: u64,               // 24시간 최저가
    pub last_price: u64,            // 현재 가격
    pub bid_price: u64,             // 매수 호가
    pub ask_price: u64,             // 매도 호가
    pub spread: u64,                // 스프레드
    pub timestamp: u64,
}

/// MDP Consumer 설정
#[derive(Debug, Clone)]
pub struct MDPConsumerConfig {
    pub kafka_brokers: Vec<String>,
    pub topics: Vec<String>,
    pub consumer_group: String,
    pub batch_size: usize,
    pub processing_interval_ms: u64,
    pub candlestick_timeframes: Vec<String>,
    pub statistics_window_ms: u64,
}

impl Default for MDPConsumerConfig {
    fn default() -> Self {
        Self {
            kafka_brokers: vec!["localhost:9092".to_string()],
            topics: vec!["market-data".to_string(), "executions".to_string()],
            consumer_group: "mdp-consumer-group".to_string(),
            batch_size: 100,
            processing_interval_ms: 1000, // 1초마다 처리
            candlestick_timeframes: vec![
                "1m".to_string(),
                "5m".to_string(),
                "15m".to_string(),
                "1h".to_string(),
                "4h".to_string(),
                "1d".to_string(),
            ],
            statistics_window_ms: 86400000, // 24시간
        }
    }
}

/// MDP Consumer
pub struct MDPConsumer {
    /// Kafka Consumer Worker
    kafka_consumer: KafkaConsumerWorker,
    /// 설정
    config: MDPConsumerConfig,
    /// 봉차트 데이터 저장소
    candlestick_data: Arc<RwLock<HashMap<String, HashMap<String, CandlestickData>>>>,
    /// 시장 통계 데이터 저장소
    market_statistics: Arc<RwLock<HashMap<String, MarketStatistics>>>,
    /// 최근 체결 데이터 (봉차트 계산용)
    recent_trades: Arc<Mutex<HashMap<String, Vec<TradeData>>>>,
    /// 처리 중인 상태
    is_processing: Arc<Mutex<bool>>,
}

impl MDPConsumer {
    /// 새 MDP Consumer 생성
    pub fn new(config: MDPConsumerConfig) -> Self {
        let kafka_config = KafkaConsumerConfig {
            kafka_brokers: config.kafka_brokers.clone(),
            topic_name: config.topics.first().unwrap_or(&"market-data".to_string()).clone(),
            consumer_group: config.consumer_group.clone(),
            worker_id: "mdp-consumer".to_string(),
            batch_size: config.batch_size,
            processing_interval_ms: config.processing_interval_ms,
        };

        Self {
            kafka_consumer: KafkaConsumerWorker::new(kafka_config),
            config,
            candlestick_data: Arc::new(RwLock::new(HashMap::new())),
            market_statistics: Arc::new(RwLock::new(HashMap::new())),
            recent_trades: Arc::new(Mutex::new(HashMap::new())),
            is_processing: Arc::new(Mutex::new(false)),
        }
    }

    /// MDP Consumer 실행
    pub async fn run(&self) -> Result<(), String> {
        info!("MDP Consumer 시작");

        let candlestick_data = self.candlestick_data.clone();
        let market_statistics = self.market_statistics.clone();
        let recent_trades = self.recent_trades.clone();
        let config = self.config.clone();
        let is_processing = self.is_processing.clone();

        // Kafka 메시지 소비 태스크
        let kafka_task = {
            let candlestick_data = candlestick_data.clone();
            let market_statistics = market_statistics.clone();
            let recent_trades = recent_trades.clone();
            let config = config.clone();

            tokio::spawn(async move {
                loop {
                    match Self::consume_kafka_messages(&candlestick_data, &market_statistics, &recent_trades, &config).await {
                        Ok(_) => {
                            debug!("Kafka 메시지 처리 완료");
                        }
                        Err(e) => {
                            error!("Kafka 메시지 처리 실패: {}", e);
                            sleep(Duration::from_millis(1000)).await;
                        }
                    }
                }
            })
        };

        // 봉차트 계산 태스크
        let candlestick_task = {
            let candlestick_data = candlestick_data.clone();
            let recent_trades = recent_trades.clone();
            let config = config.clone();

            tokio::spawn(async move {
                let mut interval = interval(Duration::from_millis(config.processing_interval_ms));
                
                loop {
                    interval.tick().await;
                    
                    if let Err(e) = Self::calculate_candlesticks(&candlestick_data, &recent_trades, &config).await {
                        error!("봉차트 계산 실패: {}", e);
                    }
                }
            })
        };

        // 시장 통계 계산 태스크
        let statistics_task = {
            let market_statistics = market_statistics.clone();
            let recent_trades = recent_trades.clone();
            let config = config.clone();

            tokio::spawn(async move {
                let mut interval = interval(Duration::from_millis(config.processing_interval_ms));
                
                loop {
                    interval.tick().await;
                    
                    if let Err(e) = Self::calculate_market_statistics(&market_statistics, &recent_trades, &config).await {
                        error!("시장 통계 계산 실패: {}", e);
                    }
                }
            })
        };

        // 모든 태스크 완료 대기
        tokio::try_join!(kafka_task, candlestick_task, statistics_task)
            .map_err(|e| format!("MDP Consumer 태스크 실패: {}", e))?;

        Ok(())
    }

    /// Kafka 메시지 소비 및 처리
    async fn consume_kafka_messages(
        candlestick_data: &Arc<RwLock<HashMap<String, HashMap<String, CandlestickData>>>>,
        market_statistics: &Arc<RwLock<HashMap<String, MarketStatistics>>>,
        recent_trades: &Arc<Mutex<HashMap<String, Vec<TradeData>>>>,
        config: &MDPConsumerConfig,
    ) -> Result<(), String> {
        // Mock: 실제로는 Kafka에서 메시지를 소비
        let mock_trades = Self::create_mock_trade_data();
        let trade_count = mock_trades.len();
        
        for trade in mock_trades {
            // 최근 체결 데이터에 추가
            let mut trades_map = recent_trades.lock().await;
            let symbol_trades = trades_map.entry(trade.symbol.clone()).or_insert_with(Vec::new);
            symbol_trades.push(trade.clone());
            
            // 최대 1000개까지만 유지 (메모리 관리)
            if symbol_trades.len() > 1000 {
                symbol_trades.drain(0..symbol_trades.len() - 1000);
            }
        }
        
        debug!("Kafka 메시지 처리 완료: {}개 체결", trade_count);
        Ok(())
    }

    /// 봉차트 계산
    async fn calculate_candlesticks(
        candlestick_data: &Arc<RwLock<HashMap<String, HashMap<String, CandlestickData>>>>,
        recent_trades: &Arc<Mutex<HashMap<String, Vec<TradeData>>>>,
        config: &MDPConsumerConfig,
    ) -> Result<(), String> {
        let trades_map = recent_trades.lock().await;
        let mut candlestick_map = candlestick_data.write().await;
        
        for (symbol, trades) in trades_map.iter() {
            if trades.is_empty() {
                continue;
            }
            
            // 각 타임프레임별로 봉차트 계산
            for timeframe in &config.candlestick_timeframes {
                let timeframe_ms = Self::timeframe_to_ms(timeframe)?;
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                
                let window_start = current_time - (current_time % timeframe_ms);
                
                // 해당 시간대의 체결 데이터 필터링
                let window_trades: Vec<&TradeData> = trades
                    .iter()
                    .filter(|trade| trade.timestamp >= window_start && trade.timestamp < window_start + timeframe_ms)
                    .collect();
                
                if window_trades.is_empty() {
                    continue;
                }
                
                // OHLCV 계산
                let open = window_trades.first().unwrap().price;
                let close = window_trades.last().unwrap().price;
                let high = window_trades.iter().map(|t| t.price).max().unwrap();
                let low = window_trades.iter().map(|t| t.price).min().unwrap();
                let volume = window_trades.iter().map(|t| t.quantity).sum();
                let trade_count = window_trades.len() as u32;
                
                let candlestick = CandlestickData {
                    symbol: symbol.clone(),
                    timeframe: timeframe.clone(),
                    open,
                    high,
                    low,
                    close,
                    volume,
                    timestamp: window_start,
                    trade_count,
                };
                
                // 봉차트 데이터 저장
                let symbol_candlesticks = candlestick_map
                    .entry(symbol.clone())
                    .or_insert_with(HashMap::new);
                
                symbol_candlesticks.insert(timeframe.clone(), candlestick);
                
                debug!("봉차트 계산 완료: {} {} - O:{}, H:{}, L:{}, C:{}, V:{}", 
                       symbol, timeframe, open, high, low, close, volume);
            }
        }
        
        Ok(())
    }

    /// 시장 통계 계산
    async fn calculate_market_statistics(
        market_statistics: &Arc<RwLock<HashMap<String, MarketStatistics>>>,
        recent_trades: &Arc<Mutex<HashMap<String, Vec<TradeData>>>>,
        config: &MDPConsumerConfig,
    ) -> Result<(), String> {
        let trades_map = recent_trades.lock().await;
        let mut stats_map = market_statistics.write().await;
        
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let window_start = current_time - config.statistics_window_ms;
        
        for (symbol, trades) in trades_map.iter() {
            if trades.is_empty() {
                continue;
            }
            
            // 24시간 내 체결 데이터 필터링
            let window_trades: Vec<&TradeData> = trades
                .iter()
                .filter(|trade| trade.timestamp >= window_start)
                .collect();
            
            if window_trades.is_empty() {
                continue;
            }
            
            // 통계 계산
            let last_price = window_trades.last().unwrap().price;
            let first_price = window_trades.first().unwrap().price;
            let price_change_24h = if first_price > 0 {
                ((last_price as f64 - first_price as f64) / first_price as f64) * 100.0
            } else {
                0.0
            };
            
            let volume_24h = window_trades.iter().map(|t| t.quantity).sum();
            let high_24h = window_trades.iter().map(|t| t.price).max().unwrap();
            let low_24h = window_trades.iter().map(|t| t.price).min().unwrap();
            
            // Mock: 매수/매도 호가 (실제로는 OrderBook에서 가져옴)
            let bid_price = last_price.saturating_sub(1000);
            let ask_price = last_price + 1000;
            let spread = ask_price - bid_price;
            
            let statistics = MarketStatistics {
                symbol: symbol.clone(),
                price_change_24h,
                volume_24h,
                high_24h,
                low_24h,
                last_price,
                bid_price,
                ask_price,
                spread,
                timestamp: current_time,
            };
            
            stats_map.insert(symbol.clone(), statistics);
            
            debug!("시장 통계 계산 완료: {} - 가격변동: {:.2}%, 거래량: {}", 
                   symbol, price_change_24h, volume_24h);
        }
        
        Ok(())
    }

    /// 타임프레임을 밀리초로 변환
    fn timeframe_to_ms(timeframe: &str) -> Result<u64, String> {
        match timeframe {
            "1m" => Ok(60 * 1000),
            "5m" => Ok(5 * 60 * 1000),
            "15m" => Ok(15 * 60 * 1000),
            "1h" => Ok(60 * 60 * 1000),
            "4h" => Ok(4 * 60 * 60 * 1000),
            "1d" => Ok(24 * 60 * 60 * 1000),
            _ => Err(format!("지원하지 않는 타임프레임: {}", timeframe)),
        }
    }

    /// Mock 체결 데이터 생성
    fn create_mock_trade_data() -> Vec<TradeData> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let symbols = vec!["BTC-KRW", "ETH-KRW", "XRP-KRW"];
        let mut trades = Vec::new();
        
        for symbol in symbols {
            for i in 0..5 {
                let price = 50000000 + (i * 1000) + (fastrand::u32(0..10000) as u64);
                let quantity = 1000 + (fastrand::u32(0..1000) as u64);
                let side = if fastrand::bool() { "buy" } else { "sell" };
                
                trades.push(TradeData {
                    symbol: symbol.to_string(),
                    price,
                    quantity,
                    timestamp: current_time - (i * 1000),
                    side: side.to_string(),
                    trade_id: uuid::Uuid::new_v4().to_string(),
                });
            }
        }
        
        trades
    }

    /// 봉차트 데이터 조회
    pub async fn get_candlestick_data(&self, symbol: &str, timeframe: &str) -> Option<CandlestickData> {
        let candlestick_map = self.candlestick_data.read().await;
        candlestick_map
            .get(symbol)
            .and_then(|timeframes| timeframes.get(timeframe))
            .cloned()
    }

    /// 모든 봉차트 데이터 조회
    pub async fn get_all_candlestick_data(&self) -> HashMap<String, HashMap<String, CandlestickData>> {
        self.candlestick_data.read().await.clone()
    }

    /// 시장 통계 조회
    pub async fn get_market_statistics(&self, symbol: &str) -> Option<MarketStatistics> {
        let stats_map = self.market_statistics.read().await;
        stats_map.get(symbol).cloned()
    }

    /// 모든 시장 통계 조회
    pub async fn get_all_market_statistics(&self) -> HashMap<String, MarketStatistics> {
        self.market_statistics.read().await.clone()
    }

    /// 처리 중인 상태 확인
    pub async fn is_processing(&self) -> bool {
        *self.is_processing.lock().await
    }

    /// Consumer 통계 조회
    pub async fn get_consumer_stats(&self) -> MDPConsumerStats {
        let candlestick_count = self.candlestick_data.read().await.len();
        let statistics_count = self.market_statistics.read().await.len();
        let trades_count = self.recent_trades.lock().await.len();
        
        MDPConsumerStats {
            candlestick_count,
            statistics_count,
            trades_count,
            is_processing: self.is_processing().await,
        }
    }
}

/// MDP Consumer 통계
#[derive(Debug, Clone)]
pub struct MDPConsumerStats {
    pub candlestick_count: usize,
    pub statistics_count: usize,
    pub trades_count: usize,
    pub is_processing: bool,
}

// fastrand 의존성을 위해 간단한 랜덤 함수 구현
mod fastrand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn bool() -> bool {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        (hash % 2) == 0
    }

    pub fn u32(range: std::ops::Range<u32>) -> u32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        range.start + ((hash % (range.end - range.start) as u64) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mdp_consumer_creation() {
        let config = MDPConsumerConfig::default();
        let consumer = MDPConsumer::new(config);
        
        let stats = consumer.get_consumer_stats().await;
        assert_eq!(stats.candlestick_count, 0);
        assert_eq!(stats.statistics_count, 0);
        assert_eq!(stats.trades_count, 0);
    }

    #[tokio::test]
    async fn test_timeframe_conversion() {
        assert_eq!(MDPConsumer::timeframe_to_ms("1m").unwrap(), 60000);
        assert_eq!(MDPConsumer::timeframe_to_ms("5m").unwrap(), 300000);
        assert_eq!(MDPConsumer::timeframe_to_ms("1h").unwrap(), 3600000);
        assert_eq!(MDPConsumer::timeframe_to_ms("1d").unwrap(), 86400000);
        
        assert!(MDPConsumer::timeframe_to_ms("invalid").is_err());
    }

    #[tokio::test]
    async fn test_trade_data_creation() {
        let trades = MDPConsumer::create_mock_trade_data();
        assert!(!trades.is_empty());
        
        for trade in trades {
            assert!(!trade.symbol.is_empty());
            assert!(trade.price > 0);
            assert!(trade.quantity > 0);
            assert!(trade.timestamp > 0);
            assert!(!trade.trade_id.is_empty());
        }
    }

    #[tokio::test]
    async fn test_candlestick_calculation() {
        let config = MDPConsumerConfig::default();
        let consumer = MDPConsumer::new(config);
        
        // Mock 체결 데이터 추가
        let mut trades_map = consumer.recent_trades.lock().await;
        trades_map.insert("BTC-KRW".to_string(), vec![
            TradeData {
                symbol: "BTC-KRW".to_string(),
                price: 50000000,
                quantity: 1000,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                side: "buy".to_string(),
                trade_id: "test1".to_string(),
            },
            TradeData {
                symbol: "BTC-KRW".to_string(),
                price: 51000000,
                quantity: 2000,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 + 1000,
                side: "sell".to_string(),
                trade_id: "test2".to_string(),
            },
        ]);
        drop(trades_map);
        
        // 봉차트 계산
        let result = MDPConsumer::calculate_candlesticks(
            &consumer.candlestick_data,
            &consumer.recent_trades,
            &consumer.config,
        ).await;
        
        assert!(result.is_ok());
        
        // 결과 확인
        let candlestick = consumer.get_candlestick_data("BTC-KRW", "1m").await;
        assert!(candlestick.is_some());
        
        let candlestick = candlestick.unwrap();
        assert_eq!(candlestick.symbol, "BTC-KRW");
        assert_eq!(candlestick.timeframe, "1m");
        assert_eq!(candlestick.open, 50000000);
        assert_eq!(candlestick.close, 51000000);
        assert_eq!(candlestick.high, 51000000);
        assert_eq!(candlestick.low, 50000000);
        assert_eq!(candlestick.volume, 3000);
        assert_eq!(candlestick.trade_count, 2);
    }
}