# 매칭 엔진 아키텍처 및 자료구조 설계

## 개요

이 문서는 xTrader의 매칭 엔진(Matching Engine) 구현에 대한 상세한 기술 문서입니다. 매칭 엔진은 주식/암호화폐 거래소의 핵심 컴포넌트로, 주문을 받아서 매칭하고 체결하는 역할을 담당합니다.

## 목차

1. [전체 아키텍처](#전체-아키텍처)
2. [핵심 자료구조](#핵심-자료구조)
3. [매칭 알고리즘](#매칭-알고리즘)
4. [성능 최적화](#성능-최적화)
5. [동시성 처리](#동시성-처리)
6. [테스트 전략](#테스트-전략)

---

## 전체 아키텍처

### 시스템 구성도

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   API Server    │───▶│  Matching Engine │───▶│  Execution      │
│                 │    │                 │    │  Reports        │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Order Queue   │    │   Order Books   │    │  WebSocket      │
│   (Channel)     │    │   (Per Symbol)  │    │  Broadcast      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### 핵심 컴포넌트

1. **MatchingEngine**: 메인 매칭 엔진 클래스
2. **OrderBook**: 심볼별 주문장 관리
3. **PriceLevel**: 특정 가격의 주문들을 관리하는 가격 레벨
4. **DoublyLinkedList**: 시간 우선순위를 위한 이중 연결 리스트

---

## 핵심 자료구조

### 1. MatchingEngine 구조체

```rust
pub struct MatchingEngine {
    order_books: HashMap<String, OrderBook>,           // 심볼별 주문장
    order_store: HashMap<String, Order>,               // 주문 ID → 주문 객체
    exec_tx: Sender<ExecutionReport>,                  // 체결 보고서 송신 채널
    broadcast_tx: Option<tokio::sync::broadcast::Sender<WebSocketMessage>>, // WebSocket 브로드캐스트
}
```

#### 설계 이유

- **HashMap<String, OrderBook>**: 심볼별로 독립적인 주문장을 관리
  - O(1) 조회 성능
  - 메모리 효율적
  - 확장성 (새로운 심볼 추가 용이)

- **HashMap<String, Order>**: 주문 ID로 빠른 주문 조회
  - 취소 주문 처리 시 원본 주문 빠른 검색
  - 체결 보고서 생성 시 주문 정보 참조

### 2. OrderBook 구조체

```rust
pub struct OrderBook {
    symbol: String,                                    // 거래 심볼
    bids: BTreeMap<Reverse<u64>, PriceLevel>,         // 매수 호가 (내림차순)
    asks: BTreeMap<u64, PriceLevel>,                  // 매도 호가 (오름차순)
    orders: HashMap<String, (Side, u64)>,             // 주문 ID → (방향, 가격)
}
```

#### 설계 이유

- **BTreeMap<Reverse<u64>, PriceLevel>** (매수):
  - 내림차순 정렬 (높은 가격이 우선)
  - O(log n) 삽입/삭제/조회 성능
  - 자동 정렬로 베스트 매수가 항상 첫 번째 요소

- **BTreeMap<u64, PriceLevel>** (매도):
  - 오름차순 정렬 (낮은 가격이 우선)
  - 베스트 매도가 항상 첫 번째 요소
  - 가격 레벨별 효율적 관리

- **HashMap<String, (Side, u64)>**:
  - 주문 취소 시 빠른 위치 찾기
  - 메모리 효율적 (주문 객체 전체 저장하지 않음)

### 3. PriceLevel 구조체

```rust
pub struct PriceLevel {
    orders: DoublyLinkedList<Order>,                   // 시간 우선순위 주문 리스트
    order_map: HashMap<String, Arc<Mutex<Node<Order>>>>, // 주문 ID → 노드 참조
    total_volume: u64,                                // 총 주문 수량
}
```

#### 설계 이유

- **DoublyLinkedList<Order>**:
  - 시간 우선순위 보장 (FIFO)
  - O(1) 삽입/삭제 성능
  - 부분 체결 시 효율적 처리

- **HashMap<String, Arc<Mutex<Node<Order>>>>**:
  - 주문 취소 시 O(1) 노드 찾기
  - Arc<Mutex<>>로 스레드 안전성 보장
  - 노드 참조로 빠른 삭제

- **total_volume**:
  - 가격 레벨별 총 수량 빠른 조회
  - 호가창 스냅샷 생성 시 효율적

### 4. DoublyLinkedList 구조체

```rust
pub struct DoublyLinkedList<T> {
    head: Option<Arc<Mutex<Node<T>>>>,
    tail: Option<Arc<Mutex<Node<T>>>>,
    len: usize,
}

pub struct Node<T> {
    value: T,
    prev: Option<Arc<Mutex<Node<T>>>>,
    next: Option<Arc<Mutex<Node<T>>>>,
}
```

#### 설계 이유

- **이중 연결 리스트**:
  - 양방향 순회 가능
  - 중간 노드 삭제 시 O(1) 성능
  - 시간 우선순위 보장

- **Arc<Mutex<>>**:
  - 스레드 안전성 보장
  - 참조 카운팅으로 메모리 관리
  - 동시 접근 제어

---

## 매칭 알고리즘

### 1. 지정가 주문 매칭

```rust
fn match_limit_order(&mut self, order: &mut Order, symbol: String) {
    match order.side {
        Side::Buy => {
            // 매수 주문: 매도 호가와 매칭
            while !order.is_filled() {
                if let Some((ask_price, _)) = order_book.get_best_ask() {
                    if order.price >= ask_price {
                        // 가격 조건 만족 시 매칭
                        self.match_at_price_level(order, order_book, ask_price, order.remaining_quantity);
                    } else {
                        break; // 가격 조건 불만족
                    }
                } else {
                    break; // 매칭할 매도 호가 없음
                }
            }
        },
        Side::Sell => {
            // 매도 주문: 매수 호가와 매칭
            // 유사한 로직...
        }
    }
}
```

#### 알고리즘 특징

- **가격 우선순위**: 베스트 호가부터 매칭
- **시간 우선순위**: 동일 가격 내에서 먼저 들어온 주문 우선
- **부분 체결**: 주문이 완전히 체결될 때까지 반복

### 2. 시장가 주문 매칭

```rust
fn match_market_order(&mut self, order: &mut Order, symbol: String) {
    match order.side {
        Side::Buy => {
            // 매수 주문: 최저 매도가부터 순차 매칭
            while !order.is_filled() {
                if let Some((price, _)) = order_book.get_best_ask() {
                    self.match_at_price_level(order, order_book, price, order.remaining_quantity);
                } else {
                    break; // 매칭할 호가 없음
                }
            }
        },
        Side::Sell => {
            // 매도 주문: 최고 매수가부터 순차 매칭
            // 유사한 로직...
        }
    }
}
```

#### 알고리즘 특징

- **즉시 체결**: 가격 조건 없이 즉시 매칭
- **최적 가격**: 베스트 호가부터 순차 매칭
- **슬리피지**: 큰 주문 시 여러 가격에 걸쳐 체결 가능

### 3. 가격 레벨 매칭

```rust
fn match_at_price_level(&mut self, order: &mut Order, order_book: &mut OrderBook, price: u64, match_qty: u64) {
    let opposite_side = match order.side {
        Side::Buy => Side::Sell,
        Side::Sell => Side::Buy,
    };
    
    let result = match opposite_side {
        Side::Buy => {
            if let Some(price_level) = order_book.bids.get_mut(&Reverse(price)) {
                price_level.match_partial(match_qty)
            } else { None }
        },
        Side::Sell => {
            if let Some(price_level) = order_book.asks.get_mut(&price) {
                price_level.match_partial(match_qty)
            } else { None }
        }
    };
    
    if let Some((maker_id, maker_order, actual_match_qty)) = result {
        // 체결 처리 및 보고서 생성
        order.fill(actual_match_qty);
        self.create_execution_reports(order, &maker_order, price, actual_match_qty);
    }
}
```

---

## 성능 최적화

### 1. 자료구조 선택 기준

| 연산 | 자료구조 | 시간복잡도 | 이유 |
|------|----------|------------|------|
| 주문 추가 | BTreeMap + DoublyLinkedList | O(log n) + O(1) | 가격별 정렬 + 시간순 삽입 |
| 베스트 호가 조회 | BTreeMap | O(1) | 첫 번째 요소 접근 |
| 주문 취소 | HashMap + DoublyLinkedList | O(1) + O(1) | 빠른 위치 찾기 + 삭제 |
| 가격 레벨 매칭 | DoublyLinkedList | O(1) | 첫 번째 요소 처리 |

### 2. 메모리 최적화

- **Arc<Mutex<>>**: 참조 카운팅으로 메모리 효율성
- **HashMap vs BTreeMap**: 상황에 따른 적절한 선택
- **캐시 지역성**: 자주 접근하는 데이터 구조 최적화

### 3. 알고리즘 최적화

- **불필요한 반복 제거**: 매칭 조건 미리 확인
- **조기 종료**: 체결 완료 시 즉시 루프 종료
- **배치 처리**: 여러 주문 동시 처리 고려

---

## 동시성 처리

### 1. 스레드 안전성

```rust
// Arc<Mutex<>>를 사용한 스레드 안전 노드
pub struct Node<T> {
    value: T,
    prev: Option<Arc<Mutex<Node<T>>>>,
    next: Option<Arc<Mutex<Node<T>>>>,
}
```

### 2. 채널 기반 통신

```rust
// 주문 수신 채널
order_rx: Receiver<Order>

// 체결 보고서 송신 채널  
exec_tx: Sender<ExecutionReport>

// WebSocket 브로드캐스트 채널
broadcast_tx: Option<tokio::sync::broadcast::Sender<WebSocketMessage>>
```

### 3. 락 최소화

- **세밀한 락**: 필요한 부분만 락
- **락 순서**: 데드락 방지를 위한 일관된 락 순서
- **비차단 연산**: 가능한 경우 비차단 연산 사용

---

## 테스트 전략

### 1. 단위 테스트

```rust
#[test]
fn test_limit_order_full_match() {
    // 완전 매칭 테스트
    let mut engine = MatchingEngine::new(symbols, exec_tx);
    
    // 매도 주문 추가 (메이커)
    let sell_order = create_test_order("order1", Side::Sell, OrderType::Limit, 10000, 100);
    engine.process_order(sell_order);
    
    // 매수 주문 추가 (테이커) - 완전 매칭
    let buy_order = create_test_order("order2", Side::Buy, OrderType::Limit, 10000, 100);
    engine.process_order(buy_order);
    
    // 체결 보고서 검증
    let taker_report = exec_rx.try_recv().unwrap();
    let maker_report = exec_rx.try_recv().unwrap();
    
    assert_eq!(taker_report.quantity, 100);
    assert_eq!(maker_report.quantity, 100);
}
```

### 2. 통합 테스트

- **시나리오 기반 테스트**: 실제 거래 시나리오 시뮬레이션
- **성능 테스트**: 대량 주문 처리 성능 측정
- **동시성 테스트**: 멀티스레드 환경에서의 안정성 검증

### 3. 테스트 커버리지

- **경계값 테스트**: 최소/최대 값 처리
- **예외 상황 테스트**: 잘못된 입력 처리
- **회귀 테스트**: 버그 수정 후 재발 방지

---

## 확장성 고려사항

### 1. 수평 확장

- **심볼별 독립성**: 새로운 심볼 추가 용이
- **마이크로서비스**: 매칭 엔진을 독립 서비스로 분리 가능
- **로드 밸런싱**: 여러 매칭 엔진 인스턴스 운영

### 2. 수직 확장

- **메모리 최적화**: 대용량 주문장 처리
- **CPU 최적화**: 멀티코어 활용
- **I/O 최적화**: 비동기 처리

### 3. 모니터링

- **성능 메트릭**: 처리량, 지연시간 측정
- **헬스 체크**: 시스템 상태 모니터링
- **알림 시스템**: 이상 상황 감지 및 알림

---

## 결론

xTrader의 매칭 엔진은 다음과 같은 특징을 가집니다:

1. **고성능**: O(log n) 또는 O(1) 시간복잡도로 효율적 처리
2. **정확성**: 가격-시간 우선순위 정확한 구현
3. **확장성**: 새로운 심볼 및 기능 추가 용이
4. **안정성**: 스레드 안전성 및 오류 처리 보장
5. **테스트 가능성**: 포괄적인 테스트 커버리지

이러한 설계를 통해 대용량 거래소에서도 안정적이고 효율적인 주문 매칭이 가능합니다.