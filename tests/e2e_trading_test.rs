use std::time::Duration;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use serde::Serialize;
use reqwest::Client;

// Test configuration
const BASE_URL: &str = "http://127.0.0.1:7000";
const WS_URL: &str = "ws://127.0.0.1:7000/ws";
const TEST_SYMBOL: &str = "BTC-KRW";
const TEST_CLIENT_ID: &str = "test_user_e2e";

// Test data structures
#[derive(Debug, Clone, Serialize)]
struct TestOrder {
    symbol: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    quantity: u64,
    price: Option<u64>,
    client_id: String,
}

#[derive(Debug)]
struct TestExecution {
    execution_id: String,
    order_id: String,
    symbol: String,
    side: String,
    price: u64,
    quantity: u64,
    timestamp: u64,
}

#[derive(Debug)]
struct TestOrderBook {
    symbol: String,
    bids: Vec<(u64, u64)>, // (price, volume)
    asks: Vec<(u64, u64)>,
    timestamp: u64,
}

#[derive(Debug)]
struct TestCandle {
    open_time: u64,
    close_time: u64,
    open: u64,
    high: u64,
    low: u64,
    close: u64,
    volume: u64,
    trade_count: u64,
}

// Test results tracking
#[derive(Debug, Default)]
struct TestResults {
    orders_submitted: u32,
    orders_accepted: u32,
    executions_received: u32,
    orderbook_updates: u32,
    candle_updates: u32,
    websocket_messages: u32,
    errors: Vec<String>,
}

impl TestResults {
    fn add_error(&mut self, error: String) {
        println!("❌ ERROR: {}", error);
        self.errors.push(error);
    }
    
    fn print_summary(&self) {
        println!("\n📊 E2E Test Summary:");
        println!("  Orders Submitted: {}", self.orders_submitted);
        println!("  Orders Accepted: {}", self.orders_accepted);
        println!("  Executions Received: {}", self.executions_received);
        println!("  OrderBook Updates: {}", self.orderbook_updates);
        println!("  Candle Updates: {}", self.candle_updates);
        println!("  WebSocket Messages: {}", self.websocket_messages);
        println!("  Errors: {}", self.errors.len());
        
        if !self.errors.is_empty() {
            println!("\n❌ Errors encountered:");
            for error in &self.errors {
                println!("  - {}", error);
            }
        }
    }
}

// HTTP Client for REST API tests
async fn create_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

// Test 1: Basic API Health Check
async fn test_api_health(client: &Client) -> Result<(), String> {
    println!("🔍 Testing API Health...");
    
    let response = client
        .get(&format!("{}/api/v1/orderbook/{}/?depth=5", BASE_URL, TEST_SYMBOL))
        .send()
        .await
        .map_err(|e| format!("Failed to get orderbook: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Orderbook API returned status: {}", response.status()));
    }
    
    let orderbook: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse orderbook JSON: {}", e))?;
    
    println!("✅ Orderbook API working: {:?}", orderbook["symbol"]);
    Ok(())
}

// Test 2: Order Submission and Processing
async fn test_order_submission(
    client: &Client,
    results: &mut TestResults,
) -> Result<Vec<String>, String> {
    println!("📝 Testing Order Submission...");
    
    let mut order_ids = Vec::new();
    
    // Test 1: Buy Limit Order
    let buy_order = TestOrder {
        symbol: TEST_SYMBOL.to_string(),
        side: "Buy".to_string(),
        order_type: "Limit".to_string(),
        quantity: 1000000, // 0.001 BTC in satoshis
        price: Some(95000000), // 95M KRW
        client_id: TEST_CLIENT_ID.to_string(),
    };
    
    let response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&buy_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit buy order: {}", e))?;
    
    results.orders_submitted += 1;
    
    if response.status().is_success() {
        results.orders_accepted += 1;
        let response_data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse order response: {}", e))?;
        
        if let Some(order_id) = response_data["order_id"].as_str() {
            order_ids.push(order_id.to_string());
            println!("✅ Buy order submitted: {}", order_id);
        }
    } else {
        let error_text = response.text().await.unwrap_or_default();
        results.add_error(format!("Buy order failed: {}", error_text));
    }
    
    // Test 2: Sell Limit Order
    let sell_order = TestOrder {
        symbol: TEST_SYMBOL.to_string(),
        side: "Sell".to_string(),
        order_type: "Limit".to_string(),
        quantity: 500000, // 0.0005 BTC in satoshis
        price: Some(96000000), // 96M KRW
        client_id: TEST_CLIENT_ID.to_string(),
    };
    
    let response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&sell_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit sell order: {}", e))?;
    
    results.orders_submitted += 1;
    
    if response.status().is_success() {
        results.orders_accepted += 1;
        let response_data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse order response: {}", e))?;
        
        if let Some(order_id) = response_data["order_id"].as_str() {
            order_ids.push(order_id.to_string());
            println!("✅ Sell order submitted: {}", order_id);
        }
    } else {
        let error_text = response.text().await.unwrap_or_default();
        results.add_error(format!("Sell order failed: {}", error_text));
    }
    
    // Test 3: Market Order
    let market_order = TestOrder {
        symbol: TEST_SYMBOL.to_string(),
        side: "Buy".to_string(),
        order_type: "Market".to_string(),
        quantity: 200000, // 0.0002 BTC in satoshis
        price: None, // Market orders don't have price
        client_id: TEST_CLIENT_ID.to_string(),
    };
    
    let response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&market_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit market order: {}", e))?;
    
    results.orders_submitted += 1;
    
    if response.status().is_success() {
        results.orders_accepted += 1;
        let response_data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse order response: {}", e))?;
        
        if let Some(order_id) = response_data["order_id"].as_str() {
            order_ids.push(order_id.to_string());
            println!("✅ Market order submitted: {}", order_id);
        }
    } else {
        let error_text = response.text().await.unwrap_or_default();
        results.add_error(format!("Market order failed: {}", error_text));
    }
    
    Ok(order_ids)
}

// Test 3: Order Status and History
async fn test_order_status(
    client: &Client,
    order_ids: &[String],
    results: &mut TestResults,
) -> Result<(), String> {
    println!("📋 Testing Order Status...");
    
    for order_id in order_ids {
        let response = client
            .get(&format!("{}/api/v1/order/{}", BASE_URL, order_id))
            .send()
            .await
            .map_err(|e| format!("Failed to get order status: {}", e))?;
        
        if response.status().is_success() {
            let order_data: Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse order status: {}", e))?;
            
            println!("✅ Order {} status: {:?}", order_id, order_data["status"]);
        } else {
            results.add_error(format!("Failed to get status for order: {}", order_id));
        }
    }
    
    Ok(())
}

// Test 4: OrderBook Data
async fn test_orderbook_data(
    client: &Client,
    _results: &mut TestResults,
) -> Result<TestOrderBook, String> {
    println!("📊 Testing OrderBook Data...");
    
    let response = client
        .get(&format!("{}/api/v1/orderbook/{}/?depth=10", BASE_URL, TEST_SYMBOL))
        .send()
        .await
        .map_err(|e| format!("Failed to get orderbook: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Orderbook API returned status: {}", response.status()));
    }
    
    let orderbook_data: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse orderbook JSON: {}", e))?;
    
    // Parse bids and asks
    let mut bids = Vec::new();
    let mut asks = Vec::new();
    
    if let Some(bids_array) = orderbook_data["bids"].as_array() {
        for bid in bids_array {
            if let (Some(price), Some(volume)) = (
                bid["price"].as_u64(),
                bid["volume"].as_u64(),
            ) {
                bids.push((price, volume));
            }
        }
    }
    
    if let Some(asks_array) = orderbook_data["asks"].as_array() {
        for ask in asks_array {
            if let (Some(price), Some(volume)) = (
                ask["price"].as_u64(),
                ask["volume"].as_u64(),
            ) {
                asks.push((price, volume));
            }
        }
    }
    
    let orderbook = TestOrderBook {
        symbol: orderbook_data["symbol"].as_str().unwrap_or(TEST_SYMBOL).to_string(),
        bids,
        asks,
        timestamp: orderbook_data["timestamp"].as_u64().unwrap_or(0),
    };
    
    println!("✅ OrderBook retrieved: {} bids, {} asks", orderbook.bids.len(), orderbook.asks.len());
    
    Ok(orderbook)
}

// Test 5: Candle Data
async fn test_candle_data(
    client: &Client,
    _results: &mut TestResults,
) -> Result<Vec<TestCandle>, String> {
    println!("📈 Testing Candle Data...");
    
    let response = client
        .get(&format!("{}/api/v1/candles/{}/?interval=1m&limit=20", BASE_URL, TEST_SYMBOL))
        .send()
        .await
        .map_err(|e| format!("Failed to get candles: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Candles API returned status: {}", response.status()));
    }
    
    let candles_data: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse candles JSON: {}", e))?;
    
    let mut candles = Vec::new();
    
    if let Some(candles_array) = candles_data["candles"].as_array() {
        for candle_data in candles_array {
            let candle = TestCandle {
                open_time: candle_data["open_time"].as_u64().unwrap_or(0),
                close_time: candle_data["close_time"].as_u64().unwrap_or(0),
                open: candle_data["open"].as_u64().unwrap_or(0),
                high: candle_data["high"].as_u64().unwrap_or(0),
                low: candle_data["low"].as_u64().unwrap_or(0),
                close: candle_data["close"].as_u64().unwrap_or(0),
                volume: candle_data["volume"].as_u64().unwrap_or(0),
                trade_count: candle_data["trade_count"].as_u64().unwrap_or(0),
            };
            candles.push(candle);
        }
    }
    
    println!("✅ Candles retrieved: {} candles", candles.len());
    
    Ok(candles)
}

// Test 6: WebSocket Connection and Real-time Updates
async fn test_websocket_updates(
    results: &mut TestResults,
) -> Result<(), String> {
    println!("🔌 Testing WebSocket Connection...");
    
    let (ws_stream, _) = connect_async(WS_URL)
        .await
        .map_err(|e| format!("Failed to connect to WebSocket: {}", e))?;
    
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to orderbook updates
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "orderbook",
        "symbol": TEST_SYMBOL
    });
    
    write.send(Message::Text(subscribe_msg.to_string()))
        .await
        .map_err(|e| format!("Failed to send subscription: {}", e))?;
    
    // Subscribe to executions
    let subscribe_executions = json!({
        "type": "subscribe",
        "channel": "executions",
        "symbol": TEST_SYMBOL
    });
    
    write.send(Message::Text(subscribe_executions.to_string()))
        .await
        .map_err(|e| format!("Failed to send executions subscription: {}", e))?;
    
    println!("✅ WebSocket connected and subscribed");
    
    // Listen for messages for 10 seconds
    let timeout = Duration::from_secs(10);
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed() < timeout {
        if let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    results.websocket_messages += 1;
                    
                    if let Ok(data) = serde_json::from_str::<Value>(&text) {
                        match data["type"].as_str() {
                            Some("orderbook") => {
                                results.orderbook_updates += 1;
                                println!("📊 OrderBook update received");
                            }
                            Some("execution") => {
                                results.executions_received += 1;
                                println!("⚡ Execution update received: {:?}", data["execution_id"]);
                            }
                            Some("candle") => {
                                results.candle_updates += 1;
                                println!("📈 Candle update received");
                            }
                            _ => {
                                println!("📨 WebSocket message: {}", text);
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("🔌 WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    results.add_error(format!("WebSocket error: {}", e));
                    break;
                }
                _ => {}
            }
        }
    }
    
    println!("✅ WebSocket test completed");
    Ok(())
}

// Test 7: User Balance and Portfolio
async fn test_user_balance(
    client: &Client,
    results: &mut TestResults,
) -> Result<(), String> {
    println!("💰 Testing User Balance...");
    
    let response = client
        .get(&format!("{}/api/v1/user/{}/balance", BASE_URL, TEST_CLIENT_ID))
        .send()
        .await
        .map_err(|e| format!("Failed to get user balance: {}", e))?;
    
    if response.status().is_success() {
        let balance_data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse balance JSON: {}", e))?;
        
        println!("✅ User balance retrieved: {:?}", balance_data);
    } else {
        results.add_error("Failed to get user balance".to_string());
    }
    
    Ok(())
}

// Test 8: Market Statistics
async fn test_market_statistics(
    client: &Client,
    results: &mut TestResults,
) -> Result<(), String> {
    println!("📊 Testing Market Statistics...");
    
    let response = client
        .get(&format!("{}/api/v1/market/{}/statistics", BASE_URL, TEST_SYMBOL))
        .send()
        .await
        .map_err(|e| format!("Failed to get market statistics: {}", e))?;
    
    if response.status().is_success() {
        let stats_data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse statistics JSON: {}", e))?;
        
        println!("✅ Market statistics retrieved: {:?}", stats_data);
    } else {
        results.add_error("Failed to get market statistics".to_string());
    }
    
    Ok(())
}

// Main E2E Test Function
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting E2E Trading System Tests");
    println!("=====================================");
    
    let mut results = TestResults::default();
    let client = create_http_client().await;
    
    // Test 1: API Health Check
    match test_api_health(&client).await {
        Ok(_) => println!("✅ API Health Check: PASSED"),
        Err(e) => {
            results.add_error(format!("API Health Check: {}", e));
            println!("❌ API Health Check: FAILED");
        }
    }
    
    // Test 2: Order Submission
    let order_ids = match test_order_submission(&client, &mut results).await {
        Ok(ids) => {
            println!("✅ Order Submission: PASSED");
            ids
        }
        Err(e) => {
            results.add_error(format!("Order Submission: {}", e));
            println!("❌ Order Submission: FAILED");
            Vec::new()
        }
    };
    
    // Wait a bit for orders to be processed
    sleep(Duration::from_secs(2)).await;
    
    // Test 3: Order Status
    if !order_ids.is_empty() {
        match test_order_status(&client, &order_ids, &mut results).await {
            Ok(_) => println!("✅ Order Status: PASSED"),
            Err(e) => {
                results.add_error(format!("Order Status: {}", e));
                println!("❌ Order Status: FAILED");
            }
        }
    }
    
    // Test 4: OrderBook Data
    match test_orderbook_data(&client, &mut results).await {
        Ok(_) => println!("✅ OrderBook Data: PASSED"),
        Err(e) => {
            results.add_error(format!("OrderBook Data: {}", e));
            println!("❌ OrderBook Data: FAILED");
        }
    }
    
    // Test 5: Candle Data
    match test_candle_data(&client, &mut results).await {
        Ok(_) => println!("✅ Candle Data: PASSED"),
        Err(e) => {
            results.add_error(format!("Candle Data: {}", e));
            println!("❌ Candle Data: FAILED");
        }
    }
    
    // Test 6: WebSocket Updates
    match test_websocket_updates(&mut results).await {
        Ok(_) => println!("✅ WebSocket Updates: PASSED"),
        Err(e) => {
            results.add_error(format!("WebSocket Updates: {}", e));
            println!("❌ WebSocket Updates: FAILED");
        }
    }
    
    // Test 7: User Balance
    match test_user_balance(&client, &mut results).await {
        Ok(_) => println!("✅ User Balance: PASSED"),
        Err(e) => {
            results.add_error(format!("User Balance: {}", e));
            println!("❌ User Balance: FAILED");
        }
    }
    
    // Test 8: Market Statistics
    match test_market_statistics(&client, &mut results).await {
        Ok(_) => println!("✅ Market Statistics: PASSED"),
        Err(e) => {
            results.add_error(format!("Market Statistics: {}", e));
            println!("❌ Market Statistics: FAILED");
        }
    }
    
    // Print final results
    results.print_summary();
    
    if results.errors.is_empty() {
        println!("\n🎉 All E2E Tests PASSED!");
        Ok(())
    } else {
        println!("\n💥 Some E2E Tests FAILED!");
        Err("E2E tests failed".into())
    }
}

// Additional utility functions for more detailed testing

// Test order matching engine behavior
async fn test_order_matching(
    client: &Client,
    results: &mut TestResults,
) -> Result<(), String> {
    println!("⚡ Testing Order Matching Engine...");
    
    // Submit matching buy and sell orders
    let buy_order = TestOrder {
        symbol: TEST_SYMBOL.to_string(),
        side: "Buy".to_string(),
        order_type: "Limit".to_string(),
        quantity: 1000000,
        price: Some(95000000),
        client_id: format!("{}_buyer", TEST_CLIENT_ID),
    };
    
    let sell_order = TestOrder {
        symbol: TEST_SYMBOL.to_string(),
        side: "Sell".to_string(),
        order_type: "Limit".to_string(),
        quantity: 1000000,
        price: Some(95000000), // Same price for matching
        client_id: format!("{}_seller", TEST_CLIENT_ID),
    };
    
    // Submit both orders
    let buy_response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&buy_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit buy order: {}", e))?;
    
    let sell_response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&sell_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit sell order: {}", e))?;
    
    if buy_response.status().is_success() && sell_response.status().is_success() {
        println!("✅ Matching orders submitted successfully");
        
        // Wait for matching to occur
        sleep(Duration::from_secs(3)).await;
        
        // Check if executions were created
        let executions_response = client
            .get(&format!("{}/api/v1/executions/{}", BASE_URL, TEST_SYMBOL))
            .send()
            .await
            .map_err(|e| format!("Failed to get executions: {}", e))?;
        
        if executions_response.status().is_success() {
            let executions_data: Value = executions_response
                .json()
                .await
                .map_err(|e| format!("Failed to parse executions: {}", e))?;
            
            println!("✅ Executions retrieved: {:?}", executions_data);
        }
    } else {
        results.add_error("Failed to submit matching orders".to_string());
    }
    
    Ok(())
}

// Test error handling
async fn test_error_handling(
    client: &Client,
    results: &mut TestResults,
) -> Result<(), String> {
    println!("🚫 Testing Error Handling...");
    
    // Test invalid order (negative quantity)
    let invalid_order = json!({
        "symbol": TEST_SYMBOL,
        "side": "Buy",
        "type": "Limit",
        "quantity": -1000000, // Invalid negative quantity
        "price": 95000000,
        "client_id": TEST_CLIENT_ID
    });
    
    let response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&invalid_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit invalid order: {}", e))?;
    
    if !response.status().is_success() {
        println!("✅ Invalid order correctly rejected");
    } else {
        results.add_error("Invalid order was accepted".to_string());
    }
    
    // Test invalid symbol
    let invalid_symbol_order = json!({
        "symbol": "INVALID-SYMBOL",
        "side": "Buy",
        "type": "Limit",
        "quantity": 1000000,
        "price": 95000000,
        "client_id": TEST_CLIENT_ID
    });
    
    let response = client
        .post(&format!("{}/api/v1/order", BASE_URL))
        .json(&invalid_symbol_order)
        .send()
        .await
        .map_err(|e| format!("Failed to submit invalid symbol order: {}", e))?;
    
    if !response.status().is_success() {
        println!("✅ Invalid symbol order correctly rejected");
    } else {
        results.add_error("Invalid symbol order was accepted".to_string());
    }
    
    Ok(())
}