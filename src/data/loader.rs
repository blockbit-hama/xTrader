use std::path::Path;
use std::fs;
use log::{info, warn, error};
use serde_json;
use crate::data::FakeDataset;
use crate::matching_engine::model::{Order, Side, OrderType};

/// 데이터 로더
pub struct DataLoader;

impl DataLoader {
    /// 실전적 데이터셋 로드 및 초기화
    pub fn load_dataset(data_path: &str) -> Result<FakeDataset, Box<dyn std::error::Error>> {
        if Path::new(data_path).exists() {
            info!("📂 데이터셋 파일 발견: {}", data_path);
            Self::load_from_json(data_path)
        } else {
            warn!("📂 데이터셋 파일이 없음: {}, 기본값 생성", data_path);
            Self::create_default_dataset(data_path)
        }
    }

    /// JSON 파일에서 데이터셋 로드
    fn load_from_json(data_path: &str) -> Result<FakeDataset, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(data_path)?;
        let json_data: serde_json::Value = serde_json::from_str(&content)?;

        info!("✅ JSON 데이터 파싱 완료");
        info!("📊 지원 심볼: {:?}",
            json_data["market_data"].as_object()
                .map(|obj| obj.keys().collect::<Vec<_>>())
                .unwrap_or_default());

        // 기존 FakeDataset과 호환되도록 변환
        let mut market_data = std::collections::HashMap::new();

        if let Some(market_obj) = json_data["market_data"].as_object() {
            for (symbol, data) in market_obj {
                market_data.insert(symbol.clone(), crate::data::FakeMarketData {
                    symbol: data["symbol"].as_str().unwrap_or(symbol).to_string(),
                    base_price: data["base_price"].as_u64().unwrap_or(100000000),
                    volatility: data["volatility"].as_f64().unwrap_or(0.02),
                    volume_range: (
                        data["volume_range"][0].as_u64().unwrap_or(1),
                        data["volume_range"][1].as_u64().unwrap_or(100)
                    ),
                    price_range: (
                        data["price_range"][0].as_u64().unwrap_or(95000000),
                        data["price_range"][1].as_u64().unwrap_or(105000000)
                    ),
                });
            }
        }

        let dataset = FakeDataset {
            market_data,
            order_templates: vec![], // 기존 구조와 호환
        };

        Ok(dataset)
    }

    /// 초기 주문서 생성 (실전적인 깊이 제공)
    pub fn create_initial_orders(json_data: &serde_json::Value) -> Vec<Order> {
        let mut orders = Vec::new();
        let mut order_id = 1;

        if let Some(orderbook_obj) = json_data["initial_orderbook"].as_object() {
            for (symbol, book_data) in orderbook_obj {
                info!("📋 {}의 초기 주문서 생성 중...", symbol);

                // 매수 주문 생성 (bids)
                if let Some(bids) = book_data["bids"].as_array() {
                    for bid in bids {
                        let price = bid[0].as_u64().unwrap_or(0);
                        let quantity = bid[1].as_u64().unwrap_or(0);

                        orders.push(Order::new(
                            format!("init_bid_{}", order_id),
                            symbol.clone(),
                            Side::Buy,
                            OrderType::Limit,
                            price,
                            quantity,
                            "market_maker".to_string(),
                        ));
                        order_id += 1;
                    }
                }

                // 매도 주문 생성 (asks)
                if let Some(asks) = book_data["asks"].as_array() {
                    for ask in asks {
                        let price = ask[0].as_u64().unwrap_or(0);
                        let quantity = ask[1].as_u64().unwrap_or(0);

                        orders.push(Order::new(
                            format!("init_ask_{}", order_id),
                            symbol.clone(),
                            Side::Sell,
                            OrderType::Limit,
                            price,
                            quantity,
                            "market_maker".to_string(),
                        ));
                        order_id += 1;
                    }
                }
            }
        }

        info!("✅ 초기 주문 {}개 생성 완료", orders.len());
        orders
    }
    
    /// 기본 데이터셋 생성 및 저장
    fn create_default_dataset(data_path: &str) -> Result<FakeDataset, Box<dyn std::error::Error>> {
        let dataset = FakeDataset::default();

        // 디렉토리 생성
        if let Some(parent) = Path::new(data_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    error!("📁 디렉토리 생성 실패: {}", e);
                    e
                })?;
            }
        }

        // 파일 저장 시도
        match dataset.save_to_file(data_path) {
            Ok(_) => info!("✅ 기본 데이터셋 생성 완료: {}", data_path),
            Err(e) => warn!("⚠️ 데이터셋 저장 실패: {}", e),
        }

        Ok(dataset)
    }

    /// 실전적인 가짜 사용자 로드
    pub fn load_fake_users(data_path: &str) -> Vec<FakeUser> {
        let content = match fs::read_to_string(data_path) {
            Ok(content) => content,
            Err(_) => {
                warn!("가짜 사용자 데이터 로드 실패, 기본값 사용");
                return Self::create_default_users();
            }
        };

        let json_data: serde_json::Value = match serde_json::from_str(&content) {
            Ok(data) => data,
            Err(_) => return Self::create_default_users(),
        };

        let mut users = Vec::new();
        if let Some(fake_users) = json_data["fake_users"].as_array() {
            for user in fake_users {
                users.push(FakeUser {
                    id: user["id"].as_str().unwrap_or("unknown").to_string(),
                    name: user["name"].as_str().unwrap_or("Unknown").to_string(),
                    user_type: user["type"].as_str().unwrap_or("retail").to_string(),
                    trading_style: user["trading_style"].as_str().unwrap_or("swing").to_string(),
                    balance: user["balance"].as_u64().unwrap_or(10000000),
                });
            }
        }

        info!("👥 가짜 사용자 {}명 로드 완료", users.len());
        users
    }

    fn create_default_users() -> Vec<FakeUser> {
        vec![
            FakeUser {
                id: "user_001".to_string(),
                name: "김투자".to_string(),
                user_type: "retail".to_string(),
                trading_style: "scalping".to_string(),
                balance: 10_000_000_000, // 100억원 (테스트용)
            },
            FakeUser {
                id: "user_002".to_string(),
                name: "박트레이더".to_string(),
                user_type: "retail".to_string(),
                trading_style: "swing".to_string(),
                balance: 20_000_000_000, // 200억원 (테스트용)
            },
            FakeUser {
                id: "test_user_001".to_string(),
                name: "테스트사용자".to_string(),
                user_type: "retail".to_string(),
                trading_style: "day_trading".to_string(),
                balance: 50_000_000_000, // 500억원 (테스트용)
            }
        ]
    }
}

/// 가짜 사용자 구조체
#[derive(Debug, Clone)]
pub struct FakeUser {
    pub id: String,
    pub name: String,
    pub user_type: String,      // retail, institutional
    pub trading_style: String,  // scalping, swing, day_trading, etc.
    pub balance: u64,
}