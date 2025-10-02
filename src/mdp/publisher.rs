use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::matching_engine::model::{ExecutionReport, OrderBookSnapshot};
use crate::mdp::model::{MarketDataEvent, CandlestickData, MarketStatistics};
use crate::api::models::WebSocketMessage;

/// 시장 데이터 발행자
pub struct MarketDataPublisher {
    /// 심볼별 체결 내역 저장소
    executions: Arc<Mutex<HashMap<String, Vec<ExecutionReport>>>>,
    /// 심볼별 봉차트 데이터 저장소
    candlesticks: Arc<Mutex<HashMap<String, HashMap<String, Vec<CandlestickData>>>>>,
    /// 심볼별 시장 통계 저장소
    statistics: Arc<Mutex<HashMap<String, MarketStatistics>>>,
    /// 최대 체결 내역 보관 수
    max_executions: usize,
    /// WebSocket 브로드캐스트 채널
    broadcast_tx: Option<tokio::sync::broadcast::Sender<WebSocketMessage>>,
}

impl MarketDataPublisher {
    /// 새 MDP 생성
    pub fn new(max_executions: usize) -> Self {
        let mdp = Self {
            executions: Arc::new(Mutex::new(HashMap::new())),
            candlesticks: Arc::new(Mutex::new(HashMap::new())),
            statistics: Arc::new(Mutex::new(HashMap::new())),
            max_executions,
            broadcast_tx: None,
        };

        // 초기 가짜 데이터 로드 시도
        if let Err(e) = mdp.load_initial_data() {
            eprintln!("Failed to load initial fake data: {}", e);
        }

        mdp
    }

    /// WebSocket 브로드캐스트 채널 설정
    pub fn set_broadcast_channel(&mut self, broadcast_tx: tokio::sync::broadcast::Sender<WebSocketMessage>) {
        self.broadcast_tx = Some(broadcast_tx);
    }

    /// 가짜 데이터에서 초기 캔들 데이터 로드
    fn load_initial_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        use std::time::{SystemTime, UNIX_EPOCH};
        use crate::mdp::model::CandlestickData;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let base_time = now - (24 * 60 * 60); // 24시간 전부터 시작

        // BTC-KRW, ETH-KRW, AAPL에 대한 가짜 historical 캔들 생성
        let symbols = vec!["BTC-KRW", "ETH-KRW", "AAPL"];
        let base_prices = vec![100000000u64, 4000000u64, 200000u64];

        tokio::spawn({
            let candlesticks = self.candlesticks.clone();
            async move {
                let mut candlesticks_guard = candlesticks.lock().await;

                for (symbol_idx, symbol) in symbols.iter().enumerate() {
                    let base_price = base_prices[symbol_idx];
                    let symbol_candlesticks = candlesticks_guard.entry(symbol.to_string()).or_insert_with(HashMap::new);

                    // 5분 간격 캔들 100개 생성 (약 8시간 분량)
                    let interval_candles = symbol_candlesticks.entry("5m".to_string()).or_insert_with(Vec::new);

                    for i in 0..100 {
                        let open_time = base_time + (i * 5 * 60); // 5분 간격
                        let close_time = open_time + (5 * 60); // 5분 후 종가 시간
                        let price_variance = ((i as f64 * 0.123).sin() * 0.02 + 1.0) as f64;
                        let adjusted_price = (base_price as f64 * price_variance) as u64;

                        let open = adjusted_price;
                        let close = adjusted_price + (((i as f64 * 0.456).cos() * 0.01) * base_price as f64) as u64;
                        let high = std::cmp::max(open, close) + (base_price / 1000);
                        let low = std::cmp::min(open, close) - (base_price / 1500);
                        let volume = 50 + (i % 200) as u64;

                        let candle = CandlestickData {
                            open_time,
                            close_time,
                            open,
                            high,
                            low,
                            close,
                            volume,
                            trade_count: 10 + (i % 50) as u64,
                        };

                        interval_candles.push(candle);
                    }

                    println!("📊 생성된 {} 캔들 데이터: 100개", symbol);
                }
            }
        });

        Ok(())
    }

    /// 체결 보고서 처리
    pub async fn process_execution(&self, execution: ExecutionReport) {
        let symbol = execution.symbol.clone();
        
        // 체결 내역 저장
        {
            let mut executions = self.executions.lock().await;
            let symbol_executions = executions.entry(symbol.clone()).or_insert_with(Vec::new);
            symbol_executions.push(execution.clone());
            
            // 최대 개수 초과 시 오래된 것 제거
            if symbol_executions.len() > self.max_executions {
                symbol_executions.remove(0);
            }
        }

        // 봉차트 업데이트
        self.update_candlesticks(&execution).await;

        // 시장 통계 업데이트
        self.update_statistics(&execution).await;

        // 시장 데이터 브로드캐스트
        self.broadcast_market_data(&execution).await;
    }

    /// 봉차트 데이터 업데이트
    async fn update_candlesticks(&self, execution: &ExecutionReport) {
        let symbol = execution.symbol.clone();
        let timestamp = execution.timestamp;
        let price = execution.price;
        let quantity = execution.quantity;

        let mut candlesticks = self.candlesticks.lock().await;
        let symbol_candlesticks = candlesticks.entry(symbol).or_insert_with(HashMap::new);

        // 다양한 시간 간격에 대해 봉차트 업데이트
        let intervals = vec!["1m", "5m", "15m", "30m", "1h", "4h", "1d"];
        
        for interval in intervals {
            let interval_candles = symbol_candlesticks.entry(interval.to_string()).or_insert_with(Vec::new);
            
            // 현재 시간에 해당하는 봉 찾기
            let current_candle_time = self.get_candle_time(timestamp, interval);
            
            if let Some(last_candle) = interval_candles.last_mut() {
                if last_candle.close_time == current_candle_time {
                    // 기존 봉 업데이트
                    last_candle.high = last_candle.high.max(price);
                    last_candle.low = last_candle.low.min(price);
                    last_candle.close = price;
                    last_candle.volume += quantity;
                    last_candle.trade_count += 1;
                } else {
                    // 새 봉 생성
                    let new_candle = CandlestickData {
                        open_time: current_candle_time,
                        close_time: current_candle_time,
                        open: price,
                        high: price,
                        low: price,
                        close: price,
                        volume: quantity,
                        trade_count: 1,
                    };
                    interval_candles.push(new_candle);
                    
                    // 최대 봉 개수 제한
                    self.limit_candles(interval_candles, interval);
                }
            } else {
                // 첫 번째 봉 생성
                let new_candle = CandlestickData {
                    open_time: current_candle_time,
                    close_time: current_candle_time,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume: quantity,
                    trade_count: 1,
                };
                interval_candles.push(new_candle);
            }
        }
    }

    /// 시장 통계 업데이트
    async fn update_statistics(&self, execution: &ExecutionReport) {
        let symbol = execution.symbol.clone();
        let price = execution.price;
        let quantity = execution.quantity;

        let mut statistics = self.statistics.lock().await;
        let stats = statistics.entry(symbol).or_insert_with(|| MarketStatistics {
            symbol: execution.symbol.clone(),
            timestamp: execution.timestamp,
            open_price_24h: None,
            high_price_24h: None,
            low_price_24h: None,
            last_price: Some(price),
            volume_24h: 0,
            price_change_24h: None,
            bid_price: None,
            ask_price: None,
        });

        // 최고가/최저가 업데이트
        if stats.high_price_24h.is_none() || price > stats.high_price_24h.unwrap() {
            stats.high_price_24h = Some(price);
        }
        if stats.low_price_24h.is_none() || price < stats.low_price_24h.unwrap() {
            stats.low_price_24h = Some(price);
        }

        // 거래량 업데이트
        stats.volume_24h += quantity;

        // 마지막 가격 업데이트
        stats.last_price = Some(price);

        // 24시간 시작가 설정 (첫 거래 시)
        if stats.open_price_24h.is_none() {
            stats.open_price_24h = Some(price);
        }

        // 가격 변화율 계산
        if let (Some(open), Some(last)) = (stats.open_price_24h, stats.last_price) {
            if open > 0 {
                stats.price_change_24h = Some(((last as f64 - open as f64) / open as f64) * 100.0);
            }
        }

        stats.timestamp = execution.timestamp;
    }

    /// 봉차트 시간 계산
    fn get_candle_time(&self, timestamp: u64, interval: &str) -> u64 {
        let interval_seconds = match interval {
            "1m" => 60,
            "5m" => 300,
            "15m" => 900,
            "30m" => 1800,
            "1h" => 3600,
            "4h" => 14400,
            "1d" => 86400,
            _ => 60,
        };

        (timestamp / interval_seconds) * interval_seconds
    }

    /// 봉차트 개수 제한
    fn limit_candles(&self, candles: &mut Vec<CandlestickData>, interval: &str) {
        let max_candles = match interval {
            "1m" => 1440,   // 24시간
            "5m" => 1152,   // 4일
            "15m" => 960,   // 10일
            "30m" => 1008,  // 3주
            "1h" => 720,    // 30일
            "4h" => 720,    // 120일
            "1d" => 365,    // 1년
            _ => 100,
        };

        if candles.len() > max_candles {
            let remove_count = candles.len() - max_candles;
            candles.drain(0..remove_count);
        }
    }

    /// 체결 내역 조회
    pub async fn get_executions(&self, symbol: &str, limit: usize) -> Vec<ExecutionReport> {
        let executions = self.executions.lock().await;
        if let Some(symbol_executions) = executions.get(symbol) {
            let start = symbol_executions.len().saturating_sub(limit);
            symbol_executions[start..].to_vec()
        } else {
            vec![]
        }
    }

    /// 봉차트 데이터 조회
    pub async fn get_candles(&self, symbol: &str, interval: &str, limit: usize) -> Vec<CandlestickData> {
        let candlesticks = self.candlesticks.lock().await;
        if let Some(symbol_candlesticks) = candlesticks.get(symbol) {
            if let Some(interval_candles) = symbol_candlesticks.get(interval) {
                let start = interval_candles.len().saturating_sub(limit);
                interval_candles[start..].to_vec()
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    /// 시장 통계 조회
    pub async fn get_statistics(&self, symbol: &str) -> Option<MarketStatistics> {
        let statistics = self.statistics.lock().await;
        statistics.get(symbol).cloned()
    }

    /// 주문서 업데이트 (오더북 스냅샷 기반)
    pub async fn update_orderbook(&self, snapshot: OrderBookSnapshot) {
        // 주문서 업데이트는 별도로 처리할 수 있음
        // 현재는 체결 데이터만 처리
    }

    /// 시장 데이터 브로드캐스트
    async fn broadcast_market_data(&self, execution: &ExecutionReport) {
        if let Some(ref broadcast_tx) = self.broadcast_tx {
            let symbol = execution.symbol.clone();
            
            // 시장 통계 브로드캐스트
            if let Some(stats) = self.get_statistics(&symbol).await {
                let message = WebSocketMessage::MarketStatistics {
                    symbol: stats.symbol.clone(),
                    timestamp: stats.timestamp,
                    last_price: stats.last_price,
                    price_change_24h: stats.price_change_24h,
                    volume_24h: stats.volume_24h,
                    high_price_24h: stats.high_price_24h,
                    low_price_24h: stats.low_price_24h,
                };
                
                if let Err(e) = broadcast_tx.send(message) {
                    eprintln!("Failed to broadcast market statistics: {}", e);
                }
            }

            // 봉차트 업데이트 브로드캐스트 (1분 봉만)
            let candles = self.get_candles(&symbol, "1m", 1).await;
            if let Some(latest_candle) = candles.last() {
                let message = WebSocketMessage::CandlestickUpdate {
                    symbol: symbol.clone(),
                    interval: "1m".to_string(),
                    candle: crate::api::models::CandleData {
                        open_time: latest_candle.open_time,
                        close_time: latest_candle.close_time,
                        open: latest_candle.open,
                        high: latest_candle.high,
                        low: latest_candle.low,
                        close: latest_candle.close,
                        volume: latest_candle.volume,
                        trade_count: latest_candle.trade_count,
                    },
                };
                
                if let Err(e) = broadcast_tx.send(message) {
                    eprintln!("Failed to broadcast candlestick update: {}", e);
                }
            }
        }
    }
}