//! 주문장 구현
//!
//! 이 모듈은 주문장(Order Book)과 가격 레벨(Price Level)을 구현합니다.
//! 주문장은 매수/매도 호가를 관리하고, 가격 레벨은 특정 가격의 주문들을 처리합니다.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::cmp::Reverse;
use log::{debug, trace};

use crate::matching_engine::model::{Order, Side, OrderBookSnapshot};
use crate::util::linked_list::{DoublyLinkedList, Node};

/// 가격 레벨(Price Level) 구현
/// 특정 가격에 대한 모든 주문을 관리합니다.
#[derive(Debug)]
pub struct PriceLevel {
  /// 주문 리스트 (시간 우선순위)
  orders: DoublyLinkedList<Order>,
  /// 주문 ID → 노드 참조 맵핑
  order_map: HashMap<String, Arc<Mutex<Node<Order>>>>,
  /// 총 주문 수량
  pub total_volume: u64,
}

impl PriceLevel {
  /// 새 가격 레벨 생성
  pub fn new() -> Self {
    PriceLevel {
      orders: DoublyLinkedList::new(),
      order_map: HashMap::new(),
      total_volume: 0,
    }
  }
  
  /// 주문 추가
  pub fn add_order(&mut self, order: Order) {
    let order_id = order.id.clone();
    let quantity = order.remaining_quantity;
    
    // 주문 추가 (Order 객체만 전달)
    let node = self.orders.push_back(order);
    
    // 주문 ID → 노드 참조 매핑 저장
    self.order_map.insert(order_id, node);
    
    // 총 수량 업데이트
    self.total_volume += quantity;
  }
  
  /// 주문 취소
  pub fn remove_order(&mut self, order_id: &str) -> Option<Order> {
    // 주문 ID로 노드 찾기
    if let Some(node) = self.order_map.remove(order_id) {
      // 총 수량에서 주문 수량 차감
      let quantity = if let Ok(node_guard) = node.lock() {
        node_guard.value.remaining_quantity
      } else {
        0
      };
      self.total_volume = self.total_volume.saturating_sub(quantity);
      
      // 링크드 리스트에서 노드 제거
      self.orders.remove(node.clone());
      
      // 노드에서 주문 객체 추출
      let order = if let Ok(node_guard) = node.lock() {
        node_guard.value.clone()
      } else {
        return None;
      };
      Some(order)
    } else {
      None
    }
  }
  
  /// 첫 번째 주문 정보 조회 (제거하지 않음)
  pub fn peek_front(&self) -> Option<(String, Order, u64)> {
    if let Some(node) = self.orders.peek_front() {
      if let Ok(node_guard) = node.lock() {
        let order = node_guard.value.clone();
        let order_id = order.id.clone();
        let quantity = order.remaining_quantity;
        
        Some((order_id, order, quantity))
      } else {
        None
      }
    } else {
      None
    }
  }
  
  /// 첫 번째 주문 수량 조회
  pub fn get_front_quantity(&self) -> Option<u64> {
    self.orders.peek_front().and_then(|node| {
      node.lock().ok().map(|node_guard| node_guard.value.remaining_quantity)
    })
  }
  
  pub fn match_partial(&mut self, match_quantity: u64) -> Option<(String, Order, u64)> {
    if let Some(node) = self.orders.peek_front() {
      let mut cloned_maker = if let Ok(node_guard) = node.lock() {
        node_guard.value.clone()
      } else {
        return None;
      };
      
      let maker_order_id = cloned_maker.id.clone();
      let maker_current_quantity = cloned_maker.remaining_quantity;
      
      // 실제 체결 수량 계산 (요청된 체결량과 현재 수량 중 작은 값)
      let actual_match = std::cmp::min(match_quantity, maker_current_quantity);
      
      // 총 수량 업데이트
      self.total_volume = self.total_volume.saturating_sub(actual_match);
      
      // 원본 주문 객체에 체결 처리
      cloned_maker.fill(actual_match);
      
      if actual_match >= maker_current_quantity {
        // 완전 체결 - 주문 제거
        self.order_map.remove(&maker_order_id);
        
        // 링크드 리스트에서 첫 번째 노드를 제거
        if let Some(front_node) = self.orders.peek_front() {
          self.orders.remove(front_node.clone());
        }
        
        trace!("주문 완전 체결: {}, 체결량: {}", maker_order_id, actual_match);
      } else {
        // 부분 체결 - 노드의 주문 객체 업데이트
        if let Ok(mut node_guard) = node.lock() {
          node_guard.value.remaining_quantity = cloned_maker.remaining_quantity;
        }
        
        trace!("주문 부분 체결: {}, 체결량: {}, 남은양: {}",
                    maker_order_id, actual_match, cloned_maker.remaining_quantity);
      }
      
      Some((maker_order_id, cloned_maker, actual_match))
    } else {
      None
    }
  }
  
  /// 가격 레벨이 비었는지 확인
  pub fn is_empty(&self) -> bool {
    self.orders.is_empty()
  }
  
  /// 주문 수 반환
  pub fn len(&self) -> usize {
    self.orders.len()
  }
}

/// 주문장(Order Book) 구현
#[derive(Debug)]
pub struct OrderBook {
  /// 심볼
  pub symbol: String,
  /// 매수 호가 (가격 → 가격 레벨)
  /// 내림차순 (높은 가격이 앞에 위치)
  pub bids: BTreeMap<Reverse<u64>, PriceLevel>,
  /// 매도 호가 (가격 → 가격 레벨)
  /// 오름차순 (낮은 가격이 앞에 위치)
  pub asks: BTreeMap<u64, PriceLevel>,
  /// 모든 주문 정보 (주문 ID → (Side, 가격))
  /// 수량은 Order 객체에서 직접 참조하므로 저장하지 않음
  pub orders: HashMap<String, (Side, u64)>,
}

impl OrderBook {
  /// 새 주문장 생성
  pub fn new(symbol: String) -> Self {
    OrderBook {
      symbol,
      bids: BTreeMap::new(),  // 내림차순 (Reverse 사용)
      asks: BTreeMap::new(),  // 오름차순
      orders: HashMap::new(),
    }
  }
  
  /// 주문 추가
  pub fn add_order(&mut self, order: Order) -> bool {
    let order_id = order.id.clone();
    let side = order.side.clone();
    let price = order.price;
    let qty = order.remaining_quantity;
    
    match side {
      Side::Buy => {
        // 매수 주문 - 내림차순으로 저장 (Reverse 사용)
        let price_level = self.bids.entry(Reverse(price)).or_insert_with(PriceLevel::new);
        price_level.add_order(order);
        self.orders.insert(order_id.clone(), (Side::Buy, price));
        
        debug!("매수 주문 추가: {} (가격: {}, 수량: {})", order_id, price, qty);
      },
      Side::Sell => {
        // 매도 주문 - 오름차순으로 저장
        let price_level = self.asks.entry(price).or_insert_with(PriceLevel::new);
        price_level.add_order(order);
        self.orders.insert(order_id.clone(), (Side::Sell, price));
        
        debug!("매도 주문 추가: {} (가격: {}, 수량: {})", order_id, price, qty);
      }
    }
    
    true
  }
  
  /// 주문 취소
  pub fn cancel_order(&mut self, order_id: &str) -> Option<Order> {
    if let Some((side, price)) = self.orders.remove(order_id) {
      match side {
        Side::Buy => {
          // 매수 주문 취소
          if let Some(price_level) = self.bids.get_mut(&Reverse(price)) {
            let order = price_level.remove_order(order_id);
            
            // 가격 레벨이 비었으면 제거
            if price_level.is_empty() {
              self.bids.remove(&Reverse(price));
              debug!("빈 가격 레벨 제거 (매수): {}", price);
            }
            
            debug!("매수 주문 취소: {} (가격: {})", order_id, price);
            return order;
          }
        },
        Side::Sell => {
          // 매도 주문 취소
          if let Some(price_level) = self.asks.get_mut(&price) {
            let order = price_level.remove_order(order_id);
            
            // 가격 레벨이 비었으면 제거
            if price_level.is_empty() {
              self.asks.remove(&price);
              debug!("빈 가격 레벨 제거 (매도): {}", price);
            }
            
            debug!("매도 주문 취소: {} (가격: {})", order_id, price);
            return order;
          }
        }
      }
    }
    
    debug!("취소 실패 - 주문 없음: {}", order_id);
    None
  }
  
  /// 특정 가격의 매도 가격 레벨 조회
  pub fn get_ask_price_level(&mut self, price: u64) -> Option<&mut PriceLevel> {
    self.asks.get_mut(&price)
  }
  
  /// 특정 가격의 매수 가격 레벨 조회
  pub fn get_bid_price_level(&mut self, price: u64) -> Option<&mut PriceLevel> {
    self.bids.get_mut(&Reverse(price))
  }
  
  /// 최고 매수가 조회
  pub fn get_best_bid(&self) -> Option<(u64, &PriceLevel)> {
    self.bids.iter().next().map(|(price, level)| (price.0, level))
  }
  
  /// 최저 매도가 조회
  pub fn get_best_ask(&self) -> Option<(u64, &PriceLevel)> {
    self.asks.iter().next().map(|(price, level)| (*price, level))
  }
  
  /// 주문장 스냅샷 생성
  pub fn get_order_book_snapshot(&self, depth: usize) -> OrderBookSnapshot {
    let mut bids = Vec::with_capacity(depth);
    let mut asks = Vec::with_capacity(depth);
    
    // 매수 호가 스냅샷 (내림차순)
    for (i, (price, level)) in self.bids.iter().enumerate() {
      if i >= depth {
        break;
      }
      bids.push((price.0, level.total_volume));
    }
    
    // 매도 호가 스냅샷 (오름차순)
    for (i, (price, level)) in self.asks.iter().enumerate() {
      if i >= depth {
        break;
      }
      asks.push((*price, level.total_volume));
    }
    
    OrderBookSnapshot {
      symbol: self.symbol.clone(),
      bids,
      asks,
    }
  }
  
  /// 심볼 조회
  pub fn symbol(&self) -> &str {
    &self.symbol
  }
  
  /// 매수 주문 수 조회
  pub fn bid_count(&self) -> usize {
    self.bids.values().map(|level| level.len()).sum()
  }
  
  /// 매도 주문 수 조회
  pub fn ask_count(&self) -> usize {
    self.asks.values().map(|level| level.len()).sum()
  }
  
  /// 총 주문 수 조회
  pub fn order_count(&self) -> usize {
    self.bid_count() + self.ask_count()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::matching_engine::model::{Order, Side, OrderType};
  use uuid::Uuid;
  
  // 테스트용 주문 생성 헬퍼 함수
  fn create_test_order(side: Side, price: u64, quantity: u64) -> Order {
    Order {
      id: Uuid::new_v4().to_string(),
      symbol: "BTC-KRW".to_string(),
      side,
      order_type: OrderType::Limit,
      price,
      quantity,
      remaining_quantity: quantity,
      client_id: "".to_string(),
      timestamp: 0,
      is_cancel: false,
      target_order_id: None,
    }
  }
  
  #[test]
  fn test_price_level_add_order() {
    let mut price_level = PriceLevel::new();
    
    // 주문 추가
    let order = create_test_order(Side::Buy, 1000, 100);
    let order_id = order.id.clone();
    price_level.add_order(order);
    
    // 검증
    assert_eq!(price_level.total_volume, 100);
    assert_eq!(price_level.len(), 1);
    
    // 두번째 주문 추가
    let order2 = create_test_order(Side::Buy, 1000, 200);
    price_level.add_order(order2);
    
    // 검증
    assert_eq!(price_level.total_volume, 300);
    assert_eq!(price_level.len(), 2);
  }
  
  #[test]
  fn test_price_level_remove_order() {
    let mut price_level = PriceLevel::new();
    
    // 주문 추가
    let order = create_test_order(Side::Buy, 1000, 100);
    let order_id = order.id.clone();
    price_level.add_order(order);
    
    // 주문 제거
    let removed = price_level.remove_order(&order_id);
    
    // 검증
    assert!(removed.is_some());
    assert_eq!(price_level.total_volume, 0);
    assert_eq!(price_level.len(), 0);
    assert!(price_level.is_empty());
  }
  
  #[test]
  fn test_price_level_match_partial() {
    let mut price_level = PriceLevel::new();
    
    // 주문 추가 (100주)
    let order = create_test_order(Side::Buy, 1000, 100);
    let order_id = order.id.clone();
    price_level.add_order(order);
    
    // 부분 체결 (40주)
    let result = price_level.match_partial(40);
    
    // 검증
    assert!(result.is_some());
    let (matched_id, matched_order, matched_qty) = result.unwrap();
    assert_eq!(matched_id, order_id);
    assert_eq!(matched_qty, 40);
    assert_eq!(matched_order.remaining_quantity, 60);
    assert_eq!(price_level.total_volume, 60);
    assert_eq!(price_level.len(), 1);
    
    // 완전 체결 (남은 60주)
    let result = price_level.match_partial(60);
    
    // 검증
    assert!(result.is_some());
    let (matched_id, matched_order, matched_qty) = result.unwrap();
    assert_eq!(matched_id, order_id);
    assert_eq!(matched_qty, 60);
    assert_eq!(matched_order.remaining_quantity, 0);
    assert_eq!(price_level.total_volume, 0);
    assert_eq!(price_level.len(), 0);
    assert!(price_level.is_empty());
  }
  
  #[test]
  fn test_order_book_add_orders() {
    let mut order_book = OrderBook::new("BTC-KRW".to_string());
    
    // 매수 주문 추가
    let buy_order = create_test_order(Side::Buy, 9000, 100);
    let buy_id = buy_order.id.clone();
    order_book.add_order(buy_order);
    
    // 매도 주문 추가
    let sell_order = create_test_order(Side::Sell, 10000, 200);
    let sell_id = sell_order.id.clone();
    order_book.add_order(sell_order);
    
    // 검증
    assert_eq!(order_book.bid_count(), 1);
    assert_eq!(order_book.ask_count(), 1);
    assert_eq!(order_book.order_count(), 2);
    
    // 베스트 호가 검증
    let (best_bid, _) = order_book.get_best_bid().unwrap();
    let (best_ask, _) = order_book.get_best_ask().unwrap();
    assert_eq!(best_bid, 9000);
    assert_eq!(best_ask, 10000);
  }
  
  #[test]
  fn test_order_book_cancel_order() {
    let mut order_book = OrderBook::new("BTC-KRW".to_string());
    
    // 매수 주문 추가
    let buy_order = create_test_order(Side::Buy, 9000, 100);
    let buy_id = buy_order.id.clone();
    order_book.add_order(buy_order);
    
    // 주문 취소
    let canceled = order_book.cancel_order(&buy_id);
    
    // 검증
    assert!(canceled.is_some());
    assert_eq!(order_book.bid_count(), 0);
    assert!(order_book.bids.is_empty());
  }
  
  #[test]
  fn test_order_book_multiple_price_levels() {
    let mut order_book = OrderBook::new("BTC-KRW".to_string());
    
    // 다양한 가격의 매수 주문 추가
    let buy_order1 = create_test_order(Side::Buy, 9000, 100);
    order_book.add_order(buy_order1);
    
    let buy_order2 = create_test_order(Side::Buy, 9100, 200);
    order_book.add_order(buy_order2);
    
    let buy_order3 = create_test_order(Side::Buy, 8900, 300);
    order_book.add_order(buy_order3);
    
    // 다양한 가격의 매도 주문 추가
    let sell_order1 = create_test_order(Side::Sell, 10000, 100);
    order_book.add_order(sell_order1);
    
    let sell_order2 = create_test_order(Side::Sell, 9900, 200);
    order_book.add_order(sell_order2);
    
    let sell_order3 = create_test_order(Side::Sell, 10100, 300);
    order_book.add_order(sell_order3);
    
    // 검증
    assert_eq!(order_book.bid_count(), 3);
    assert_eq!(order_book.ask_count(), 3);
    
    // 베스트 호가 검증 (최고 매수가, 최저 매도가)
    let (best_bid, _) = order_book.get_best_bid().unwrap();
    let (best_ask, _) = order_book.get_best_ask().unwrap();
    assert_eq!(best_bid, 9100);  // 매수는 높은 가격이 우선
    assert_eq!(best_ask, 9900);  // 매도는 낮은 가격이 우선
    
    // 스냅샷 검증
    let snapshot = order_book.get_order_book_snapshot(10);
    assert_eq!(snapshot.bids.len(), 3);
    assert_eq!(snapshot.asks.len(), 3);
    
    // 매수 호가는 내림차순 정렬
    assert_eq!(snapshot.bids[0].0, 9100);
    assert_eq!(snapshot.bids[1].0, 9000);
    assert_eq!(snapshot.bids[2].0, 8900);
    
    // 매도 호가는 오름차순 정렬
    assert_eq!(snapshot.asks[0].0, 9900);
    assert_eq!(snapshot.asks[1].0, 10000);
    assert_eq!(snapshot.asks[2].0, 10100);
  }
  
  #[test]
  fn test_same_price_level() {
    let mut order_book = OrderBook::new("BTC-KRW".to_string());
    
    // 동일 가격에 여러 매수 주문 추가
    let buy_order1 = create_test_order(Side::Buy, 9000, 100);
    let buy_id1 = buy_order1.id.clone();
    order_book.add_order(buy_order1);
    
    let buy_order2 = create_test_order(Side::Buy, 9000, 200);
    order_book.add_order(buy_order2);
    
    // 검증
    assert_eq!(order_book.bid_count(), 2);
    
    // 가격 레벨 가져오기
    let price_level = order_book.get_bid_price_level(9000).unwrap();
    assert_eq!(price_level.total_volume, 300);
    assert_eq!(price_level.len(), 2);
    
    // 첫번째 주문 취소
    let canceled = order_book.cancel_order(&buy_id1);
    assert!(canceled.is_some());
    
    // 가격 레벨은 여전히 존재
    assert!(!order_book.bids.is_empty());
    let price_level = order_book.get_bid_price_level(9000).unwrap();
    assert_eq!(price_level.total_volume, 200);
    assert_eq!(price_level.len(), 1);
  }
}