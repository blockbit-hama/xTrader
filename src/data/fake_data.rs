use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::matching_engine::model::{Order, OrderType, Side};

/// 가짜 시장 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FakeMarketData {
    pub symbol: String,
    pub base_price: u64,
    pub volatility: f64, // 가격 변동성 (0.01 = 1%)
    pub volume_range: (u64, u64), // (최소, 최대) 수량
    pub price_range: (u64, u64), // (최소, 최대) 가격 범위
}

/// 가짜 주문 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FakeOrderData {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price_range: (u64, u64),
    pub quantity_range: (u64, u64),
    pub frequency_ms: u64, // 주문 생성 빈도 (밀리초)
}

/// 가짜 데이터셋
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FakeDataset {
    pub market_data: HashMap<String, FakeMarketData>,
    pub order_templates: Vec<FakeOrderData>,
}

impl Default for FakeDataset {
    fn default() -> Self {
        let mut market_data = HashMap::new();
        
        // BTC-KRW 데이터
        market_data.insert("BTC-KRW".to_string(), FakeMarketData {
            symbol: "BTC-KRW".to_string(),
            base_price: 100_000_000, // 1억원
            volatility: 0.02, // 2% 변동성
            volume_range: (1, 100), // 1~100 BTC
            price_range: (95_000_000, 105_000_000), // 9500만~1억500만원
        });
        
        // ETH-KRW 데이터
        market_data.insert("ETH-KRW".to_string(), FakeMarketData {
            symbol: "ETH-KRW".to_string(),
            base_price: 4_000_000, // 400만원
            volatility: 0.03, // 3% 변동성
            volume_range: (1, 50), // 1~50 ETH
            price_range: (3_800_000, 4_200_000), // 380만~420만원
        });
        
        // AAPL 데이터
        market_data.insert("AAPL".to_string(), FakeMarketData {
            symbol: "AAPL".to_string(),
            base_price: 200_000, // 20만원
            volatility: 0.015, // 1.5% 변동성
            volume_range: (10, 1000), // 10~1000주
            price_range: (190_000, 210_000), // 19만~21만원
        });
        
        // 주문 템플릿
        let order_templates = vec![
            // BTC 매수 주문 (높은 빈도)
            FakeOrderData {
                symbol: "BTC-KRW".to_string(),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price_range: (98_000_000, 102_000_000),
                quantity_range: (1, 10),
                frequency_ms: 2000, // 2초마다
            },
            // BTC 매도 주문
            FakeOrderData {
                symbol: "BTC-KRW".to_string(),
                side: Side::Sell,
                order_type: OrderType::Limit,
                price_range: (98_000_000, 102_000_000),
                quantity_range: (1, 8),
                frequency_ms: 3000, // 3초마다
            },
            // ETH 매수 주문
            FakeOrderData {
                symbol: "ETH-KRW".to_string(),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price_range: (3_900_000, 4_100_000),
                quantity_range: (1, 20),
                frequency_ms: 2500, // 2.5초마다
            },
            // ETH 매도 주문
            FakeOrderData {
                symbol: "ETH-KRW".to_string(),
                side: Side::Sell,
                order_type: OrderType::Limit,
                price_range: (3_900_000, 4_100_000),
                quantity_range: (1, 15),
                frequency_ms: 3500, // 3.5초마다
            },
            // AAPL 매수 주문
            FakeOrderData {
                symbol: "AAPL".to_string(),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price_range: (195_000, 205_000),
                quantity_range: (50, 500),
                frequency_ms: 1500, // 1.5초마다
            },
            // AAPL 매도 주문
            FakeOrderData {
                symbol: "AAPL".to_string(),
                side: Side::Sell,
                order_type: OrderType::Limit,
                price_range: (195_000, 205_000),
                quantity_range: (50, 400),
                frequency_ms: 2000, // 2초마다
            },
            // 시장가 주문 (가끔)
            FakeOrderData {
                symbol: "BTC-KRW".to_string(),
                side: Side::Buy,
                order_type: OrderType::Market,
                price_range: (0, 0), // 시장가는 가격 무의미
                quantity_range: (1, 5),
                frequency_ms: 10000, // 10초마다
            },
            FakeOrderData {
                symbol: "BTC-KRW".to_string(),
                side: Side::Sell,
                order_type: OrderType::Market,
                price_range: (0, 0),
                quantity_range: (1, 3),
                frequency_ms: 12000, // 12초마다
            },
        ];
        
        Self {
            market_data,
            order_templates,
        }
    }
}

impl FakeDataset {
    /// 데이터셋을 JSON 파일로 저장
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    /// JSON 파일에서 데이터셋 로드
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let dataset: FakeDataset = serde_json::from_str(&json)?;
        Ok(dataset)
    }
    
    /// 심볼에 대한 시장 데이터 가져오기
    pub fn get_market_data(&self, symbol: &str) -> Option<&FakeMarketData> {
        self.market_data.get(symbol)
    }
    
    /// 주문 템플릿 가져오기
    pub fn get_order_templates(&self) -> &[FakeOrderData] {
        &self.order_templates
    }
}