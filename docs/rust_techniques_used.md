# Rust 기술 활용 가이드 - xTrader 매칭 엔진

## 개요

이 문서는 xTrader 매칭 엔진에서 사용된 Rust의 핵심 기술들을 설명합니다. 실제 코드에서 발췌한 예제를 통해 각 기술의 활용법과 장점을 살펴봅니다.

## 목차

1. [패턴 매칭 (Pattern Matching)](#패턴-매칭-pattern-matching)
2. [스마트 포인터 (Smart Pointers)](#스마트-포인터-smart-pointers)
3. [동기화 및 동시성 (Concurrency)](#동기화-및-동시성-concurrency)
4. [소유권 및 생명주기 (Ownership & Lifetimes)](#소유권-및-생명주기-ownership--lifetimes)
5. [에러 처리 (Error Handling)](#에러-처리-error-handling)
6. [제네릭 및 트레이트 (Generics & Traits)](#제네릭-및-트레이트-generics--traits)

---

## 패턴 매칭 (Pattern Matching)

### 1. 열거형 매칭

#### 매수/매도 방향 처리

```rust
// src/matching_engine/model.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,   // 매수 주문
    Sell,  // 매도 주문
}

// 매칭 엔진에서의 활용
match order.side {
    Side::Buy => {
        // 매수 주문 처리 로직
        debug!("매수 주문 처리: {}", order.id);
        self.match_market_order(&mut order, symbol.clone());
    },
    Side::Sell => {
        // 매도 주문 처리 로직
        debug!("매도 주문 처리: {}", order.id);
        self.match_market_order(&mut order, symbol.clone());
    }
}
```

#### 주문 타입별 처리

```rust
// src/matching_engine/model.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,  // 시장가 주문
    Limit,   // 지정가 주문
}

// 주문 타입에 따른 처리 분기
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
        }
    }
}
```

### 2. Option과 Result 패턴 매칭

#### 안전한 값 접근

```rust
// src/matching_engine/engine.rs
// 베스트 호가 조회 시 안전한 패턴 매칭
if let Some((price, _price_level)) = order_book.get_best_ask() {
    // 매칭 가능한 매도 호가가 있는 경우
    let match_qty = order.remaining_quantity;
    self.match_at_price_level(order, order_book, price, match_qty);
} else {
    // 매칭할 매도 호가가 없는 경우
    debug!("매칭할 매도 주문 없음, 시장가 매수 주문 미체결: {}", order.id);
    break;
}
```

#### 중첩된 Option 처리

```rust
// src/matching_engine/order_book.rs
// 가격 레벨에서 주문 매칭
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
```

### 3. 구조체 분해 (Destructuring)

```rust
// src/matching_engine/engine.rs
// 매칭 결과 분해
if let Some((cloned_maker_id, cloned_maker, actual_match_qty)) = result {
    // 체결 처리
    order.fill(actual_match_qty);
    
    // 메이커 주문 저장소 업데이트
    if cloned_maker.is_filled() {
        order_book.orders.remove(&cloned_maker_id);
        self.order_store.remove(&cloned_maker_id);
    } else {
        self.order_store.insert(cloned_maker_id.clone(), cloned_maker.clone());
    }
}
```

---

## 스마트 포인터 (Smart Pointers)

### 1. Arc (Atomically Reference Counted)

#### 스레드 안전한 참조 카운팅

```rust
// src/util/linked_list.rs
use std::sync::Arc;

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

#### Arc의 장점

- **참조 카운팅**: 자동 메모리 관리
- **스레드 안전**: 여러 스레드에서 안전하게 공유
- **불변성**: 데이터 변경 시 복사본 생성
- **순환 참조 방지**: Weak<T>와 함께 사용 가능

### 2. Mutex (Mutual Exclusion)

#### 동시 접근 제어

```rust
// src/matching_engine/order_book.rs
pub struct PriceLevel {
    orders: DoublyLinkedList<Order>,
    order_map: HashMap<String, Arc<Mutex<Node<Order>>>>,
    total_volume: u64,
}

impl PriceLevel {
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
}
```

#### Mutex 사용 시 주의사항

```rust
// 안전한 락 사용
if let Ok(node_guard) = node.lock() {
    let order = node_guard.value.clone();
    // 락이 자동으로 해제됨
} else {
    // 락 획득 실패 처리
    return None;
}
```

### 3. Box (힙 할당)

#### 트레이트 객체 저장

```rust
// 예시: 트레이트 객체를 Box로 저장
trait OrderProcessor {
    fn process(&self, order: &Order) -> Result<(), String>;
}

struct MatchingEngine {
    processors: Vec<Box<dyn OrderProcessor>>,
}
```

---

## 동기화 및 동시성 (Concurrency)

### 1. 채널 (Channels)

#### 주문 수신 채널

```rust
// src/matching_engine/engine.rs
use std::sync::mpsc::{Receiver, Sender};

pub struct MatchingEngine {
    exec_tx: Sender<ExecutionReport>,  // 체결 보고서 송신 채널
    // ...
}

impl MatchingEngine {
    pub fn run(&mut self, order_rx: Receiver<Order>) {
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
}
```

#### 비동기 브로드캐스트 채널

```rust
// src/matching_engine/engine.rs
use tokio::sync::broadcast;

pub struct MatchingEngine {
    broadcast_tx: Option<tokio::sync::broadcast::Sender<WebSocketMessage>>,
}

impl MatchingEngine {
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
```

### 2. 스레드 안전성 보장

#### Arc<Mutex<>> 조합

```rust
// src/matching_engine/order_book.rs
pub struct PriceLevel {
    orders: DoublyLinkedList<Order>,
    order_map: HashMap<String, Arc<Mutex<Node<Order>>>>,
    total_volume: u64,
}

impl PriceLevel {
    pub fn match_partial(&mut self, match_quantity: u64) -> Option<(String, Order, u64)> {
        if let Some(node) = self.orders.peek_front() {
            let mut cloned_maker = if let Ok(node_guard) = node.lock() {
                node_guard.value.clone()
            } else {
                return None;
            };
            
            let maker_order_id = cloned_maker.id.clone();
            let maker_current_quantity = cloned_maker.remaining_quantity;
            
            // 실제 체결 수량 계산
            let actual_match = std::cmp::min(match_quantity, maker_current_quantity);
            
            // 총 수량 업데이트
            self.total_volume = self.total_volume.saturating_sub(actual_match);
            
            // 원본 주문 객체에 체결 처리
            cloned_maker.fill(actual_match);
            
            if actual_match >= maker_current_quantity {
                // 완전 체결 - 주문 제거
                self.order_map.remove(&maker_order_id);
                
                if let Some(front_node) = self.orders.peek_front() {
                    self.orders.remove(front_node.clone());
                }
            } else {
                // 부분 체결 - 노드의 주문 객체 업데이트
                if let Ok(mut node_guard) = node.lock() {
                    node_guard.value.remaining_quantity = cloned_maker.remaining_quantity;
                }
            }
            
            Some((maker_order_id, cloned_maker, actual_match))
        } else {
            None
        }
    }
}
```

### 3. 동시성 패턴

#### Producer-Consumer 패턴

```rust
// Producer: 주문 생성
pub fn submit_order(&self, order_data: OrderData) -> Result<String, String> {
    let order = Order::new(
        Uuid::new_v4().to_string(),
        order_data.symbol,
        order_data.side,
        order_data.order_type,
        order_data.price,
        order_data.quantity,
        order_data.client_id,
    );
    
    // 채널을 통해 매칭 엔진으로 전송
    self.order_tx.send(order.clone())
        .map_err(|_| "주문 전송 실패".to_string())?;
    
    Ok(order.id)
}

// Consumer: 매칭 엔진에서 주문 처리
pub fn run(&mut self, order_rx: Receiver<Order>) {
    while let Ok(order) = order_rx.recv() {
        self.process_order(order);
    }
}
```

---

## 소유권 및 생명주기 (Ownership & Lifetimes)

### 1. 소유권 전달

#### 값 이동 (Move)

```rust
// src/matching_engine/engine.rs
fn process_order(&mut self, mut order: Order) {
    let symbol = order.symbol.clone();  // 소유권 유지
    
    // 주문 저장 (소유권 전달)
    self.order_store.insert(order.id.clone(), order.clone());
    
    // 주문 타입에 따라 처리
    match order.order_type {
        OrderType::Market => {
            self.match_market_order(&mut order, symbol.clone());
        },
        OrderType::Limit => {
            self.match_limit_order(&mut order, symbol.clone());
            
            if !order.is_filled() {
                let order_book = self.order_books.get_mut(&symbol).unwrap();
                order_book.add_order(order);  // 소유권 전달
            }
        }
    }
}
```

### 2. 참조 및 생명주기

#### 가변 참조

```rust
// src/matching_engine/engine.rs
fn match_at_price_level(
    &mut self,
    order: &mut Order,           // 가변 참조
    order_book: &mut OrderBook,  // 가변 참조
    price: u64,
    match_qty: u64
) {
    // order와 order_book을 동시에 가변 참조로 사용
    order.fill(actual_match_qty);
    
    if cloned_maker.is_filled() {
        order_book.orders.remove(&cloned_maker_id);
        self.order_store.remove(&cloned_maker_id);
    }
}
```

#### 생명주기 명시

```rust
// 예시: 생명주기가 필요한 경우
struct OrderProcessor<'a> {
    order_book: &'a mut OrderBook,
    symbol: &'a str,
}

impl<'a> OrderProcessor<'a> {
    fn process_order(&mut self, order: Order) {
        // 생명주기가 명시적으로 관리됨
        self.order_book.add_order(order);
    }
}
```

---

## 에러 처리 (Error Handling)

### 1. Result 타입 활용

#### 안전한 에러 처리

```rust
// src/matching_engine/engine.rs
// 체결 보고서 전송
if let Err(e) = self.exec_tx.send(taker_exec) {
    error!("테이커 체결 보고서 전송 실패: {}", e);
}

// WebSocket 브로드캐스트
if let Err(e) = broadcast_tx.send(message) {
    warn!("호가창 업데이트 브로드캐스트 실패: {}", e);
}
```

### 2. Option과 Result 조합

```rust
// src/matching_engine/order_book.rs
pub fn remove_order(&mut self, order_id: &str) -> Option<Order> {
    if let Some(node) = self.order_map.remove(order_id) {
        // 락 획득 시도
        let quantity = if let Ok(node_guard) = node.lock() {
            node_guard.value.remaining_quantity
        } else {
            0  // 락 획득 실패 시 기본값
        };
        
        self.total_volume = self.total_volume.saturating_sub(quantity);
        self.orders.remove(node.clone());
        
        // 주문 객체 추출
        if let Ok(node_guard) = node.lock() {
            Some(node_guard.value.clone())
        } else {
            None
        }
    } else {
        None
    }
}
```

### 3. 에러 전파

```rust
// 예시: 에러 전파 패턴
fn process_order_chain(&mut self, order: Order) -> Result<ExecutionReport, String> {
    let symbol = order.symbol.clone();
    
    // 주문장 조회
    let order_book = self.order_books.get_mut(&symbol)
        .ok_or_else(|| format!("지원하지 않는 심볼: {}", symbol))?;
    
    // 매칭 처리
    let execution = self.match_order(order, order_book)
        .map_err(|e| format!("매칭 실패: {}", e))?;
    
    Ok(execution)
}
```

---

## 제네릭 및 트레이트 (Generics & Traits)

### 1. 제네릭 구조체

#### 타입 안전성과 재사용성

```rust
// src/util/linked_list.rs
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

impl<T> DoublyLinkedList<T> {
    pub fn new() -> Self {
        DoublyLinkedList {
            head: None,
            tail: None,
            len: 0,
        }
    }
    
    pub fn push_back(&mut self, value: T) -> Arc<Mutex<Node<T>>> {
        let new_node = Arc::new(Mutex::new(Node {
            value,
            prev: None,
            next: None,
        }));
        
        // 리스트에 추가하는 로직...
        new_node
    }
}
```

### 2. 트레이트 구현

#### 직렬화 트레이트

```rust
// src/matching_engine/model.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: u64,
    pub quantity: u64,
    pub remaining_quantity: u64,
    pub client_id: String,
    pub timestamp: u64,
    pub is_cancel: bool,
    pub target_order_id: Option<String>,
}
```

### 3. 트레이트 바운드

#### Clone 트레이트 바운드

```rust
// src/matching_engine/order_book.rs
impl<T: Clone> PriceLevel {
    pub fn peek_front(&self) -> Option<(String, T, u64)> {
        if let Some(node) = self.orders.peek_front() {
            if let Ok(node_guard) = node.lock() {
                let value = node_guard.value.clone();  // Clone 트레이트 필요
                // ...
            }
        }
    }
}
```

---

## 성능 최적화 기법

### 1. 메모리 효율성

#### 참조 카운팅 최적화

```rust
// Arc<Mutex<>> 사용 시 메모리 효율성 고려
pub struct PriceLevel {
    orders: DoublyLinkedList<Order>,
    order_map: HashMap<String, Arc<Mutex<Node<Order>>>>,  // 참조 카운팅
    total_volume: u64,  // 캐시된 값으로 빠른 조회
}
```

### 2. 지연 평가 (Lazy Evaluation)

#### Option 체이닝

```rust
// 지연 평가를 통한 효율적 처리
let result = order_book.bids.get_mut(&Reverse(price))
    .and_then(|price_level| price_level.match_partial(match_qty));
```

### 3. 인라인 최적화

```rust
#[inline]
pub fn is_filled(&self) -> bool {
    self.remaining_quantity == 0
}

#[inline]
pub fn fill(&mut self, quantity: u64) {
    self.remaining_quantity = self.remaining_quantity.saturating_sub(quantity);
}
```

---

## 결론

xTrader 매칭 엔진에서 사용된 Rust 기술들은 다음과 같은 장점을 제공합니다:

### 🎯 **패턴 매칭**
- **안전성**: 컴파일 타임에 모든 경우 처리 보장
- **가독성**: 명확한 조건 분기 표현
- **성능**: 효율적인 분기 처리

### 🔒 **스마트 포인터**
- **메모리 안전성**: 자동 메모리 관리
- **스레드 안전성**: 동시성 환경에서 안전한 공유
- **성능**: 참조 카운팅으로 효율적 메모리 사용

### ⚡ **동시성**
- **채널**: 안전한 스레드 간 통신
- **뮤텍스**: 동시 접근 제어
- **비동기**: 논블로킹 처리

### 🛡️ **소유권 시스템**
- **메모리 안전성**: 컴파일 타임에 메모리 오류 방지
- **성능**: 런타임 오버헤드 없는 안전성
- **명확성**: 소유권 전달이 명시적

이러한 Rust 기술들을 통해 고성능, 안전한 매칭 엔진을 구현할 수 있었습니다.