//! 외부 거래소 가격 동기화 시스템
//!
//! 이 모듈은 Binance, Coinbase, Kraken 등 외부 거래소의
//! 실시간 가격 데이터를 동기화하여 내부 시스템과 비교합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};

/// 외부 거래소 타입
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExchangeType {
    Binance,
    Coinbase,
    Kraken,
    Upbit,
    Bithumb,
}

impl std::fmt::Display for ExchangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExchangeType::Binance => write!(f, "Binance"),
            ExchangeType::Coinbase => write!(f, "Coinbase"),
            ExchangeType::Kraken => write!(f, "Kraken"),
            ExchangeType::Upbit => write!(f, "Upbit"),
            ExchangeType::Bithumb => write!(f, "Bithumb"),
        }
    }
}

/// 외부 거래소 가격 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPriceData {
    pub exchange: ExchangeType,
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: u64,
    pub bid_price: Option<f64>,
    pub ask_price: Option<f64>,
    pub spread: Option<f64>,
}

/// 가격 동기화 설정
#[derive(Debug, Clone)]
pub struct PriceSyncConfig {
    pub sync_interval_ms: u64,
    pub symbols: Vec<String>,
    pub exchanges: Vec<ExchangeType>,
    pub price_tolerance_percent: f64,
    pub max_price_deviation_percent: f64,
    pub enable_arbitrage_detection: bool,
}

impl Default for PriceSyncConfig {
    fn default() -> Self {
        Self {
            sync_interval_ms: 1000, // 1초마다 동기화
            symbols: vec![
                "BTC-KRW".to_string(),
                "ETH-KRW".to_string(),
                "XRP-KRW".to_string(),
            ],
            exchanges: vec![
                ExchangeType::Binance,
                ExchangeType::Coinbase,
                ExchangeType::Kraken,
                ExchangeType::Upbit,
                ExchangeType::Bithumb,
            ],
            price_tolerance_percent: 0.1, // 0.1% 허용 오차
            max_price_deviation_percent: 5.0, // 최대 5% 편차
            enable_arbitrage_detection: true,
        }
    }
}

/// 가격 동기화 결과
#[derive(Debug, Clone)]
pub struct PriceSyncResult {
    pub symbol: String,
    pub internal_price: f64,
    pub external_prices: HashMap<ExchangeType, f64>,
    pub price_deviations: HashMap<ExchangeType, f64>,
    pub arbitrage_opportunities: Vec<ArbitrageOpportunity>,
    pub sync_timestamp: u64,
}

/// 차익거래 기회
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub symbol: String,
    pub buy_exchange: ExchangeType,
    pub sell_exchange: ExchangeType,
    pub buy_price: f64,
    pub sell_price: f64,
    pub profit_percent: f64,
    pub estimated_profit: f64,
    pub timestamp: u64,
}

/// 외부 거래소 가격 동기화 관리자
pub struct ExternalPriceSyncManager {
    /// 설정
    config: PriceSyncConfig,
    /// 외부 가격 데이터 저장소
    external_prices: Arc<RwLock<HashMap<String, HashMap<ExchangeType, ExternalPriceData>>>>,
    /// 내부 가격 데이터 저장소
    internal_prices: Arc<RwLock<HashMap<String, f64>>>,
    /// 동기화 결과 저장소
    sync_results: Arc<RwLock<Vec<PriceSyncResult>>>,
    /// 동기화 활성화 상태
    is_syncing: Arc<Mutex<bool>>,
}

impl ExternalPriceSyncManager {
    /// 새 가격 동기화 관리자 생성
    pub fn new(config: PriceSyncConfig) -> Self {
        Self {
            config,
            external_prices: Arc::new(RwLock::new(HashMap::new())),
            internal_prices: Arc::new(RwLock::new(HashMap::new())),
            sync_results: Arc::new(RwLock::new(Vec::new())),
            is_syncing: Arc::new(Mutex::new(false)),
        }
    }

    /// 가격 동기화 시작
    pub async fn start_sync(&self) {
        let mut is_syncing = self.is_syncing.lock().await;
        if *is_syncing {
            warn!("가격 동기화가 이미 실행 중입니다");
            return;
        }
        *is_syncing = true;
        drop(is_syncing);

        info!("외부 거래소 가격 동기화 시작");

        let external_prices = self.external_prices.clone();
        let internal_prices = self.internal_prices.clone();
        let sync_results = self.sync_results.clone();
        let config = self.config.clone();
        let is_syncing = self.is_syncing.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.sync_interval_ms));

            loop {
                interval.tick().await;

                // 동기화 중단 확인
                {
                    let syncing = is_syncing.lock().await;
                    if !*syncing {
                        break;
                    }
                }

                // 각 심볼별로 동기화 수행
                for symbol in &config.symbols {
                    if let Err(e) = Self::sync_symbol_prices(
                        symbol,
                        &external_prices,
                        &internal_prices,
                        &sync_results,
                        &config,
                    ).await {
                        error!("심볼 {} 가격 동기화 실패: {}", symbol, e);
                    }
                }
            }

            info!("외부 거래소 가격 동기화 종료");
        });
    }

    /// 가격 동기화 중단
    pub async fn stop_sync(&self) {
        let mut is_syncing = self.is_syncing.lock().await;
        *is_syncing = false;
        info!("외부 거래소 가격 동기화 중단 요청");
    }

    /// 특정 심볼의 가격 동기화
    async fn sync_symbol_prices(
        symbol: &str,
        external_prices: &Arc<RwLock<HashMap<String, HashMap<ExchangeType, ExternalPriceData>>>>,
        internal_prices: &Arc<RwLock<HashMap<String, f64>>>,
        sync_results: &Arc<RwLock<Vec<PriceSyncResult>>>,
        config: &PriceSyncConfig,
    ) -> Result<(), String> {
        // 내부 가격 조회 (Mock)
        let internal_price = Self::get_internal_price(symbol).await?;
        
        // 내부 가격 업데이트
        {
            let mut prices = internal_prices.write().await;
            prices.insert(symbol.to_string(), internal_price);
        }

        // 외부 거래소 가격 조회
        let mut external_price_map = HashMap::new();
        let mut price_deviations = HashMap::new();
        let mut arbitrage_opportunities = Vec::new();

        for exchange in &config.exchanges {
            match Self::fetch_external_price(symbol, exchange).await {
                Ok(price_data) => {
                    external_price_map.insert(exchange.clone(), price_data.price);
                    
                    // 가격 편차 계산
                    let deviation = ((price_data.price - internal_price) / internal_price) * 100.0;
                    price_deviations.insert(exchange.clone(), deviation);
                    
                    debug!("{} {} 가격: 내부={:.2}, 외부({:?})={:.2}, 편차={:.2}%", 
                           symbol, exchange, internal_price, exchange, price_data.price, deviation);
                }
                Err(e) => {
                    warn!("{} {} 가격 조회 실패: {}", symbol, exchange, e);
                }
            }
        }

        // 차익거래 기회 탐지
        if config.enable_arbitrage_detection {
            arbitrage_opportunities = Self::detect_arbitrage_opportunities(
                symbol,
                &external_price_map,
                config,
            ).await;
        }

        // 동기화 결과 저장
        let sync_result = PriceSyncResult {
            symbol: symbol.to_string(),
            internal_price,
            external_prices: external_price_map.clone(),
            price_deviations,
            arbitrage_opportunities,
            sync_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };

        {
            let mut results = sync_results.write().await;
            results.push(sync_result);
            
            // 최대 1000개 결과만 유지
            if results.len() > 1000 {
                let excess = results.len() - 1000;
                results.drain(0..excess);
            }
        }

        Ok(())
    }

    /// 내부 가격 조회 (Mock)
    async fn get_internal_price(symbol: &str) -> Result<f64, String> {
        // Mock: 실제로는 내부 시스템에서 가격 조회
        sleep(Duration::from_millis(10)).await;
        
        let base_prices: HashMap<&str, f64> = [
            ("BTC-KRW", 50000000.0),
            ("ETH-KRW", 3000000.0),
            ("XRP-KRW", 500.0),
        ].iter().cloned().collect();
        
        let base_price = base_prices.get(symbol)
            .ok_or_else(|| format!("지원하지 않는 심볼: {}", symbol))?;
        
        // ±1% 랜덤 변동
        let variation = (fastrand::f32() - 0.5) * 0.02; // -1% ~ +1%
        let price = base_price * (1.0 + variation as f64);
        
        Ok(price)
    }

    /// 외부 거래소 가격 조회 (Mock)
    async fn fetch_external_price(symbol: &str, exchange: &ExchangeType) -> Result<ExternalPriceData, String> {
        // Mock: 실제로는 외부 API 호출
        sleep(Duration::from_millis(50 + fastrand::u32(0..100) as u64)).await; // 50-150ms 지연
        
        let base_prices: HashMap<&str, f64> = [
            ("BTC-KRW", 50000000.0),
            ("ETH-KRW", 3000000.0),
            ("XRP-KRW", 500.0),
        ].iter().cloned().collect();
        
        let base_price = base_prices.get(symbol)
            .ok_or_else(|| format!("지원하지 않는 심볼: {}", symbol))?;
        
        // 거래소별 가격 변동 시뮬레이션
        let exchange_multiplier = match exchange {
            ExchangeType::Binance => 1.001,  // +0.1%
            ExchangeType::Coinbase => 0.999, // -0.1%
            ExchangeType::Kraken => 1.002,  // +0.2%
            ExchangeType::Upbit => 0.998,   // -0.2%
            ExchangeType::Bithumb => 1.000,  // 동일
        };
        
        // 추가 랜덤 변동 (±0.5%)
        let random_variation = (fastrand::f32() - 0.5) * 0.01;
        let price = base_price * exchange_multiplier * (1.0 + random_variation as f64);
        
        // 매수/매도 호가 계산
        let spread_percent = 0.001; // 0.1% 스프레드
        let bid_price = price * (1.0 - spread_percent);
        let ask_price = price * (1.0 + spread_percent);
        let spread = ask_price - bid_price;
        
        Ok(ExternalPriceData {
            exchange: exchange.clone(),
            symbol: symbol.to_string(),
            price,
            volume: fastrand::f32() as f64 * 1000.0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            bid_price: Some(bid_price),
            ask_price: Some(ask_price),
            spread: Some(spread),
        })
    }

    /// 차익거래 기회 탐지
    async fn detect_arbitrage_opportunities(
        symbol: &str,
        external_prices: &HashMap<ExchangeType, f64>,
        config: &PriceSyncConfig,
    ) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();
        
        if external_prices.len() < 2 {
            return opportunities;
        }
        
        let exchanges: Vec<_> = external_prices.keys().collect();
        
        for i in 0..exchanges.len() {
            for j in i+1..exchanges.len() {
                let buy_exchange = exchanges[i];
                let sell_exchange = exchanges[j];
                
                let buy_price = external_prices[buy_exchange];
                let sell_price = external_prices[sell_exchange];
                
                // 매수 가격이 매도 가격보다 낮은 경우 차익거래 기회
                if buy_price < sell_price {
                    let profit_percent = ((sell_price - buy_price) / buy_price) * 100.0;
                    
                    // 최소 수익률 확인 (거래 수수료 고려)
                    if profit_percent > config.price_tolerance_percent {
                        let estimated_profit = sell_price - buy_price;
                        
                        opportunities.push(ArbitrageOpportunity {
                            symbol: symbol.to_string(),
                            buy_exchange: buy_exchange.clone(),
                            sell_exchange: sell_exchange.clone(),
                            buy_price,
                            sell_price,
                            profit_percent,
                            estimated_profit,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        });
                    }
                }
            }
        }
        
        // 수익률 기준으로 정렬
        opportunities.sort_by(|a, b| b.profit_percent.partial_cmp(&a.profit_percent).unwrap());
        
        opportunities
    }

    /// 동기화 결과 조회
    pub async fn get_sync_results(&self, limit: Option<usize>) -> Vec<PriceSyncResult> {
        let results = self.sync_results.read().await;
        if let Some(limit) = limit {
            results.iter().rev().take(limit).cloned().collect()
        } else {
            results.clone()
        }
    }

    /// 특정 심볼의 최신 동기화 결과 조회
    pub async fn get_latest_sync_result(&self, symbol: &str) -> Option<PriceSyncResult> {
        let results = self.sync_results.read().await;
        results.iter().rev().find(|r| r.symbol == symbol).cloned()
    }

    /// 차익거래 기회 조회
    pub async fn get_arbitrage_opportunities(&self, min_profit_percent: Option<f64>) -> Vec<ArbitrageOpportunity> {
        let results = self.sync_results.read().await;
        let mut opportunities = Vec::new();
        
        for result in results.iter() {
            for opportunity in &result.arbitrage_opportunities {
                if let Some(min_profit) = min_profit_percent {
                    if opportunity.profit_percent >= min_profit {
                        opportunities.push(opportunity.clone());
                    }
                } else {
                    opportunities.push(opportunity.clone());
                }
            }
        }
        
        // 수익률 기준으로 정렬
        opportunities.sort_by(|a, b| b.profit_percent.partial_cmp(&a.profit_percent).unwrap());
        
        opportunities
    }

    /// 동기화 통계 조회
    pub async fn get_sync_stats(&self) -> PriceSyncStats {
        let external_prices = self.external_prices.read().await;
        let internal_prices = self.internal_prices.read().await;
        let sync_results = self.sync_results.read().await;
        
        let mut total_arbitrage_opportunities = 0;
        let mut max_profit_percent = 0.0;
        
        for result in sync_results.iter() {
            total_arbitrage_opportunities += result.arbitrage_opportunities.len();
            for opportunity in &result.arbitrage_opportunities {
                if opportunity.profit_percent > max_profit_percent {
                    max_profit_percent = opportunity.profit_percent;
                }
            }
        }
        
        PriceSyncStats {
            symbols_synced: internal_prices.len(),
            exchanges_monitored: self.config.exchanges.len(),
            total_sync_results: sync_results.len(),
            total_arbitrage_opportunities,
            max_profit_percent,
            is_syncing: *self.is_syncing.lock().await,
        }
    }
}

/// 가격 동기화 통계
#[derive(Debug, Clone)]
pub struct PriceSyncStats {
    pub symbols_synced: usize,
    pub exchanges_monitored: usize,
    pub total_sync_results: usize,
    pub total_arbitrage_opportunities: usize,
    pub max_profit_percent: f64,
    pub is_syncing: bool,
}

// fastrand 의존성을 위해 간단한 랜덤 함수 구현
mod fastrand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn f32() -> f32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        (hash % 1000) as f32 / 1000.0
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
    async fn test_price_sync_manager_creation() {
        let config = PriceSyncConfig::default();
        let manager = ExternalPriceSyncManager::new(config);
        
        let stats = manager.get_sync_stats().await;
        assert_eq!(stats.symbols_synced, 0);
        assert_eq!(stats.exchanges_monitored, 5);
        assert!(!stats.is_syncing);
    }

    #[tokio::test]
    async fn test_internal_price_fetch() {
        let price = ExternalPriceSyncManager::get_internal_price("BTC-KRW").await.unwrap();
        assert!(price > 0.0);
        assert!(price >= 49500000.0 && price <= 50500000.0); // ±1% 범위
    }

    #[tokio::test]
    async fn test_external_price_fetch() {
        let price_data = ExternalPriceSyncManager::fetch_external_price("BTC-KRW", &ExchangeType::Binance).await.unwrap();
        assert_eq!(price_data.exchange, ExchangeType::Binance);
        assert_eq!(price_data.symbol, "BTC-KRW");
        assert!(price_data.price > 0.0);
        assert!(price_data.bid_price.is_some());
        assert!(price_data.ask_price.is_some());
        assert!(price_data.spread.is_some());
    }

    #[tokio::test]
    async fn test_arbitrage_detection() {
        let mut external_prices = HashMap::new();
        external_prices.insert(ExchangeType::Binance, 50000000.0);
        external_prices.insert(ExchangeType::Coinbase, 50100000.0);
        
        let config = PriceSyncConfig::default();
        let opportunities = ExternalPriceSyncManager::detect_arbitrage_opportunities(
            "BTC-KRW",
            &external_prices,
            &config,
        ).await;
        
        assert!(!opportunities.is_empty());
        assert_eq!(opportunities[0].buy_exchange, ExchangeType::Binance);
        assert_eq!(opportunities[0].sell_exchange, ExchangeType::Coinbase);
        assert!(opportunities[0].profit_percent > 0.0);
    }

    #[tokio::test]
    async fn test_price_sync_config() {
        let config = PriceSyncConfig {
            sync_interval_ms: 2000,
            symbols: vec!["BTC-KRW".to_string()],
            exchanges: vec![ExchangeType::Binance],
            price_tolerance_percent: 0.5,
            max_price_deviation_percent: 10.0,
            enable_arbitrage_detection: false,
        };
        
        assert_eq!(config.sync_interval_ms, 2000);
        assert_eq!(config.symbols.len(), 1);
        assert_eq!(config.exchanges.len(), 1);
        assert_eq!(config.price_tolerance_percent, 0.5);
        assert!(!config.enable_arbitrage_detection);
    }
}