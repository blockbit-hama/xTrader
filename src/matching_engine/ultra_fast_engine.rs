use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use log::{debug, error, info, warn, trace};

use crate::matching_engine::model::{Order, OrderType, Side, ExecutionReport, OrderBookSnapshot};
use crate::matching_engine::order_book::OrderBook;

/// 초고성능 매칭 엔진
/// 
/// 핵심 최적화:
/// 1. 메모리 우선 처리 (DB 접근 없음)
/// 2. 비동기 영속화 큐
/// 3. Lock-Free 잔고 관리
/// 4. 배치 처리
pub struct UltraFastMatchingEngine {
    /// 심볼별 주문장 (메모리 전용)
    order_books: HashMap<String, OrderBook>,
    
    /// 주문 ID → 주문 객체 저장소 (메모리 전용)
    order_store: HashMap<String, Order>,
    
    /// Lock-Free 잔고 관리 (메모리 전용)
    balances: HashMap<String, Arc<AtomicU64>>,
    
    /// 체결 보고서 송신 채널
    exec_tx: tokio::sync::mpsc::Sender<ExecutionReport>,
    
    /// WebSocket 브로드캐스트 채널
    broadcast_tx: Option<tokio::sync::broadcast::Sender<crate::api::models::WebSocketMessage>>,
    
    /// 비동기 영속화 큐
    persistence_queue: Arc<Mutex<VecDeque<PersistenceTask>>>,
    
    /// 성능 메트릭
    metrics: Arc<PerformanceMetrics>,
}

/// 영속화 작업
#[derive(Debug, Clone)]
pub struct PersistenceTask {
    pub execution: ExecutionReport,
    pub timestamp: u64,
    pub user_id: String,
    pub balance_delta: i64,
}

/// 성능 메트릭
#[derive(Debug)]
pub struct PerformanceMetrics {
    pub total_executions: AtomicU64,
    pub total_latency_ns: AtomicU64,
    pub memory_executions: AtomicU64,
    pub db_executions: AtomicU64,
    pub start_time: Instant,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            total_executions: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            memory_executions: AtomicU64::new(0),
            db_executions: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }
    
    pub fn record_execution(&self, latency_ns: u64) {
        self.total_executions.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);
        self.memory_executions.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_tps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_executions.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }
    
    pub fn get_avg_latency_us(&self) -> f64 {
        let total_executions = self.total_executions.load(Ordering::Relaxed);
        if total_executions > 0 {
            self.total_latency_ns.load(Ordering::Relaxed) as f64 / total_executions as f64 / 1000.0
        } else {
            0.0
        }
    }
}

impl UltraFastMatchingEngine {
    /// 새 초고성능 매칭 엔진 생성
    pub fn new(
        symbols: Vec<String>, 
        exec_tx: tokio::sync::mpsc::Sender<ExecutionReport>
    ) -> Self {
        let mut order_books = HashMap::new();
        let mut balances = HashMap::new();
        
        // 지원하는 모든 심볼에 대해 주문장 생성
        for symbol in symbols {
            order_books.insert(symbol.clone(), OrderBook::new(symbol));
        }
        
        // 기본 사용자 잔고 초기화 (테스트용)
        balances.insert("user1".to_string(), Arc::new(AtomicU64::new(50_000_000_000))); // 500억원
        balances.insert("user2".to_string(), Arc::new(AtomicU64::new(50_000_000_000))); // 500억원
        
        Self {
            order_books,
            order_store: HashMap::new(),
            balances,
            exec_tx,
            broadcast_tx: None,
            persistence_queue: Arc::new(Mutex::new(VecDeque::new())),
            metrics: Arc::new(PerformanceMetrics::new()),
        }
    }
    
    /// WebSocket 브로드캐스트 채널 설정
    pub fn set_broadcast_channel(&mut self, broadcast_tx: tokio::sync::broadcast::Sender<crate::api::models::WebSocketMessage>) {
        self.broadcast_tx = Some(broadcast_tx);
    }
    
    /// 초고속 체결 처리 (< 1μs 목표)
    pub async fn ultra_fast_execution(&mut self, order: Order) -> Result<ExecutionReport, ExecutionError> {
        let start_time = Instant::now();
        
        // 1. 메모리에서만 매칭 (DB 접근 없음)
        let execution = self.match_in_memory_only(order).await?;
        
        // 2. 즉시 클라이언트 응답 (DB 대기 없음)
        self.send_immediate_response(&execution).await;
        
        // 3. 비동기로 영속화 큐에 추가
        self.queue_for_persistence(execution.clone()).await;
        
        // 4. 성능 메트릭 기록
        let latency_ns = start_time.elapsed().as_nanos() as u64;
        self.metrics.record_execution(latency_ns);
        
        debug!("초고속 체결 완료: {} (지연: {}ns)", execution.execution_id, latency_ns);
        
        Ok(execution)
    }
    
    /// 메모리 전용 매칭 (DB 접근 없음)
    async fn match_in_memory_only(&mut self, order: Order) -> Result<ExecutionReport, ExecutionError> {
        // 잔고 검증 (메모리 캐시에서만)
        let balance = self.get_balance_memory_only(&order.user_id);
        if balance < order.total_amount() {
            return Err(ExecutionError::InsufficientBalance);
        }
        
        // 주문 매칭 (메모리 주문장에서만)
        let execution = self.match_order_memory_only(order)?;
        
        // 잔고 업데이트 (메모리에서만)
        self.update_balance_memory_only(&execution);
        
        Ok(execution)
    }
    
    /// 메모리에서만 잔고 조회
    fn get_balance_memory_only(&self, user_id: &str) -> u64 {
        self.balances.get(user_id)
            .map(|balance| balance.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
    
    /// 메모리에서만 잔고 업데이트
    fn update_balance_memory_only(&self, execution: &ExecutionReport) {
        // 테이커와 메이커 모두 잔고 업데이트
        let taker_balance = self.balances.get(&execution.order_id);
        let maker_balance = self.balances.get(&execution.counterparty_id);
        
        match execution.side {
            Side::Buy => {
                // 매수: KRW 차감, BTC 증가
                if let Some(balance) = taker_balance {
                    let krw_cost = execution.price * execution.quantity;
                    balance.fetch_sub(krw_cost, Ordering::Relaxed);
                }
                if let Some(balance) = maker_balance {
                    balance.fetch_add(execution.price * execution.quantity, Ordering::Relaxed);
                }
            }
            Side::Sell => {
                // 매도: BTC 차감, KRW 증가
                if let Some(balance) = taker_balance {
                    balance.fetch_sub(execution.quantity, Ordering::Relaxed);
                }
                if let Some(balance) = maker_balance {
                    balance.fetch_add(execution.quantity, Ordering::Relaxed);
                }
            }
        }
    }
    
    /// 메모리에서만 주문 매칭
    fn match_order_memory_only(&mut self, mut order: Order) -> Result<ExecutionReport, ExecutionError> {
        let symbol = order.symbol.clone();
        
        // 지원 심볼 확인
        if !self.order_books.contains_key(&symbol) {
            return Err(ExecutionError::UnsupportedSymbol);
        }
        
        // 주문 저장 (메모리에서만)
        self.order_store.insert(order.id.clone(), order.clone());
        
        // 주문 타입에 따라 처리
        match order.order_type {
            OrderType::Market => {
                self.match_market_order_memory_only(&mut order, symbol.clone())?;
            }
            OrderType::Limit => {
                self.match_limit_order_memory_only(&mut order, symbol.clone())?;
                
                // 완전히 체결되지 않은 경우 주문장에 추가
                if !order.is_filled() {
                    let order_book = self.order_books.get_mut(&symbol).unwrap();
                    order_book.add_order(order);
                } else {
                    // 완전히 체결된 주문은 저장소에서 제거
                    self.order_store.remove(&order.id);
                }
            }
        }
        
        // 체결 보고서 생성
        Ok(self.create_execution_report(&order))
    }
    
    /// 메모리에서만 시장가 주문 매칭
    fn match_market_order_memory_only(&mut self, order: &mut Order, symbol: String) -> Result<(), ExecutionError> {
        let order_book_ptr = self.order_books.get_mut(&symbol).unwrap() as *mut OrderBook;
        
        match order.side {
            Side::Buy => {
                while !order.is_filled() {
                    let order_book = unsafe { &mut *order_book_ptr };
                    
                    if let Some((price, _price_level)) = order_book.get_best_ask() {
                        let match_qty = order.remaining_quantity;
                        self.match_at_price_level_memory_only(order, order_book, price, match_qty)?;
                        
                        if order_book.asks.is_empty() || order_book.get_best_ask().is_none() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            Side::Sell => {
                while !order.is_filled() {
                    let order_book = unsafe { &mut *order_book_ptr };
                    
                    if let Some((price, _price_level)) = order_book.get_best_bid() {
                        let match_qty = order.remaining_quantity;
                        self.match_at_price_level_memory_only(order, order_book, price, match_qty)?;
                        
                        if order_book.bids.is_empty() || order_book.get_best_bid().is_none() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 메모리에서만 지정가 주문 매칭
    fn match_limit_order_memory_only(&mut self, order: &mut Order, symbol: String) -> Result<(), ExecutionError> {
        let order_book_ptr = self.order_books.get_mut(&symbol).unwrap() as *mut OrderBook;
        
        match order.side {
            Side::Buy => {
                let mut continue_matching = true;
                
                while !order.is_filled() && continue_matching {
                    let order_book = unsafe { &mut *order_book_ptr };
                    
                    if let Some((ask_price, _price_level)) = order_book.get_best_ask() {
                        if order.price >= ask_price {
                            let match_qty = order.remaining_quantity;
                            self.match_at_price_level_memory_only(order, order_book, ask_price, match_qty)?;
                            
                            if order_book.asks.is_empty() || order_book.get_best_ask().is_none() {
                                continue_matching = false;
                            }
                        } else {
                            continue_matching = false;
                        }
                    } else {
                        continue_matching = false;
                    }
                }
            }
            Side::Sell => {
                let mut continue_matching = true;
                
                while !order.is_filled() && continue_matching {
                    let order_book = unsafe { &mut *order_book_ptr };
                    
                    if let Some((bid_price, _price_level)) = order_book.get_best_bid() {
                        if order.price <= bid_price {
                            let match_qty = order.remaining_quantity;
                            self.match_at_price_level_memory_only(order, order_book, bid_price, match_qty)?;
                            
                            if order_book.bids.is_empty() || order_book.get_best_bid().is_none() {
                                continue_matching = false;
                            }
                        } else {
                            continue_matching = false;
                        }
                    } else {
                        continue_matching = false;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 메모리에서만 가격 레벨 매칭
    fn match_at_price_level_memory_only(
        &mut self,
        order: &mut Order,
        order_book: &mut OrderBook,
        price: u64,
        match_qty: u64,
    ) -> Result<(), ExecutionError> {
        if match_qty == 0 {
            return Ok(());
        }
        
        let opposite_side = match order.side {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        };
        
        let result = match opposite_side {
            Side::Buy => {
                if let Some(price_level) = order_book.bids.get_mut(&std::cmp::Reverse(price)) {
                    price_level.match_partial(match_qty)
                } else {
                    None
                }
            }
            Side::Sell => {
                if let Some(price_level) = order_book.asks.get_mut(&price) {
                    price_level.match_partial(match_qty)
                } else {
                    None
                }
            }
        };
        
        if let Some((cloned_maker_id, cloned_maker, actual_match_qty)) = result {
            // 빈 가격 레벨 제거
            match opposite_side {
                Side::Buy => {
                    if let Some(price_level) = order_book.bids.get(&std::cmp::Reverse(price)) {
                        if price_level.is_empty() {
                            order_book.bids.remove(&std::cmp::Reverse(price));
                        }
                    }
                }
                Side::Sell => {
                    if let Some(price_level) = order_book.asks.get(&price) {
                        if price_level.is_empty() {
                            order_book.asks.remove(&price);
                        }
                    }
                }
            }
            
            // 테이커 주문 수량 업데이트
            order.fill(actual_match_qty);
            
            // 메이커 주문 저장소 업데이트
            if cloned_maker.is_filled() {
                order_book.orders.remove(&cloned_maker_id);
                self.order_store.remove(&cloned_maker_id);
            } else {
                self.order_store.insert(cloned_maker_id.clone(), cloned_maker.clone());
            }
            
            debug!("메모리 매칭 완료: {} <-> {}, 가격: {}, 수량: {}", 
                   order.id, cloned_maker.id, price, actual_match_qty);
        }
        
        Ok(())
    }
    
    /// 체결 보고서 생성
    fn create_execution_report(&self, order: &Order) -> ExecutionReport {
        ExecutionReport {
            execution_id: Uuid::new_v4().to_string(),
            order_id: order.id.clone(),
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            price: order.price,
            quantity: order.quantity - order.remaining_quantity,
            remaining_quantity: order.remaining_quantity,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            counterparty_id: "system".to_string(), // 실제로는 매칭된 주문 ID
            is_maker: false,
        }
    }
    
    /// 즉시 클라이언트 응답 전송
    async fn send_immediate_response(&self, execution: &ExecutionReport) {
        if let Some(ref broadcast_tx) = self.broadcast_tx {
            let message = crate::api::models::WebSocketMessage::ExecutionUpdate {
                execution_id: execution.execution_id.clone(),
                order_id: execution.order_id.clone(),
                symbol: execution.symbol.clone(),
                side: execution.side.clone(),
                price: execution.price,
                quantity: execution.quantity,
                remaining_quantity: execution.remaining_quantity,
                timestamp: execution.timestamp,
            };
            
            if let Err(e) = broadcast_tx.send(message) {
                warn!("즉시 응답 전송 실패: {}", e);
            }
        }
    }
    
    /// 영속화 큐에 추가
    async fn queue_for_persistence(&self, execution: ExecutionReport) {
        let task = PersistenceTask {
            execution: execution.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            user_id: execution.order_id.clone(), // 실제로는 사용자 ID
            balance_delta: execution.price as i64 * execution.quantity as i64,
        };
        
        let mut queue = self.persistence_queue.lock().await;
        queue.push_back(task);
    }
    
    /// 영속화 큐 가져오기 (배치 처리용)
    pub async fn drain_persistence_queue(&self, batch_size: usize) -> Vec<PersistenceTask> {
        let mut queue = self.persistence_queue.lock().await;
        queue.drain(..batch_size.min(queue.len())).collect()
    }
    
    /// 성능 메트릭 조회
    pub fn get_metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }
    
    /// 주문장 조회
    pub fn get_order_books(&self) -> &HashMap<String, OrderBook> {
        &self.order_books
    }
    
    /// 주문 조회
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        self.order_store.get(order_id)
    }
    
    /// 주문장 스냅샷 조회
    pub fn get_order_book_snapshot(&self, symbol: &str, depth: usize) -> Option<OrderBookSnapshot> {
        self.order_books.get(symbol).map(|ob| ob.get_order_book_snapshot(depth))
    }
    
    /// 잔고 조회 (메모리에서만)
    pub fn get_balance(&self, user_id: &str) -> u64 {
        self.get_balance_memory_only(user_id)
    }
    
    /// 성능 통계 출력
    pub fn print_performance_stats(&self) {
        let metrics = &self.metrics;
        let tps = metrics.get_tps();
        let avg_latency = metrics.get_avg_latency_us();
        
        info!("🚀 초고성능 매칭 엔진 통계:");
        info!("   총 체결 수: {}", metrics.total_executions.load(Ordering::Relaxed));
        info!("   처리량: {:.2} TPS", tps);
        info!("   평균 지연: {:.2} μs", avg_latency);
        info!("   메모리 체결: {}", metrics.memory_executions.load(Ordering::Relaxed));
        info!("   DB 체결: {}", metrics.db_executions.load(Ordering::Relaxed));
    }
}

/// 체결 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("잔고 부족")]
    InsufficientBalance,
    #[error("지원하지 않는 심볼")]
    UnsupportedSymbol,
    #[error("매칭 실패")]
    MatchingFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching_engine::model::{Order, Side, OrderType};
    use tokio::sync::mpsc;
    
    fn create_test_order(id: &str, side: Side, order_type: OrderType, price: u64, quantity: u64) -> Order {
        Order {
            id: id.to_string(),
            symbol: "BTC-KRW".to_string(),
            side,
            order_type,
            price,
            quantity,
            remaining_quantity: quantity,
            client_id: "user1".to_string(),
            user_id: "user1".to_string(),
            timestamp: 0,
            is_cancel: false,
            target_order_id: None,
        }
    }
    
    #[tokio::test]
    async fn test_ultra_fast_execution() {
        let (exec_tx, mut exec_rx) = mpsc::channel(1000);
        let symbols = vec!["BTC-KRW".to_string()];
        let mut engine = UltraFastMatchingEngine::new(symbols, exec_tx);
        
        // 매도 주문 추가
        let sell_order = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 100);
        engine.process_order(sell_order).await.unwrap();
        
        // 매수 주문 추가 (매칭)
        let buy_order = create_test_order("order2", Side::Buy, OrderType::Limit, 10000, 100);
        let execution = engine.process_order(buy_order).await.unwrap();
        
        assert_eq!(execution.price, 10000);
        assert_eq!(execution.quantity, 100);
        
        // 성능 메트릭 확인
        let metrics = engine.get_metrics();
        assert!(metrics.get_tps() > 0.0);
        assert!(metrics.get_avg_latency_us() < 1000.0); // 1ms 미만
    }
    
    #[tokio::test]
    async fn test_memory_only_balance_check() {
        let (exec_tx, _) = mpsc::channel(1000);
        let symbols = vec!["BTC-KRW".to_string()];
        let engine = UltraFastMatchingEngine::new(symbols, exec_tx);
        
        // 잔고 조회 (메모리에서만)
        let balance = engine.get_balance("user1");
        assert_eq!(balance, 50_000_000_000);
        
        // 존재하지 않는 사용자
        let balance_unknown = engine.get_balance("unknown_user");
        assert_eq!(balance_unknown, 0);
    }
}