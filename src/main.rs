use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

mod matching_engine;
mod util;

use crate::matching_engine::engine::MatchingEngine;
use crate::matching_engine::model::{Order, OrderType, Side, ExecutionReport};


#[derive(Clone)]
pub struct Config {
    pub ws_port: u16,
    pub rest_port: u16,
    pub symbols: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ws_port: 8080,
            rest_port: 8081,
            symbols: vec!["AAPL".into(), "MSFT".into(), "GOOG".into()],
        }
    }
}

// 시장 데이터 이벤트 타입 (임시)
#[derive(Debug)]
pub enum MarketDataEvent {
    OrderBookUpdate { symbol: String, bids: Vec<(u64, u64)>, asks: Vec<(u64, u64)> },
    Trade { symbol: String, price: u64, quantity: u64, timestamp: u64 },
}

// 테스트용 주문 생성 헬퍼 함수
fn create_test_order(id: &str, symbol: &str, side: Side, order_type: OrderType, price: u64, quantity: u64) -> Order {
    Order {
        id: id.to_string(),
        symbol: symbol.to_string(),
        side,
        order_type,
        price,
        quantity,
        remaining_quantity: quantity,
        client_id: "".to_string(),
        timestamp: 0,
        is_cancel: false,
        target_order_id: None,
    }
}

// 취소 주문 생성 헬퍼 함수
fn create_cancel_order(id: &str, symbol: &str, target_id: &str) -> Order {
    Order {
        id: id.to_string(),
        symbol: symbol.to_string(),
        side: Side::Buy,  // 취소 주문은 side가 중요하지 않음
        order_type: OrderType::Limit,
        price: 0,
        quantity: 0,
        remaining_quantity: 0,
        client_id: "".to_string(),
        timestamp: 0,
        is_cancel: true,
        target_order_id: Some(target_id.to_string()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("주문 매칭 시스템 시작 (통합 테스트 모드)");
    
    // 설정 로드
    let config = Config::default();
    
    // 채널 생성
    let (order_tx, order_rx) = mpsc::channel::<Order>();
    let (exec_tx, exec_rx) = mpsc::channel::<ExecutionReport>();
    let (md_tx, md_rx) = mpsc::channel::<MarketDataEvent>();
    
    // 체결 보고서를 수집할 벡터
    let exec_reports = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let exec_reports_clone = exec_reports.clone();
    
    // 체결 엔진 스레드 실행
    let engine_handle = {
        let config = config.clone();
        let exec_tx = exec_tx.clone();
        thread::spawn(move || {
            let mut engine = MatchingEngine::new(config.symbols, exec_tx);
            engine.run(order_rx);
        })
    };
    
    // 체결 보고서 처리 스레드
    let exec_report_handle = thread::spawn(move || {
        while let Ok(report) = exec_rx.recv() {
            println!("체결 보고서 수신: Order {} (수량: {}, 가격: {}, 남은수량: {})",
                     report.order_id, report.quantity, report.price, report.remaining_quantity);
            
            // 체결 보고서 저장
            let mut reports = exec_reports_clone.lock().unwrap();
            reports.push(report);
        }
    });
    
    // 시장 데이터 처리 스레드
    let market_data_handle = thread::spawn(move || {
        while let Ok(event) = md_rx.recv() {
            println!("시장 데이터 이벤트 수신: {:?}", event);
        }
    });
    
    // 테스트 시나리오 실행
    run_integration_tests(&order_tx);
    
    // 테스트 완료 후 스레드 종료를 위해 채널 닫기
    // 참고: 이 예제에서는 실제로 채널을 명시적으로 닫을 수 없지만,
    // 프로덕션 코드에서는 적절한 종료 시그널을 보내야 합니다.
    drop(order_tx);
    drop(md_tx);
    
    // 모든 스레드가 종료되기를 기다림 (실제 코드에서는 timeout 추가 필요)
    exec_report_handle.join().unwrap();
    market_data_handle.join().unwrap();
    engine_handle.join().unwrap();
    
    println!("모든 테스트 완료, 프로그램 종료.");
    
    Ok(())
}

/// 통합 테스트 시나리오 실행
fn run_integration_tests(order_tx: &mpsc::Sender<Order>) {
    println!("\n===== 통합 테스트 시작 =====\n");
    
    // 테스트 1: 기본 주문 체결 (매수/매도 완전 매칭)
    test_basic_order_matching(order_tx);
    thread::sleep(Duration::from_millis(100)); // 처리 시간 기다림
    
    // 테스트 2: 부분 체결
    test_partial_order_matching(order_tx);
    thread::sleep(Duration::from_millis(100));
    
    // 테스트 3: 여러 가격 레벨에서의 매칭
    test_multiple_price_levels(order_tx);
    thread::sleep(Duration::from_millis(100));
    
    // 테스트 4: 주문 취소
    test_order_cancellation(order_tx);
    thread::sleep(Duration::from_millis(100));
    
    // 테스트 5: 시장가 주문
    test_market_orders(order_tx);
    thread::sleep(Duration::from_millis(100));
    
    // 테스트 6: 대량 주문 (스트레스 테스트)
    test_order_stress(order_tx);
    thread::sleep(Duration::from_millis(200));
    
    println!("\n===== 통합 테스트 완료 =====\n");
}

/// 테스트 1: 기본 주문 체결 (매수/매도 완전 매칭)
fn test_basic_order_matching(order_tx: &mpsc::Sender<Order>) {
    println!("테스트 1: 기본 주문 체결");
    
    // 매도 주문 (매도 10,000원에 100주)
    let sell_order = create_test_order(
        "sell-basic-1",
        "BTC-KRW",
        Side::Sell,
        OrderType::Limit,
        10000,
        100
    );
    order_tx.send(sell_order).unwrap();
    thread::sleep(Duration::from_millis(10));
    
    // 매수 주문 (매수 10,000원에 100주) - 완전 매칭
    let buy_order = create_test_order(
        "buy-basic-1",
        "BTC-KRW",
        Side::Buy,
        OrderType::Limit,
        10000,
        100
    );
    order_tx.send(buy_order).unwrap();
    
    println!("테스트 1 주문 전송 완료.");
}

/// 테스트 2: 부분 체결
fn test_partial_order_matching(order_tx: &mpsc::Sender<Order>) {
    println!("테스트 2: 부분 체결");
    
    // 매도 주문 (매도 10,100원에 200주)
    let sell_order = create_test_order(
        "sell-partial-1",
        "BTC-KRW",
        Side::Sell,
        OrderType::Limit,
        10100,
        200
    );
    order_tx.send(sell_order).unwrap();
    thread::sleep(Duration::from_millis(10));
    
    // 매수 주문 (매수 10,100원에 50주) - 부분 체결 (200주 중 50주)
    let buy_order1 = create_test_order(
        "buy-partial-1",
        "BTC-KRW",
        Side::Buy,
        OrderType::Limit,
        10100,
        50
    );
    order_tx.send(buy_order1).unwrap();
    thread::sleep(Duration::from_millis(10));
    
    // 추가 매수 주문 (매수 10,100원에 70주) - 추가 부분 체결 (150주 중 70주)
    let buy_order2 = create_test_order(
        "buy-partial-2",
        "BTC-KRW",
        Side::Buy,
        OrderType::Limit,
        10100,
        70
    );
    order_tx.send(buy_order2).unwrap();
    
    println!("테스트 2 주문 전송 완료.");
}

/// 테스트 3: 여러 가격 레벨에서의 매칭
fn test_multiple_price_levels(order_tx: &mpsc::Sender<Order>) {
    println!("테스트 3: 여러 가격 레벨에서의 매칭");
    
    // 여러 가격의 매도 주문 추가
    let sell_orders = vec![
        create_test_order("sell-multi-1", "ETH-KRW", Side::Sell, OrderType::Limit, 2000000, 1),
        create_test_order("sell-multi-2", "ETH-KRW", Side::Sell, OrderType::Limit, 2010000, 2),
        create_test_order("sell-multi-3", "ETH-KRW", Side::Sell, OrderType::Limit, 2020000, 3),
    ];
    
    for order in sell_orders {
        order_tx.send(order).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    
    // 여러 가격의 매수 주문 추가
    let buy_orders = vec![
        create_test_order("buy-multi-1", "ETH-KRW", Side::Buy, OrderType::Limit, 1980000, 1),
        create_test_order("buy-multi-2", "ETH-KRW", Side::Buy, OrderType::Limit, 1990000, 2),
        create_test_order("buy-multi-3", "ETH-KRW", Side::Buy, OrderType::Limit, 2000000, 3),
    ];
    
    for order in buy_orders {
        order_tx.send(order).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    
    println!("테스트 3 주문 전송 완료.");
}

/// 테스트 4: 주문 취소
fn test_order_cancellation(order_tx: &mpsc::Sender<Order>) {
    println!("테스트 4: 주문 취소");
    
    // 취소 대상 주문 추가
    let sell_order = create_test_order(
        "sell-cancel-target",
        "BTC-KRW",
        Side::Sell,
        OrderType::Limit,
        10500,
        50
    );
    order_tx.send(sell_order).unwrap();
    thread::sleep(Duration::from_millis(10));
    
    // 취소 주문 전송
    let cancel_order = create_cancel_order(
        "cancel-1",
        "BTC-KRW",
        "sell-cancel-target"
    );
    order_tx.send(cancel_order).unwrap();
    
    println!("테스트 4 주문 전송 완료.");
}

/// 테스트 5: 시장가 주문
fn test_market_orders(order_tx: &mpsc::Sender<Order>) {
    println!("테스트 5: 시장가 주문");
    
    // 먼저 매도 주문 추가 (시장가 매수 테스트용)
    let sell_orders = vec![
        create_test_order("sell-market-1", "BTC-KRW", Side::Sell, OrderType::Limit, 10200, 10),
        create_test_order("sell-market-2", "BTC-KRW", Side::Sell, OrderType::Limit, 10300, 20),
        create_test_order("sell-market-3", "BTC-KRW", Side::Sell, OrderType::Limit, 10400, 30),
    ];
    
    for order in sell_orders {
        order_tx.send(order).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    
    // 시장가 매수 주문 (40주 - 최저가부터 체결)
    let market_buy = create_test_order(
        "buy-market-1",
        "BTC-KRW",
        Side::Buy,
        OrderType::Market,
        0,  // 시장가는 가격 필요 없음 
        40
    );
    order_tx.send(market_buy).unwrap();
    thread::sleep(Duration::from_millis(20));
    
    // 매수 주문 추가 (시장가 매도 테스트용)
    let buy_orders = vec![
        create_test_order("buy-market-limit-1", "ETH-KRW", Side::Buy, OrderType::Limit, 2050000, 1),
        create_test_order("buy-market-limit-2", "ETH-KRW", Side::Buy, OrderType::Limit, 2040000, 1),
        create_test_order("buy-market-limit-3", "ETH-KRW", Side::Buy, OrderType::Limit, 2030000, 1),
    ];
    
    for order in buy_orders {
        order_tx.send(order).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    
    // 시장가 매도 주문 (2주 - 최고가부터 체결)
    let market_sell = create_test_order(
        "sell-market-1",
        "ETH-KRW",
        Side::Sell,
        OrderType::Market,
        0,  // 시장가는 가격 필요 없음 
        2
    );
    order_tx.send(market_sell).unwrap();
    
    println!("테스트 5 주문 전송 완료.");
}

/// 테스트 6: 대량 주문 (스트레스 테스트)
fn test_order_stress(order_tx: &mpsc::Sender<Order>) {
    println!("테스트 6: 대량 주문 (스트레스 테스트)");
    
    // 같은 가격 레벨에 다수의 주문 추가 (10,000원에 매도 100주 * 5개)
    for i in 1..=5 {
        let order_id = format!("sell-stress-{}", i);
        let sell_order = create_test_order(
            &order_id,
            "BTC-KRW",
            Side::Sell,
            OrderType::Limit,
            10000,
            100
        );
        order_tx.send(sell_order).unwrap();
        thread::sleep(Duration::from_millis(2));
    }
    
    // 대량 매수 주문 하나로 여러 주문 체결 (10,000원에 매수 430주)
    // 4개의 매도 주문이 완전 체결되고, 1개는 부분 체결됨
    let buy_order = create_test_order(
        "buy-stress-1",
        "BTC-KRW",
        Side::Buy,
        OrderType::Limit,
        10000,
        430
    );
    order_tx.send(buy_order).unwrap();
    
    println!("테스트 6 주문 전송 완료.");
}