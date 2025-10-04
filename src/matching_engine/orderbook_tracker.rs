//! 호가창 변경 추적 및 Delta 생성 모듈
//!
//! 이 모듈은 호가창의 변경사항을 추적하고 Delta 업데이트를 생성하는 역할을 담당합니다.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::api::models::{OrderBookChange, OrderBookChangeType, OrderBookDelta, OrderBookSnapshot};
use crate::matching_engine::model::OrderBookSnapshot as EngineOrderBookSnapshot;

/// 호가창 변경 추적기
pub struct OrderBookTracker {
    /// 심볼별 이전 호가창 상태
    previous_snapshots: HashMap<String, Vec<(u64, u64)>>,
    /// 심볼별 시퀀스 번호
    sequence_numbers: HashMap<String, u64>,
    /// Delta 업데이트 임계값 (변경사항이 이 값 이상일 때만 Delta 전송)
    delta_threshold: usize,
    /// Snapshot 전송 간격 (밀리초)
    snapshot_interval_ms: u64,
    /// 마지막 Snapshot 전송 시간
    last_snapshot_time: HashMap<String, u64>,
}

impl OrderBookTracker {
    /// 새 추적기 생성
    pub fn new(delta_threshold: usize, snapshot_interval_ms: u64) -> Self {
        Self {
            previous_snapshots: HashMap::new(),
            sequence_numbers: HashMap::new(),
            delta_threshold,
            snapshot_interval_ms,
            last_snapshot_time: HashMap::new(),
        }
    }

    /// 호가창 변경사항 분석 및 Delta 생성
    pub fn analyze_changes(
        &mut self,
        symbol: &str,
        current_snapshot: &EngineOrderBookSnapshot,
    ) -> Option<OrderBookDelta> {
        let current_bids = &current_snapshot.bids;
        let current_asks = &current_snapshot.asks;
        
        // 이전 상태 가져오기
        let previous_bids = self.previous_snapshots.get(symbol).cloned().unwrap_or_default();
        let previous_asks = self.previous_snapshots.get(symbol).cloned().unwrap_or_default();
        
        // 변경사항 분석
        let bid_changes = self.analyze_price_level_changes(&previous_bids, current_bids);
        let ask_changes = self.analyze_price_level_changes(&previous_asks, current_asks);
        
        // 변경사항이 임계값 미만이면 Delta 전송하지 않음
        if bid_changes.len() + ask_changes.len() < self.delta_threshold {
            return None;
        }
        
        // 시퀀스 번호 증가
        let sequence = self.sequence_numbers.entry(symbol.to_string()).or_insert(0);
        *sequence += 1;
        
        // 현재 상태 저장
        self.previous_snapshots.insert(symbol.to_string(), current_bids.clone());
        self.previous_snapshots.insert(format!("{}_asks", symbol), current_asks.clone());
        
        // Delta 생성
        Some(OrderBookDelta {
            symbol: symbol.to_string(),
            bid_changes,
            ask_changes,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            sequence: *sequence,
        })
    }

    /// Snapshot 전송이 필요한지 확인
    pub fn should_send_snapshot(&mut self, symbol: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let last_time = self.last_snapshot_time.get(symbol).copied().unwrap_or(0);
        
        if now - last_time >= self.snapshot_interval_ms {
            self.last_snapshot_time.insert(symbol.to_string(), now);
            true
        } else {
            false
        }
    }

    /// Snapshot 생성
    pub fn create_snapshot(
        &mut self,
        symbol: &str,
        current_snapshot: &EngineOrderBookSnapshot,
    ) -> OrderBookSnapshot {
        // 시퀀스 번호 증가
        let sequence = self.sequence_numbers.entry(symbol.to_string()).or_insert(0);
        *sequence += 1;
        
        // 현재 상태 저장
        self.previous_snapshots.insert(symbol.to_string(), current_snapshot.bids.clone());
        self.previous_snapshots.insert(format!("{}_asks", symbol), current_snapshot.asks.clone());
        
        OrderBookSnapshot {
            symbol: symbol.to_string(),
            bids: current_snapshot.bids.clone(),
            asks: current_snapshot.asks.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            sequence: *sequence,
        }
    }

    /// 가격 레벨 변경사항 분석
    fn analyze_price_level_changes(
        &self,
        previous: &[(u64, u64)],
        current: &[(u64, u64)],
    ) -> Vec<OrderBookChange> {
        let mut changes = Vec::new();
        
        // 이전 상태를 HashMap으로 변환
        let mut prev_map: HashMap<u64, u64> = previous.iter().cloned().collect();
        
        // 현재 상태와 비교
        for &(price, quantity) in current {
            match prev_map.remove(&price) {
                Some(prev_quantity) => {
                    // 가격이 존재하지만 수량이 변경됨
                    if prev_quantity != quantity {
                        changes.push(OrderBookChange {
                            change_type: OrderBookChangeType::Update,
                            price,
                            quantity,
                        });
                    }
                }
                None => {
                    // 새로운 가격 레벨 추가
                    changes.push(OrderBookChange {
                        change_type: OrderBookChangeType::Add,
                        price,
                        quantity,
                    });
                }
            }
        }
        
        // 제거된 가격 레벨들
        for (price, _quantity) in prev_map {
            changes.push(OrderBookChange {
                change_type: OrderBookChangeType::Remove,
                price,
                quantity: 0, // 제거된 경우 수량은 0
            });
        }
        
        changes
    }

    /// 심볼별 시퀀스 번호 조회
    pub fn get_sequence(&self, symbol: &str) -> u64 {
        self.sequence_numbers.get(symbol).copied().unwrap_or(0)
    }

    /// 심볼별 마지막 Snapshot 시간 조회
    pub fn get_last_snapshot_time(&self, symbol: &str) -> u64 {
        self.last_snapshot_time.get(symbol).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching_engine::model::OrderBookSnapshot as EngineOrderBookSnapshot;

    fn create_test_snapshot(bids: Vec<(u64, u64)>, asks: Vec<(u64, u64)>) -> EngineOrderBookSnapshot {
        EngineOrderBookSnapshot {
            symbol: "BTC-KRW".to_string(),
            bids,
            asks,
        }
    }

    #[test]
    fn test_delta_generation() {
        let mut tracker = OrderBookTracker::new(1, 1000);
        
        // 초기 호가창
        let initial_snapshot = create_test_snapshot(
            vec![(50000, 100), (49900, 200)],
            vec![(50100, 150), (50200, 300)],
        );
        
        // 변경된 호가창
        let changed_snapshot = create_test_snapshot(
            vec![(50000, 150), (49900, 200), (49800, 100)], // 50000 수량 변경, 49800 추가
            vec![(50100, 150), (50300, 200)], // 50200 제거, 50300 추가
        );
        
        // 첫 번째 분석 (이전 상태가 없으므로 Delta 생성 안됨)
        let delta1 = tracker.analyze_changes("BTC-KRW", &initial_snapshot);
        assert!(delta1.is_none());
        
        // 두 번째 분석 (변경사항 있음)
        let delta2 = tracker.analyze_changes("BTC-KRW", &changed_snapshot);
        assert!(delta2.is_some());
        
        let delta = delta2.unwrap();
        assert_eq!(delta.symbol, "BTC-KRW");
        assert_eq!(delta.sequence, 1);
        
        // 매수 변경사항 확인
        assert_eq!(delta.bid_changes.len(), 2);
        assert!(delta.bid_changes.iter().any(|c| c.price == 50000 && c.quantity == 150));
        assert!(delta.bid_changes.iter().any(|c| c.price == 49800 && c.quantity == 100));
        
        // 매도 변경사항 확인
        assert_eq!(delta.ask_changes.len(), 2);
        assert!(delta.ask_changes.iter().any(|c| c.price == 50200 && c.change_type == OrderBookChangeType::Remove));
        assert!(delta.ask_changes.iter().any(|c| c.price == 50300 && c.change_type == OrderBookChangeType::Add));
    }

    #[test]
    fn test_snapshot_generation() {
        let mut tracker = OrderBookTracker::new(1, 1000);
        
        let snapshot = create_test_snapshot(
            vec![(50000, 100), (49900, 200)],
            vec![(50100, 150), (50200, 300)],
        );
        
        let orderbook_snapshot = tracker.create_snapshot("BTC-KRW", &snapshot);
        
        assert_eq!(orderbook_snapshot.symbol, "BTC-KRW");
        assert_eq!(orderbook_snapshot.bids.len(), 2);
        assert_eq!(orderbook_snapshot.asks.len(), 2);
        assert_eq!(orderbook_snapshot.sequence, 1);
    }

    #[test]
    fn test_snapshot_interval() {
        let mut tracker = OrderBookTracker::new(1, 1000);
        
        // 첫 번째 확인
        assert!(tracker.should_send_snapshot("BTC-KRW"));
        
        // 즉시 다시 확인 (간격이 지나지 않았으므로 false)
        assert!(!tracker.should_send_snapshot("BTC-KRW"));
    }
}
