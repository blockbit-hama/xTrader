//! 주문 시퀀서 구현
//!
//! 이 모듈은 주문의 순서를 보장하고 매칭 엔진으로 전달하는 역할을 담당합니다.
//! FIFO(First-In-First-Out) 방식으로 주문을 처리하여 공정한 거래 환경을 제공합니다.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{debug, info, warn, error};
use uuid::Uuid;
use sqlx::sqlite::SqlitePool;

use crate::matching_engine::model::{Order, ExecutionReport, OrderBookSnapshot};
use crate::api::models::WebSocketMessage;
use crate::mdp::model::{MarketStatistics, CandlestickData};
use crate::mdp::MarketDataPublisher;
use crate::db::models::ExecutionRecord;
use crate::db::repository::ExecutionRepository;
use crate::db::AsyncCommitManager;

/// 주문 시퀀서
pub struct OrderSequencer {
    /// 주문 수신 채널 (API에서 받음)
    order_rx: Receiver<Order>,
    /// 매칭 엔진으로의 주문 전송 채널
    engine_tx: Sender<Order>,
    /// 체결 보고서 수신 채널 (매칭 엔진에서 받음)
    exec_rx: Receiver<ExecutionReport>,
    /// WebSocket 메시지 브로드캐스트 채널
    broadcast_tx: tokio::sync::broadcast::Sender<WebSocketMessage>,
    /// 시장 데이터 발행자 참조
    mdp: Arc<Mutex<MarketDataPublisher>>,
    /// 비동기 커밋 매니저 (초고성능 DB 저장)
    async_commit_mgr: Arc<AsyncCommitManager>,
    /// 시퀀서 ID (로깅용)
    sequencer_id: String,
    /// 처리된 주문 수
    processed_orders: Arc<Mutex<u64>>,
}

impl OrderSequencer {
    /// 새 시퀀서 생성
    pub fn new(
        order_rx: Receiver<Order>,
        engine_tx: Sender<Order>,
        exec_rx: Receiver<ExecutionReport>,
        broadcast_tx: tokio::sync::broadcast::Sender<WebSocketMessage>,
        mdp: Arc<Mutex<MarketDataPublisher>>,
        async_commit_mgr: Arc<AsyncCommitManager>,
    ) -> Self {
        Self {
            order_rx,
            engine_tx,
            exec_rx,
            broadcast_tx,
            mdp,
            async_commit_mgr,
            sequencer_id: Uuid::new_v4().to_string(),
            processed_orders: Arc::new(Mutex::new(0)),
        }
    }

    /// 시퀀서 실행
    pub async fn run(&mut self) {
        info!("시퀀서 시작: {}", self.sequencer_id);

        // 주문 처리 태스크
        let order_task = {
            let processed_orders = self.processed_orders.clone();
            let sequencer_id = self.sequencer_id.clone();
            let engine_tx = self.engine_tx.clone();
            let mut order_rx = std::mem::replace(&mut self.order_rx, unsafe { std::mem::zeroed() });

            tokio::spawn(async move {
                while let Ok(order) = order_rx.recv() {
                    debug!("시퀀서 {}: 주문 수신 - {}", sequencer_id, order.id);
                    
                    // 매칭 엔진으로 주문 전달
                    match engine_tx.send(order.clone()) {
                        Ok(_) => {
                            let mut count = processed_orders.lock().await;
                            *count += 1;
                            debug!("시퀀서 {}: 주문 전달 완료 - {} (총 처리: {})", 
                                   sequencer_id, order.id, *count);
                        }
                        Err(e) => {
                            error!("시퀀서 {}: 주문 전달 실패 - {}: {}", 
                                   sequencer_id, order.id, e);
                        }
                    }
                }
                info!("시퀀서 {}: 주문 처리 종료", sequencer_id);
            })
        };

    // 체결 보고서 브로드캐스트 태스크
    let broadcast_task = {
      let sequencer_id = self.sequencer_id.clone();
      let broadcast_tx = self.broadcast_tx.clone();
      let mdp = self.mdp.clone();
      let async_commit_mgr = self.async_commit_mgr.clone();
      let mut exec_rx = std::mem::replace(&mut self.exec_rx, unsafe { std::mem::zeroed() });

      tokio::spawn(async move {
        while let Ok(report) = exec_rx.recv() {
          debug!("시퀀서 {}: 체결 보고서 수신 - {}", sequencer_id, report.execution_id);

          // 🚀 초고성능: 체결 내역을 비차단 큐에 추가 (즉시 반환)
          let exec_record = ExecutionRecord {
            exec_id: report.execution_id.clone(),
            taker_order_id: report.order_id.clone(),
            maker_order_id: "maker_placeholder".to_string(), // TODO: 실제 maker order id 추가 필요
            symbol: report.symbol.clone(),
            side: format!("{:?}", report.side),
            price: report.price as i64,
            quantity: report.quantity as i64,
            taker_fee: 0, // TODO: 수수료 계산 로직 추가 필요
            maker_fee: 0, // TODO: 수수료 계산 로직 추가 필요
            transaction_time: report.timestamp as i64,
          };

          // 비동기 큐에 추가 (마이크로초 단위 지연)
          async_commit_mgr.enqueue(exec_record).await;
          debug!("시퀀서 {}: 체결 내역 비동기 큐 추가 - {}", sequencer_id, report.execution_id);

          // MDP로 체결 데이터 전달
          {
            let mut mdp_guard = mdp.lock().await;
            mdp_guard.process_execution(report.clone()).await;
          }

          // WebSocket 메시지로 변환하여 브로드캐스트
          let message = WebSocketMessage::Execution(report.clone());
          match broadcast_tx.send(message) {
            Ok(receiver_count) => {
              debug!("시퀀서 {}: 체결 보고서 브로드캐스트 완료 - {} (수신자: {})",
                     sequencer_id, report.execution_id, receiver_count);
            }
            Err(e) => {
              warn!("시퀀서 {}: 체결 보고서 브로드캐스트 실패 - {}: {}",
                    sequencer_id, report.execution_id, e);
            }
          }
        }
        info!("시퀀서 {}: 체결 보고서 브로드캐스트 종료", sequencer_id);
      })
    };

        // 두 태스크 모두 완료될 때까지 대기
        tokio::select! {
            _ = order_task => {
                info!("시퀀서 {}: 주문 처리 태스크 완료", self.sequencer_id);
            }
            _ = broadcast_task => {
                info!("시퀀서 {}: 브로드캐스트 태스크 완료", self.sequencer_id);
            }
        }

        info!("시퀀서 종료: {}", self.sequencer_id);
    }

    /// 처리된 주문 수 조회
    pub async fn get_processed_count(&self) -> u64 {
        *self.processed_orders.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tokio::sync::broadcast;
    use crate::matching_engine::model::{Order, OrderType, Side};

    fn create_test_order(id: &str) -> Order {
        Order {
            id: id.to_string(),
            symbol: "BTC-KRW".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: 100000,
            quantity: 100,
            remaining_quantity: 100,
            client_id: "test".to_string(),
            timestamp: 0,
            is_cancel: false,
            target_order_id: None,
        }
    }

    #[tokio::test]
    async fn test_sequencer_order_processing() {
        // 채널 생성
        let (order_tx, order_rx) = mpsc::channel();
        let (engine_tx, engine_rx) = mpsc::channel();
        let (exec_tx, exec_rx) = mpsc::channel();
        let (broadcast_tx, _broadcast_rx) = broadcast::channel(100);

        // MDP 생성 (테스트용)
        let mdp = Arc::new(Mutex::new(MarketDataPublisher::new(1000)));

        // 시퀀서 생성
        let mut sequencer = OrderSequencer::new(
            order_rx,
            engine_tx,
            exec_rx,
            broadcast_tx,
            mdp,
        );

        // 테스트 주문 전송
        let order1 = create_test_order("order1");
        let order2 = create_test_order("order2");

        order_tx.send(order1.clone()).unwrap();
        order_tx.send(order2.clone()).unwrap();

        // 시퀀서 실행 (짧은 시간만)
        let sequencer_task = tokio::spawn(async move {
            sequencer.run().await;
        });

        // 매칭 엔진에서 주문 수신 확인
        let received_order1 = engine_rx.recv().unwrap();
        let received_order2 = engine_rx.recv().unwrap();

        assert_eq!(received_order1.id, "order1");
        assert_eq!(received_order2.id, "order2");

        // 시퀀서 종료
        sequencer_task.abort();
    }

    #[tokio::test]
    async fn test_sequencer_execution_broadcast() {
        // 채널 생성
        let (order_tx, order_rx) = mpsc::channel();
        let (engine_tx, _engine_rx) = mpsc::channel();
        let (exec_tx, exec_rx) = mpsc::channel();
        let (broadcast_tx, mut broadcast_rx) = broadcast::channel(100);

        // MDP 생성 (테스트용)
        let mdp = Arc::new(Mutex::new(MarketDataPublisher::new(1000)));

        // 시퀀서 생성
        let mut sequencer = OrderSequencer::new(
            order_rx,
            engine_tx,
            exec_rx,
            broadcast_tx,
            mdp,
        );

        // 테스트 체결 보고서 전송
        let execution_report = ExecutionReport {
            execution_id: "exec1".to_string(),
            order_id: "order1".to_string(),
            symbol: "BTC-KRW".to_string(),
            side: Side::Buy,
            price: 100000,
            quantity: 100,
            remaining_quantity: 0,
            timestamp: 0,
            counterparty_id: "order2".to_string(),
            is_maker: false,
        };

        exec_tx.send(execution_report.clone()).unwrap();

        // 시퀀서 실행 (짧은 시간만)
        let sequencer_task = tokio::spawn(async move {
            sequencer.run().await;
        });

        // 브로드캐스트 수신 확인
        let received_message = broadcast_rx.recv().await.unwrap();
        match received_message {
            WebSocketMessage::Execution(report) => {
                assert_eq!(report.execution_id, "exec1");
            }
            _ => panic!("Expected Execution message"),
        }

        // 시퀀서 종료
        sequencer_task.abort();
    }
}