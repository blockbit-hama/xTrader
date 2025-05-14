//! 주문 매칭 엔진의 기본 모델
//!
//! 이 모듈은 주문, 주문 타입, 매수/매도 방향, 체결 보고서 등
//! 매칭 엔진의 핵심 데이터 모델을 정의합니다.

use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

/// 매수/매도 방향
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
  /// 매수 주문
  Buy,
  /// 매도 주문
  Sell,
}

/// 주문 타입
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
  /// 시장가 주문 - 현재 시장 가격에 즉시 체결
  Market,
  /// 지정가 주문 - 지정된 가격에서만 체결
  Limit,
}

/// 주문 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
  /// 주문 고유 ID
  pub id: String,
  /// 거래 대상 심볼 (예: "AAPL", "MSFT")
  pub symbol: String,
  /// 매수/매도 방향
  pub side: Side,
  /// 주문 타입 (시장가/지정가)
  pub order_type: OrderType,
  /// 주문 가격 (지정가 주문에만 의미 있음)
  pub price: u64,
  /// 주문 총 수량
  pub quantity: u64,
  /// 남은 미체결 수량
  pub remaining_quantity: u64,
  /// 고객 ID
  pub client_id: String,
  /// 주문 생성 시간 (Unix 타임스탬프)
  pub timestamp: u64,
  /// 취소 주문 여부
  pub is_cancel: bool,
  /// 취소 대상 주문 ID (취소 주문인 경우에만 있음)
  pub target_order_id: Option<String>,
}

impl Order {
  /// 새 주문 생성
  pub fn new(
    id: String,
    symbol: String,
    side: Side,
    order_type: OrderType,
    price: u64,
    quantity: u64,
    client_id: String,
  ) -> Self {
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
    
    Self {
      id,
      symbol,
      side,
      order_type,
      price,
      quantity,
      remaining_quantity: quantity,
      client_id,
      timestamp: now,
      is_cancel: false,
      target_order_id: None,
    }
  }
  
  /// 취소 주문 생성
  pub fn new_cancel(target_order_id: String) -> Self {
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
    
    Self {
      id: format!("cancel-{}", target_order_id),
      symbol: String::new(),
      side: Side::Buy,  // 취소 주문은 side가 의미 없음
      order_type: OrderType::Market,  // 취소 주문은 order_type이 의미 없음
      price: 0,
      quantity: 0,
      remaining_quantity: 0,
      client_id: "system".to_string(),
      timestamp: now,
      is_cancel: true,
      target_order_id: Some(target_order_id),
    }
  }
  
  /// 주문 체결 처리
  pub fn fill(&mut self, quantity: u64) {
    self.remaining_quantity = self.remaining_quantity.saturating_sub(quantity);
  }
  
  /// 주문이 완전히 체결되었는지 확인
  pub fn is_filled(&self) -> bool {
    self.remaining_quantity == 0
  }
}

/// 체결 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
  /// 체결 고유 ID
  pub execution_id: String,
  /// 주문 ID
  pub order_id: String,
  /// 거래 대상 심볼
  pub symbol: String,
  /// 매수/매도 방향
  pub side: Side,
  /// 체결 가격
  pub price: u64,
  /// 체결 수량
  pub quantity: u64,
  /// 남은 미체결 수량
  pub remaining_quantity: u64,
  /// 체결 시간 (Unix 타임스탬프)
  pub timestamp: u64,
  /// 상대방 주문 ID
  pub counterparty_id: String,
  /// 메이커(호가에 있던 주문) 여부
  pub is_maker: bool,
}

/// 주문장 스냅샷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
  /// 심볼
  pub symbol: String,
  /// 매수 호가 목록 [(가격, 수량)]
  pub bids: Vec<(u64, u64)>,
  /// 매도 호가 목록 [(가격, 수량)]
  pub asks: Vec<(u64, u64)>,
}