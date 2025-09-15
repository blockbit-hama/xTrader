use std::collections::HashMap;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{debug, error, info, warn, trace};
use std::cmp::Reverse;
use std::sync::mpsc::{Receiver, Sender};
use crate::matching_engine::model::{
  Order, OrderType, Side, ExecutionReport, OrderBookSnapshot
};
use crate::matching_engine::order_book::OrderBook;
use crate::api::models::WebSocketMessage;

/// 매칭 엔진 구현
pub struct MatchingEngine {
  /// 심볼별 주문장
  order_books: HashMap<String, OrderBook>,
  /// 주문 ID → 주문 객체 저장소
  order_store: HashMap<String, Order>,
  /// 체결 보고서 송신 채널
  exec_tx: Sender<ExecutionReport>,
  /// WebSocket 브로드캐스트 채널
  broadcast_tx: Option<tokio::sync::broadcast::Sender<WebSocketMessage>>,
}

impl MatchingEngine {
  /// 새 매칭 엔진 생성
  pub fn new(symbols: Vec<String>, exec_tx: Sender<ExecutionReport>) -> Self {
    let mut order_books = HashMap::new();
    
    // 지원하는 모든 심볼에 대해 주문장 생성
    for symbol in symbols {
      order_books.insert(symbol.clone(), OrderBook::new(symbol));
    }
    
    MatchingEngine {
      order_books,
      order_store: HashMap::new(),
      exec_tx,
      broadcast_tx: None,
    }
  }

  /// WebSocket 브로드캐스트 채널 설정
  pub fn set_broadcast_channel(&mut self, broadcast_tx: tokio::sync::broadcast::Sender<WebSocketMessage>) {
    self.broadcast_tx = Some(broadcast_tx);
  }
  
  /// 매칭 엔진 실행 (주문 처리 루프)
  pub fn run(&mut self, mut order_rx: Receiver<Order>) {
    info!("매칭 엔진 시작");
    
    // 주문 수신 및 처리
    while let Ok(order) = order_rx.recv() {
      if order.is_cancel {
        debug!("취소 주문 수신: {}", order.id);
        self.handle_cancel_order(&order);
      } else {
        debug!("새 주문 수신: {} ({}, {}, 가격: {}, 수량: {})", 
                      order.id, 
                      order.symbol,
                      if let Side::Buy = order.side { "매수" } else { "매도" },
                      order.price,
                      order.quantity);
        self.process_order(order);
      }
    }
    
    info!("매칭 엔진 종료");
  }

  /// 시퀀서를 통한 매칭 엔진 실행 (순서 보장)
  pub fn run_sequenced(&mut self, mut order_rx: Receiver<Order>) {
    info!("매칭 엔진 시작 (시퀀서 모드)");
    
    // 주문 수신 및 처리 (FIFO 순서 보장)
    while let Ok(order) = order_rx.recv() {
      if order.is_cancel {
        debug!("[시퀀서] 취소 주문 수신: {}", order.id);
        self.handle_cancel_order(&order);
      } else {
        debug!("[시퀀서] 새 주문 수신: {} ({}, {}, 가격: {}, 수량: {})", 
                      order.id, 
                      order.symbol,
                      if let Side::Buy = order.side { "매수" } else { "매도" },
                      order.price,
                      order.quantity);
        self.process_order(order);
      }
    }
    
    info!("매칭 엔진 종료 (시퀀서 모드)");
  }
  
  /// 취소 주문 처리
  pub fn handle_cancel_order(&mut self, cancel_order: &Order) {
    if let Some(target_order_id) = &cancel_order.target_order_id {
      // 취소할 원본 주문 찾기
      if let Some(order) = self.order_store.get(target_order_id) {
        let symbol = order.symbol.clone();
        
        // 주문장에서 주문 취소
        if let Some(order_book) = self.order_books.get_mut(&symbol) {
          if let Some(cancelled_order) = order_book.cancel_order(target_order_id) {
            // 취소 확인 체결 보고서 생성
            let cancel_report = ExecutionReport {
              execution_id: Uuid::new_v4().to_string(),
              order_id: target_order_id.clone(),
              symbol: cancelled_order.symbol.clone(),
              side: cancelled_order.side.clone(),
              price: cancelled_order.price,
              quantity: 0,
              remaining_quantity: 0,
              timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
              counterparty_id: "system".to_string(),
              is_maker: false,
            };
            
            // 체결 보고서 전송
            if let Err(e) = self.exec_tx.send(cancel_report) {
              error!("취소 체결 보고서 전송 실패: {}", e);
            } else {
              debug!("주문 취소 완료: {}", target_order_id);
            }
            
            // 저장소에서 주문 제거
            self.order_store.remove(target_order_id);
          } else {
            warn!("취소할 주문을 찾을 수 없음: {}", target_order_id);
          }
        } else {
          warn!("주문장을 찾을 수 없음: {}", symbol);
        }
      } else {
        warn!("취소할 주문을 저장소에서 찾을 수 없음: {}", target_order_id);
      }
      
      // 취소 처리 후 호가창 업데이트 브로드캐스트
      self.broadcast_orderbook_update(&cancel_order.symbol);
    } else {
      error!("취소 주문에 대상 주문 ID가 없음: {}", cancel_order.id);
    }
  }
  
  /// 주문 처리
  fn process_order(&mut self, mut order: Order) {
    let symbol = order.symbol.clone();
    
    // 지원 심볼 확인
    if !self.order_books.contains_key(&symbol) {
      error!("지원하지 않는 심볼: {}", symbol);
      return;
    }
    
    // 주문 저장
    self.order_store.insert(order.id.clone(), order.clone());
    
    // 주문 타입에 따라 처리
    match order.order_type {
      OrderType::Market => {
        debug!("시장가 주문 처리: {}", order.id);
        self.match_market_order(&mut order, symbol.clone());
      },
      OrderType::Limit => {
        debug!("지정가 주문 처리: {} (가격: {})", order.id, order.price);
        self.match_limit_order(&mut order, symbol.clone());
        
        // 완전히 체결되지 않은 경우 주문장에 추가
        if !order.is_filled() {
          debug!("미체결 지정가 주문 주문장 추가: {}, 남은 수량: {}", 
                          order.id, order.remaining_quantity);
          let order_book = self.order_books.get_mut(&symbol).unwrap();
          order_book.add_order(order);
        } else {
          // 완전히 체결된 주문은 저장소에서 제거
          debug!("완전 체결된 주문 제거: {}", order.id);
          self.order_store.remove(&order.id);
        }
      }
    }
    
    // 주문 처리 후 호가창 업데이트 브로드캐스트
    self.broadcast_orderbook_update(&symbol);
  }
  
  /// 시장가 주문 매칭
  fn match_market_order(&mut self, order: &mut Order, symbol: String) {
    // order_book을 self에서 미리 꺼냄
    let order_book_ptr = self.order_books.get_mut(&symbol).unwrap() as *mut OrderBook;
    
    match order.side {
      Side::Buy => {
        // 매수 주문(테이커)은 매도 주문(메이커)과 매칭
        while !order.is_filled() {
          let order_book = unsafe { &mut *order_book_ptr };
          
          // 최저 매도가 확인
          if let Some((price, _price_level)) = order_book.get_best_ask() {
            // 최대 매칭 가능 수량 계산
            let match_qty = order.remaining_quantity;
            
            // 매칭 시도
            self.match_at_price_level(order, order_book, price, match_qty);
            
            // 매칭 후에도 매도 호가가 남아있는지 확인
            if order_book.asks.is_empty() || order_book.get_best_ask().is_none() {
              debug!("더 이상 매칭할 매도 주문 없음, 시장가 매수 주문 미체결: {}", order.id);
              break;
            }
          } else {
            debug!("매칭할 매도 주문 없음, 시장가 매수 주문 미체결: {}", order.id);
            break;
          }
        }
      }
      Side::Sell => {
        // 매도 주문(테이커)은 매수 주문(메이커)과 매칭
        while !order.is_filled() {
          let order_book = unsafe { &mut *order_book_ptr };
          
          // 최고 매수가 확인
          if let Some((price, _price_level)) = order_book.get_best_bid() {
            // 최대 매칭 가능 수량 계산
            let match_qty = order.remaining_quantity;
            
            // 매칭 시도
            self.match_at_price_level(order, order_book, price, match_qty);
            
            // 매칭 후에도 매수 호가가 남아있는지 확인
            if order_book.bids.is_empty() || order_book.get_best_bid().is_none() {
              debug!("더 이상 매칭할 매수 주문 없음, 시장가 매도 주문 미체결: {}", order.id);
              break;
            }
          } else {
            debug!("매칭할 매수 주문 없음, 시장가 매도 주문 미체결: {}", order.id);
            break;
          }
        }
      }
    }
    
    debug!("시장가 주문 처리 완료: {}, 체결량: {}/{}", 
        order.id, order.quantity - order.remaining_quantity, order.quantity);
    
    // 처리 완료한 주문은 저장소에서 제거
    self.order_store.remove(&order.id);
  }
  
  /// 지정가 주문 매칭
  fn match_limit_order(&mut self, order: &mut Order, symbol: String) {
    // order_book을 self에서 미리 꺼냄
    let order_book_ptr = self.order_books.get_mut(&symbol).unwrap() as *mut OrderBook;
    
    match order.side {
      Side::Buy => {
        // 매수 주문(테이커)은 매도 주문(메이커)과 매칭
        let mut continue_matching = true;
        
        while !order.is_filled() && continue_matching {
          let order_book = unsafe { &mut *order_book_ptr };
          
          if let Some((ask_price, _price_level)) = order_book.get_best_ask() {
            // 가격 확인: 매수가 >= 매도가 (체결 가능)
            if order.price >= ask_price {
              // 최대 매칭 가능 수량 계산
              let match_qty = order.remaining_quantity;
              
              // 매칭 시도
              self.match_at_price_level(order, order_book, ask_price, match_qty);
              
              // 매칭 후에도 매도 호가가 남아있는지 확인
              if order_book.asks.is_empty() || order_book.get_best_ask().is_none() {
                debug!("더 이상 매칭할 매도 주문 없음");
                continue_matching = false;
              }
            } else {
              // 가격이 맞지 않으면 매칭 종료
              debug!("매칭 불가: 매수가({}) < 매도가({})", order.price, ask_price);
              continue_matching = false;
            }
          } else {
            // 매도 호가가 없으면 매칭 종료
            debug!("매칭할 매도 주문 없음");
            continue_matching = false;
          }
        }
      }
      Side::Sell => {
        // 매도 주문(테이커)은 매수 주문(메이커)과 매칭
        let mut continue_matching = true;
        
        while !order.is_filled() && continue_matching {
          let order_book = unsafe { &mut *order_book_ptr };
          
          if let Some((bid_price, _price_level)) = order_book.get_best_bid() {
            // 가격 확인: 매도가 <= 매수가 (체결 가능)
            if order.price <= bid_price {
              // 최대 매칭 가능 수량 계산
              let match_qty = order.remaining_quantity;
              
              // 매칭 시도
              self.match_at_price_level(order, order_book, bid_price, match_qty);
              
              // 매칭 후에도 매수 호가가 남아있는지 확인
              if order_book.bids.is_empty() || order_book.get_best_bid().is_none() {
                debug!("더 이상 매칭할 매수 주문 없음");
                continue_matching = false;
              }
            } else {
              // 가격이 맞지 않으면 매칭 종료
              debug!("매칭 불가: 매도가({}) > 매수가({})", order.price, bid_price);
              continue_matching = false;
            }
          } else {
            // 매수 호가가 없으면 매칭 종료
            debug!("매칭할 매수 주문 없음");
            continue_matching = false;
          }
        }
      }
    }
  }
  
  /// 특정 가격 레벨에서 주문 매칭
  ///
  /// 지정된 가격과 수량으로 주문을 매칭합니다.
  /// 이 메서드는 실제 체결 처리와 체결 보고서 생성을 담당합니다.
  ///
  /// # 매개변수
  /// - `order`: 매칭할 주문 (테이커)
  /// - `order_book`: 주문장
  /// - `price`: 체결 가격
  /// - `match_qty`: 체결 수량
  fn match_at_price_level(
    &mut self,
    order: &mut Order,
    order_book: &mut OrderBook,
    price: u64,
    match_qty: u64
  ) {
    // 매칭할 수량이 없으면 종료
    if match_qty == 0 {
      return;
    }
    
    // 반대편 가격 레벨 가져오기
    let opposite_side = match order.side {
      Side::Buy => Side::Sell,
      Side::Sell => Side::Buy,
    };
    
    // 반대 가격 레벨에서 주문 매칭
    let result = match opposite_side {
      Side::Buy => {
        // 매수 가격 레벨에서 매칭
        if let Some(price_level) = order_book.bids.get_mut(&Reverse(price)) {
          price_level.match_partial(match_qty)
        } else {
          None
        }
      },
      Side::Sell => {
        // 매도 가격 레벨에서 매칭
        if let Some(price_level) = order_book.asks.get_mut(&price) {
          price_level.match_partial(match_qty)
        } else {
          None
        }
      }
    };
    
    // 매칭 결과 처리
    if let Some((cloned_maker_id, cloned_maker, actual_match_qty)) = result {
      // 빈 가격 레벨 제거 확인
      match opposite_side {
        Side::Buy => {
          if let Some(price_level) = order_book.bids.get(&Reverse(price)) {
            if price_level.is_empty() {
              order_book.bids.remove(&Reverse(price));
              debug!("빈 가격 레벨 제거 (매수): {}", price);
            }
          }
        },
        Side::Sell => {
          if let Some(price_level) = order_book.asks.get(&price) {
            if price_level.is_empty() {
              order_book.asks.remove(&price);
              debug!("빈 가격 레벨 제거 (매도): {}", price);
            }
          }
        }
      }
      
      // 테이커 주문 수량 업데이트
      order.fill(actual_match_qty);
      
      // 메이커 주문 저장소 업데이트 (완전 체결된 경우 제거)
      if cloned_maker.is_filled() {
        order_book.orders.remove(&cloned_maker_id);
        self.order_store.remove(&cloned_maker_id);
      } else {
        // 부분 체결된 경우 저장소 업데이트
        self.order_store.insert(cloned_maker_id.clone(), cloned_maker.clone());
      }
      
      // 현재 시간 가져오기
      let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
      
      // taker 체결 보고서 생성 
      let exec_id = Uuid::new_v4().to_string();
      let taker_exec = ExecutionReport {
        execution_id: exec_id.clone(),
        order_id: order.id.clone(),
        symbol: order.symbol.clone(),
        side: order.side.clone(),
        price,
        quantity: actual_match_qty,
        remaining_quantity: order.remaining_quantity,
        timestamp: now,
        counterparty_id: cloned_maker.id.clone(),
        is_maker: false,
      };
      
      if let Err(e) = self.exec_tx.send(taker_exec) {
        error!("테이커 체결 보고서 전송 실패: {}", e);
      }
      
      // maker 체결 보고서 생성
      let maker_exec = ExecutionReport {
        execution_id: exec_id,
        order_id: cloned_maker.id.clone(),
        symbol: cloned_maker.symbol.clone(),
        side: cloned_maker.side.clone(),
        price,
        quantity: actual_match_qty,
        remaining_quantity: cloned_maker.remaining_quantity,
        timestamp: now,
        counterparty_id: order.id.clone(),
        is_maker: true,
      };
      
      if let Err(e) = self.exec_tx.send(maker_exec) {
        error!("메이커 체결 보고서 전송 실패: {}", e);
      }
      
      debug!("주문 체결: {} <-> {}, 가격: {}, 수량: {}", 
                  order.id, cloned_maker.id, price, actual_match_qty);
    }
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
  
  /// 심볼별 주문 통계 출력
  pub fn print_stats(&self) {
    for (symbol, order_book) in &self.order_books {
      info!("심볼: {}, 매수 주문 수: {}, 매도 주문 수: {}, 총 주문 수: {}", 
                 symbol, 
                 order_book.bid_count(), 
                 order_book.ask_count(),
                 order_book.order_count());
    }
  }

  /// 호가창 업데이트 브로드캐스트
  fn broadcast_orderbook_update(&self, symbol: &str) {
    if let Some(ref broadcast_tx) = self.broadcast_tx {
      if let Some(snapshot) = self.get_order_book_snapshot(symbol, 10) {
        let message = WebSocketMessage::OrderBookUpdate {
          symbol: symbol.to_string(),
          bids: snapshot.bids,
          asks: snapshot.asks,
          timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        };
        
        if let Err(e) = broadcast_tx.send(message) {
          warn!("호가창 업데이트 브로드캐스트 실패: {}", e);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::matching_engine::model::{Order, Side, OrderType};
  use std::sync::mpsc;
  
  // 테스트용 주문 생성 헬퍼 함수
  fn create_test_order(id: &str, side: Side, order_type: OrderType, price: u64, quantity: u64) -> Order {
    Order {
      id: id.to_string(),
      symbol: "BTC-KRW".to_string(),
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
  fn create_cancel_order(id: &str, target_id: &str) -> Order {
    Order {
      id: id.to_string(),
      symbol: "BTC-KRW".to_string(),
      side: Side::Buy, // 취소 주문은 side가 중요하지 않음
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
  
  #[test]
  fn test_matching_engine_initialization() {
    // 채널 생성
    let (exec_tx, _exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string(), "ETH-KRW".to_string()];
    let engine = MatchingEngine::new(symbols, exec_tx);
    
    // 검증
    assert_eq!(engine.order_books.len(), 2);
    assert!(engine.order_books.contains_key("BTC-KRW"));
    assert!(engine.order_books.contains_key("ETH-KRW"));
    assert!(engine.order_store.is_empty());
  }
  
  #[test]
  fn test_limit_order_no_match() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 매수 주문 추가 (매칭되지 않음)
    let buy_order = create_test_order("order1", Side::Buy, OrderType::Limit, 10000, 100);
    engine.process_order(buy_order);
    
    // 매도 주문 추가 (매칭되지 않음 - 가격이 맞지 않음)
    let sell_order = create_test_order("order2", Side::Sell, OrderType::Limit, 11000, 100);
    engine.process_order(sell_order);
    
    // 체결 보고서가 생성되지 않았는지 확인
    assert!(exec_rx.try_recv().is_err());
  }
  
  #[test]
  fn test_limit_order_full_match() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 매도 주문 추가 (메이커)
    let sell_order = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 100);
    engine.process_order(sell_order);
    
    // 매수 주문 추가 (테이커) - 동일한 가격으로 완전 매칭
    let buy_order = create_test_order("order2", Side::Buy, OrderType::Limit, 10000, 100);
    engine.process_order(buy_order);
    
    // 체결 보고서 수신
    let taker_report = exec_rx.try_recv().unwrap();
    let maker_report = exec_rx.try_recv().unwrap();
    
    // 테이커 체결 보고서 검증
    assert_eq!(taker_report.order_id, "order2");
    assert_eq!(taker_report.side, Side::Buy);
    assert_eq!(taker_report.price, 10000);
    assert_eq!(taker_report.quantity, 100);
    assert_eq!(taker_report.remaining_quantity, 0);
    assert_eq!(taker_report.is_maker, false);
    assert_eq!(taker_report.counterparty_id, "order1");
    
    // 메이커 체결 보고서 검증
    assert_eq!(maker_report.order_id, "order1");
    assert_eq!(maker_report.side, Side::Sell);
    assert_eq!(maker_report.price, 10000);
    assert_eq!(maker_report.quantity, 100);
    assert_eq!(maker_report.remaining_quantity, 0);
    assert_eq!(maker_report.is_maker, true);
    assert_eq!(maker_report.counterparty_id, "order2");
    
    // 더 이상 체결 보고서가 없는지 확인
    assert!(exec_rx.try_recv().is_err());
  }
  
  #[test]
  fn test_limit_order_partial_match() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 매도 주문 추가 (메이커)
    let sell_order = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 100);
    engine.process_order(sell_order);
    
    // 매수 주문 추가 (테이커) - 부분 매칭
    let buy_order = create_test_order("order2", Side::Buy, OrderType::Limit, 10000, 60);
    engine.process_order(buy_order);
    
    // 체결 보고서 수신
    let taker_report = exec_rx.try_recv().unwrap();
    let maker_report = exec_rx.try_recv().unwrap();
    
    // 테이커 체결 보고서 검증
    assert_eq!(taker_report.order_id, "order2");
    assert_eq!(taker_report.quantity, 60);
    assert_eq!(taker_report.remaining_quantity, 0);
    
    // 메이커 체결 보고서 검증
    assert_eq!(maker_report.order_id, "order1");
    assert_eq!(maker_report.quantity, 60);
    assert_eq!(maker_report.remaining_quantity, 40);  // 40주 남음
    
    // 더 이상 체결 보고서가 없는지 확인
    assert!(exec_rx.try_recv().is_err());
  }
  
  #[test]
  fn test_market_order_matching() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 여러 가격의 매도 주문 추가 (메이커)
    let sell_order1 = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 50);
    let sell_order2 = create_test_order("order2", Side::Sell, OrderType::Limit, 10100, 30);
    let sell_order3 = create_test_order("order3", Side::Sell, OrderType::Limit, 10200, 20);
    
    engine.process_order(sell_order1);
    engine.process_order(sell_order2);
    engine.process_order(sell_order3);
    
    // 시장가 매수 주문 (테이커)
    let market_buy = create_test_order("market1", Side::Buy, OrderType::Market, 0, 80);
    engine.process_order(market_buy);
    
    // 체결 보고서 수신 (최저가부터 매칭됨)
    let taker_report1 = exec_rx.try_recv().unwrap();
    let maker_report1 = exec_rx.try_recv().unwrap();
    let taker_report2 = exec_rx.try_recv().unwrap();
    let maker_report2 = exec_rx.try_recv().unwrap();
    
    // 첫번째 매칭 검증 (50주)
    assert_eq!(taker_report1.order_id, "market1");
    assert_eq!(taker_report1.price, 10000);
    assert_eq!(taker_report1.quantity, 50);
    assert_eq!(taker_report1.remaining_quantity, 30);
    
    assert_eq!(maker_report1.order_id, "order1");
    assert_eq!(maker_report1.quantity, 50);
    assert_eq!(maker_report1.remaining_quantity, 0);
    
    // 두번째 매칭 검증 (30주)
    assert_eq!(taker_report2.order_id, "market1");
    assert_eq!(taker_report2.price, 10100);
    assert_eq!(taker_report2.quantity, 30);
    assert_eq!(taker_report2.remaining_quantity, 0);
    
    assert_eq!(maker_report2.order_id, "order2");
    assert_eq!(maker_report2.quantity, 30);
    assert_eq!(maker_report2.remaining_quantity, 0);
    
    // 세번째 매칭은 없음 (시장가 주문이 이미 80주로 완전 체결됨)
    assert!(exec_rx.try_recv().is_err());
  }
  
  #[test]
  fn test_cancel_order() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 매도 주문 추가
    let sell_order = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 100);
    engine.process_order(sell_order);
    
    // 주문 취소
    let cancel_order = create_cancel_order("cancel1", "order1");
    engine.handle_cancel_order(&cancel_order);
    
    // 취소 체결 보고서 수신
    let cancel_report = exec_rx.try_recv().unwrap();
    
    // 취소 보고서 검증
    assert_eq!(cancel_report.order_id, "order1");
    assert_eq!(cancel_report.price, 10000);
    assert_eq!(cancel_report.quantity, 0);
    assert_eq!(cancel_report.remaining_quantity, 0);
    assert_eq!(cancel_report.counterparty_id, "system");
    
    // 더 이상 체결 보고서가 없는지 확인
    assert!(exec_rx.try_recv().is_err());
  }
  
  #[test]
  fn test_multiple_orders_at_same_price_level() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 동일 가격 매도 주문 여러개 추가 (메이커)
    let sell_order1 = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 40);
    let sell_order2 = create_test_order("order2", Side::Sell, OrderType::Limit, 10000, 60);
    
    engine.process_order(sell_order1);
    engine.process_order(sell_order2);
    
    // 매수 주문 추가 (테이커) - 시간 우선순위에 따라 첫번째 주문부터 체결
    let buy_order = create_test_order("buy1", Side::Buy, OrderType::Limit, 10000, 50);
    engine.process_order(buy_order);
    
    // 체결 보고서 수신
    let taker_report1 = exec_rx.try_recv().unwrap();
    let maker_report1 = exec_rx.try_recv().unwrap();
    let taker_report2 = exec_rx.try_recv().unwrap();
    let maker_report2 = exec_rx.try_recv().unwrap();
    
    // 첫번째 매칭 검증 (order1 완전 체결)
    assert_eq!(taker_report1.order_id, "buy1");
    assert_eq!(taker_report1.quantity, 40);
    assert_eq!(taker_report1.remaining_quantity, 10);
    
    // 시간 우선 원칙에 따라 order1이 먼저 체결됨
    assert_eq!(maker_report1.order_id, "order1");
    assert_eq!(maker_report1.quantity, 40);
    assert_eq!(maker_report1.remaining_quantity, 0);
    
    // 두번째 매칭 검증 (order2 부분 체결)
    assert_eq!(taker_report2.order_id, "buy1");
    assert_eq!(taker_report2.quantity, 10);
    assert_eq!(taker_report2.remaining_quantity, 0);
    
    assert_eq!(maker_report2.order_id, "order2");
    assert_eq!(maker_report2.quantity, 10); // 남은 10주 체결
    assert_eq!(maker_report2.remaining_quantity, 50); // 60 - 10 = 50
  }
  
  #[test]
  fn test_order_book_snapshot() {
    // 채널 생성
    let (exec_tx, _) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 주문장에 직접 주문 추가
    let order_book = engine.order_books.get_mut("BTC-KRW").unwrap();
    
    // 매수 주문 추가
    let buy_order1 = create_test_order("buy1", Side::Buy, OrderType::Limit, 9000, 100);
    let buy_order2 = create_test_order("buy2", Side::Buy, OrderType::Limit, 9100, 200);
    
    order_book.add_order(buy_order1);
    order_book.add_order(buy_order2);
    
    // 매도 주문 추가
    let sell_order1 = create_test_order("sell1", Side::Sell, OrderType::Limit, 9900, 300);
    let sell_order2 = create_test_order("sell2", Side::Sell, OrderType::Limit, 10000, 400);
    
    order_book.add_order(sell_order1);
    order_book.add_order(sell_order2);
    
    // 스냅샷 가져오기
    let snapshot = engine.get_order_book_snapshot("BTC-KRW", 5).unwrap();
    
    // 검증
    assert_eq!(snapshot.symbol, "BTC-KRW");
    assert_eq!(snapshot.bids.len(), 2);
    assert_eq!(snapshot.asks.len(), 2);
    
    // 매수 호가 검증 (내림차순)
    assert_eq!(snapshot.bids[0].0, 9100);
    assert_eq!(snapshot.bids[0].1, 200);
    assert_eq!(snapshot.bids[1].0, 9000);
    assert_eq!(snapshot.bids[1].1, 100);
    
    // 매도 호가 검증 (오름차순)
    assert_eq!(snapshot.asks[0].0, 9900);
    assert_eq!(snapshot.asks[0].1, 300);
    assert_eq!(snapshot.asks[1].0, 10000);
    assert_eq!(snapshot.asks[1].1, 400);
  }
  
  #[test]
  fn test_sequential_matching() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 매도 주문 3개 추가 (10주, 50주, 40주)
    let sell_order1 = create_test_order("sell1", Side::Sell, OrderType::Limit, 10000, 10);
    let sell_order2 = create_test_order("sell2", Side::Sell, OrderType::Limit, 10000, 50);
    let sell_order3 = create_test_order("sell3", Side::Sell, OrderType::Limit, 10000, 40);
    
    engine.process_order(sell_order1);
    engine.process_order(sell_order2);
    engine.process_order(sell_order3);
    
    // 매수 주문 100주 (모든 매도 주문 체결)
    let buy_order = create_test_order("buy1", Side::Buy, OrderType::Limit, 10000, 100);
    engine.process_order(buy_order);
    
    // 첫번째 매칭 (10주)
    let taker_report1 = exec_rx.try_recv().unwrap();
    let maker_report1 = exec_rx.try_recv().unwrap();
    
    assert_eq!(taker_report1.order_id, "buy1");
    assert_eq!(taker_report1.quantity, 10);
    assert_eq!(taker_report1.remaining_quantity, 90);
    
    assert_eq!(maker_report1.order_id, "sell1");
    assert_eq!(maker_report1.quantity, 10);
    assert_eq!(maker_report1.remaining_quantity, 0);
    
    // 두번째 매칭 (50주)
    let taker_report2 = exec_rx.try_recv().unwrap();
    let maker_report2 = exec_rx.try_recv().unwrap();
    
    assert_eq!(taker_report2.order_id, "buy1");
    assert_eq!(taker_report2.quantity, 50);
    assert_eq!(taker_report2.remaining_quantity, 40);
    
    assert_eq!(maker_report2.order_id, "sell2");
    assert_eq!(maker_report2.quantity, 50);
    assert_eq!(maker_report2.remaining_quantity, 0);
    
    // 세번째 매칭 (40주)
    let taker_report3 = exec_rx.try_recv().unwrap();
    let maker_report3 = exec_rx.try_recv().unwrap();
    
    assert_eq!(taker_report3.order_id, "buy1");
    assert_eq!(taker_report3.quantity, 40);
    assert_eq!(taker_report3.remaining_quantity, 0);
    
    assert_eq!(maker_report3.order_id, "sell3");
    assert_eq!(maker_report3.quantity, 40);
    assert_eq!(maker_report3.remaining_quantity, 0);
    
    // 모든 매칭 후 가격 레벨이 제거되었는지 확인
    let snapshot = engine.get_order_book_snapshot("BTC-KRW", 5).unwrap();
    assert!(snapshot.asks.is_empty()); // 모든 매도 주문이 체결되어 없어야 함
  }
  
  #[test]
  fn test_price_time_priority() {
    // 채널 생성
    let (exec_tx, exec_rx) = mpsc::channel();
    
    // 매칭 엔진 생성
    let symbols = vec!["BTC-KRW".to_string()];
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 다양한 가격의 매도 주문 추가
    let sell_order1 = create_test_order("sell1", Side::Sell, OrderType::Limit, 10100, 30);
    let sell_order2 = create_test_order("sell2", Side::Sell, OrderType::Limit, 10000, 20); // 최저가
    let sell_order3 = create_test_order("sell3", Side::Sell, OrderType::Limit, 10200, 40);
    
    engine.process_order(sell_order1);
    engine.process_order(sell_order2);
    engine.process_order(sell_order3);
    
    // 매수 주문 (시장가)
    let buy_order = create_test_order("buy1", Side::Buy, OrderType::Market, 0, 30);
    engine.process_order(buy_order);
    
    // 첫번째 매칭 (가격 우선 - 최저가 매칭)
    let taker_report1 = exec_rx.try_recv().unwrap();
    let maker_report1 = exec_rx.try_recv().unwrap();
    
    assert_eq!(taker_report1.order_id, "buy1");
    assert_eq!(taker_report1.price, 10000); // 최저가 매칭
    assert_eq!(taker_report1.quantity, 20);
    
    assert_eq!(maker_report1.order_id, "sell2");
    assert_eq!(maker_report1.quantity, 20);
    assert_eq!(maker_report1.remaining_quantity, 0);
    
    // 두번째 매칭 (다음 최저가 매칭)
    let taker_report2 = exec_rx.try_recv().unwrap();
    let maker_report2 = exec_rx.try_recv().unwrap();
    
    assert_eq!(taker_report2.order_id, "buy1");
    assert_eq!(taker_report2.price, 10100); // 다음 최저가 매칭
    assert_eq!(taker_report2.quantity, 10);
    assert_eq!(taker_report2.remaining_quantity, 0);
    
    assert_eq!(maker_report2.order_id, "sell1");
    assert_eq!(maker_report2.quantity, 10);
    assert_eq!(maker_report2.remaining_quantity, 20); // 30 - 10 = 20
  }
}