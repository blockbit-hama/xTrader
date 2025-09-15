use std::time::Duration;
use tokio::time;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;
use rand::{Rng, thread_rng};
use log::{info, warn, error};

#[derive(Debug, Clone)]
struct OrderTemplate {
    symbol: String,
    side: String,
    order_type: String,
    price_range: (u64, u64),
    quantity_range: (u64, u64),
    frequency_ms: u64,
}

impl OrderTemplate {
    fn new(symbol: &str, side: &str, order_type: &str, price_range: (u64, u64), quantity_range: (u64, u64), frequency_ms: u64) -> Self {
        Self {
            symbol: symbol.to_string(),
            side: side.to_string(),
            order_type: order_type.to_string(),
            price_range,
            quantity_range,
            frequency_ms,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // 로깅 초기화
    env_logger::init();
    
    println!("xTrader 시뮬레이션 프로그램 시작");
    
    // API 클라이언트 생성
    let client = Client::new();
    let api_base_url = "http://localhost:7000";
    
    // 실전적인 주문 템플릿 생성 (현실적인 가격대와 패턴)
    let order_templates = vec![
        // BTC-KRW: 고빈도 스캘핑 (좁은 스프레드)
        OrderTemplate::new("BTC-KRW", "Buy", "Limit", (99_800_000, 100_000_000), (1, 5), 1000),
        OrderTemplate::new("BTC-KRW", "Sell", "Limit", (100_000_000, 100_200_000), (1, 5), 1200),

        // BTC-KRW: 중간 규모 거래
        OrderTemplate::new("BTC-KRW", "Buy", "Limit", (99_500_000, 100_500_000), (5, 20), 3000),
        OrderTemplate::new("BTC-KRW", "Sell", "Limit", (99_500_000, 100_500_000), (5, 15), 3500),

        // ETH-KRW: 알트코인 거래 패턴
        OrderTemplate::new("ETH-KRW", "Buy", "Limit", (3_980_000, 4_020_000), (10, 50), 2500),
        OrderTemplate::new("ETH-KRW", "Sell", "Limit", (3_980_000, 4_020_000), (8, 40), 2800),

        // AAPL: 미국 주식 거래 시간 패턴
        OrderTemplate::new("AAPL", "Buy", "Limit", (199_000, 201_000), (100, 1000), 5000),
        OrderTemplate::new("AAPL", "Sell", "Limit", (199_000, 201_000), (100, 800), 5500),

        // 시장가 주문 (급한 거래)
        OrderTemplate::new("BTC-KRW", "Buy", "Market", (0, 0), (1, 3), 15000),
        OrderTemplate::new("BTC-KRW", "Sell", "Market", (0, 0), (1, 2), 18000),
        OrderTemplate::new("ETH-KRW", "Buy", "Market", (0, 0), (5, 15), 20000),

        // 대량 거래 (기관투자자 패턴)
        OrderTemplate::new("BTC-KRW", "Buy", "Limit", (99_000_000, 101_000_000), (20, 100), 30000),
        OrderTemplate::new("ETH-KRW", "Buy", "Limit", (3_900_000, 4_100_000), (100, 500), 25000),
    ];
    
    // 각 템플릿에 대해 스케줄러 시작
    let mut handles = Vec::new();
    
    for template in order_templates {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            simulate_orders(client_clone, api_base_url, template).await;
        });
        handles.push(handle);
    }
    
    // 모든 스케줄러가 실행되도록 대기
    for handle in handles {
        handle.await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

async fn simulate_orders(client: Client, api_base_url: &str, template: OrderTemplate) {
    let mut interval = time::interval(Duration::from_millis(template.frequency_ms));
    
    println!("시뮬레이션 시작: {} {} {} ({}ms 간격)", 
             template.symbol, template.side, template.order_type, template.frequency_ms);
    
    loop {
        interval.tick().await;
        
        // 주문 생성
        let order = create_random_order(&template);
        
        // API 호출
        let order_result = submit_order(&client, api_base_url, &order).await;
        let order_symbol = order["symbol"].as_str().unwrap_or("UNKNOWN").to_string();

        match order_result {
            Ok(_response) => {
                info!("✅ 주문 성공: {} {} {} - 수량: {}, 가격: ₩{}",
                     order["symbol"], order["side"], order["order_type"],
                     order["quantity"], order["price"]);
            }
            Err(e) => {
                let error_str = e.to_string();
                warn!("❌ 주문 실패: {} - {}", order_symbol, error_str);
                // 서버가 다운된 경우 잠시 대기
                time::sleep(Duration::from_secs(5)).await;
            }
        }
        
        // 잠시 대기 (API 서버 부하 방지)
        time::sleep(Duration::from_millis(100)).await;
    }
}

fn create_random_order(template: &OrderTemplate) -> serde_json::Value {
    let order_id = Uuid::new_v4().to_string();
    let mut rng = thread_rng();
    let client_id = format!("simulator_{}", rng.gen_range(1..=10));
    
    let price = if template.order_type == "Market" {
        0 // 시장가는 가격 0
    } else {
        rng.gen_range(template.price_range.0..=template.price_range.1)
    };
    
    let quantity = rng.gen_range(template.quantity_range.0..=template.quantity_range.1);
    
    json!({
        "symbol": template.symbol,
        "side": template.side,
        "order_type": template.order_type,
        "price": price,
        "quantity": quantity,
        "client_id": client_id
    })
}

async fn submit_order(client: &Client, api_base_url: &str, order: &serde_json::Value) -> Result<String, String> {
    let url = format!("{}/v1/order", api_base_url);

    let response = client
        .post(&url)
        .json(order)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        let response_text = response.text().await.map_err(|e| e.to_string())?;
        Ok(response_text)
    } else {
        let status = response.status();
        let error_text = response.text().await.map_err(|e| e.to_string())?;
        Err(format!("HTTP {}: {}", status, error_text))
    }
}