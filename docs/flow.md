# xTrader 시스템 흐름도 (하이브리드 방식 - 실제 구현)

## 목차
1. [개요](#개요)
2. [하이브리드 주문 처리 흐름](#하이브리드-주문-처리-흐름)
3. [매칭 및 체결 흐름](#매칭-및-체결-흐름)
4. [체결 결과 처리 흐름](#체결-결과-처리-흐름)
5. [프론트엔드 데이터 수신 흐름](#프론트엔드-데이터-수신-흐름)
6. [Redis Streams 영속화 흐름](#redis-streams-영속화-흐름)
7. [RabbitMQ WebSocket 알림 흐름](#rabbitmq-websocket-알림-흐름)
8. [전체 시스템 통합 흐름](#전체-시스템-통합-흐름)
9. [장애 복구 흐름](#장애-복구-흐름)

---

## 개요

xTrader는 **하이브리드 방식의 초고성능 거래소 시스템**으로, 다음 특징을 가집니다:

- **즉시 응답**: API는 주문 접수 후 즉시 응답 (50ms 이내)
- **실시간 상태**: WebSocket으로 체결 결과와 상태 정보 실시간 전달
- **상태 조회**: REST API로 언제든지 주문 상태 확인 가능
- **3중 메시지 큐**: Redis Streams, Kafka, RabbitMQ를 활용한 고가용성

---

## 하이브리드 주문 처리 흐름

### 1단계: 주문 제출 및 즉시 응답

```mermaid
sequenceDiagram
    participant Client as 클라이언트 (HTS)
    participant API as REST API 서버<br/>(Port 7000)
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진

    Note over Client,Engine: 하이브리드 주문 처리 흐름

    Client->>API: POST /v1/order<br/>{symbol, side, price, quantity}

    API->>API: 주문 검증<br/>(수량, 가격, 심볼)

    alt 검증 실패
        API->>Client: 400 Bad Request<br/>{error, message}
    else 검증 성공
        API->>Sequencer: 주문 전송 (FIFO 큐)
        Note over Sequencer: FIFO 큐에 주문 추가<br/>공정한 순서 보장
        
        API->>Client: 200 OK<br/>{order_id, status: "ACCEPTED", message}
        Note over Client: 즉시 응답 (50ms 이내)<br/>체결 결과는 WebSocket으로 전달
    end
```

### 2단계: 비동기 매칭 처리

```mermaid
sequenceDiagram
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant OrderBook as 주문서<br/>(BTreeMap)

    Note over Sequencer,OrderBook: 비동기 매칭 처리

    Sequencer->>Engine: 순서대로 주문 전달

    Engine->>Engine: 매칭 시도

    alt 즉시 체결
        Engine->>Engine: 체결 보고서 생성 (Taker + Maker)
        Engine->>Sequencer: 체결 보고서 전달
        Note over Sequencer: 체결 결과를 WebSocket으로 브로드캐스트
    else 부분 체결
        Engine->>Engine: 체결 보고서 생성
        Engine->>OrderBook: 잔여 수량 주문서 등록
        Engine->>Sequencer: 체결 보고서 전달
    else 대기
        Engine->>OrderBook: 주문서에 등록
        Note over Engine: 매칭 대기 상태
    end
```

---

## 매칭 및 체결 흐름

### 매칭 엔진 내부 처리 흐름

```mermaid
sequenceDiagram
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant OrderBook as 주문서<br/>(BTreeMap)
    participant Memory as 메모리 상태

    Note over Sequencer,Memory: 매칭 엔진 내부 동작

    Sequencer->>Engine: 신규 주문 전달

    Engine->>OrderBook: 반대편 호가 조회

    alt 매칭 가능한 주문 존재
        OrderBook->>Engine: 최우선 호가 반환

        Engine->>Engine: 가격/시간 우선순위 매칭

        loop 호가창에서 매칭 가능한 모든 주문
            Engine->>Engine: 체결 수량 계산
            Engine->>Memory: 체결 내역 생성
            Engine->>OrderBook: 체결된 주문 제거/수정
        end

        alt 주문 완전 체결
            Engine->>Sequencer: ExecutionReport<br/>(remaining_quantity: 0)
        else 주문 부분 체결
            Engine->>OrderBook: 잔여 수량 주문서 등록
            Engine->>Sequencer: ExecutionReport<br/>(remaining_quantity > 0)
        end

    else 매칭 불가능
        Engine->>OrderBook: 주문서에 등록
        Note over Engine: 매칭 대기 상태
    end

    Note over Engine: 모든 처리는 메모리에서<br/>마이크로초 단위 완료
```

---

## 체결 결과 처리 흐름

### 하이브리드 방식: 상태 정보 포함 체결 처리

```mermaid
sequenceDiagram
    participant Engine as 매칭 엔진
    participant Sequencer as 주문 시퀀서
    participant Redis as Redis Streams
    participant Kafka as Apache Kafka
    participant RabbitMQ as RabbitMQ
    participant WS as WebSocket 서버
    participant Client as 클라이언트들

    Note over Engine,Client: 하이브리드 체결 결과 처리

    Engine->>Sequencer: ExecutionReport 생성

    Sequencer->>Sequencer: 체결 상태 결정<br/>Filled/PartiallyFilled/Pending

    par 3개 MQ 동시 발행
        Sequencer->>Redis: XADD executions *<br/>(체결 내역 영속화)
    and
        Sequencer->>Kafka: publish(market-data)<br/>(시장 데이터 발행)
    and
        Sequencer->>RabbitMQ: publish_websocket_message<br/>(WebSocket 알림)
    end

    RabbitMQ->>WS: WebSocketMessage::Execution<br/>{execution_report, order_status}

    WS->>Client: 실시간 체결 알림<br/>{type: "Execution", execution_report, order_status}

    Note over Client: 클라이언트는 즉시 정확한<br/>체결 상태를 확인 가능
```

### 체결 상태 결정 로직

```mermaid
flowchart TD
    A[ExecutionReport 수신] --> B{remaining_quantity 확인}
    
    B -->|remaining_quantity == 0| C[상태: "Filled"]
    B -->|remaining_quantity < quantity| D[상태: "PartiallyFilled"]
    B -->|remaining_quantity == quantity| E[상태: "Pending"]
    
    C --> F[WebSocket으로 상태 전달]
    D --> F
    E --> F
    
    F --> G[클라이언트 실시간 업데이트]
    
    style C fill:#51cf66
    style D fill:#ffd43b
    style E fill:#74c0fc
```

---

## 프론트엔드 데이터 수신 흐름

### 하이브리드 통신: REST API + WebSocket

```mermaid
sequenceDiagram
    participant HTS as 프론트엔드 HTS
    participant API as REST API 서버<br/>(Port 7000)
    participant WS as WebSocket 서버<br/>(Port 7001)
    participant Sequencer as 주문 시퀀서

    Note over HTS,Sequencer: 하이브리드 데이터 수신

    rect rgb(200, 255, 200)
        Note over HTS,API: 1단계: 주문 제출 (즉시 응답)
        HTS->>API: POST /v1/order
        API->>HTS: 200 OK {order_id, status: "ACCEPTED"}
        Note over HTS: 즉시 주문 접수 확인
    end

    rect rgb(255, 230, 200)
        Note over HTS,WS: 2단계: 실시간 체결 결과 (WebSocket)
        HTS->>WS: WebSocket 연결
        WS->>HTS: 연결 성공
        
        Sequencer->>WS: 체결 발생 시 브로드캐스트
        WS->>HTS: {type: "Execution", execution_report, order_status}
        Note over HTS: 실시간 체결 상태 확인
    end

    rect rgb(200, 220, 255)
        Note over HTS,API: 3단계: 상태 조회 (필요시)
        HTS->>API: GET /v1/order/{order_id}
        API->>HTS: {order_id, status, price, quantity, ...}
        Note over HTS: 언제든지 정확한 상태 확인
    end
```

### 클라이언트 구현 예시

```javascript
// 1. 주문 제출 (즉시 응답)
const orderResponse = await fetch('/v1/order', {
  method: 'POST',
  body: JSON.stringify({
    symbol: 'BTC-KRW',
    side: 'Buy',
    order_type: 'Limit',
    price: 50000000,
    quantity: 0.001
  })
});
const { order_id, status } = await orderResponse.json();
console.log(`주문 접수: ${order_id}, 상태: ${status}`);

// 2. WebSocket으로 실시간 체결 결과 수신
websocket.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.type === 'Execution' && message.execution_report.order_id === order_id) {
    console.log(`체결 완료: ${message.order_status}`);
    console.log(`체결 가격: ${message.execution_report.price}`);
    console.log(`체결 수량: ${message.execution_report.quantity}`);
  }
};

// 3. 필요시 상태 조회
const statusResponse = await fetch(`/v1/order/${order_id}`);
const orderStatus = await statusResponse.json();
console.log(`현재 상태: ${orderStatus.status}`);
```

---

## Redis Streams 영속화 흐름

### Consumer Group 기반 병렬 처리

```mermaid
sequenceDiagram
    participant Sequencer as 주문 시퀀서
    participant Redis as Redis Streams<br/>Stream: executions
    participant CG as Consumer Group<br/>execution_processors
    participant C1 as Consumer 1
    participant C2 as Consumer 2
    participant C3 as Consumer 3
    participant DB as PostgreSQL 데이터베이스

    Note over Sequencer,DB: Redis Streams 영속화 흐름

    Sequencer->>Redis: XADD executions *<br/>execution_data
    Note over Redis: Stream: executions<br/>고속 추가 (마이크로초)

    Redis->>CG: 새 메시지 알림

    par Consumer Group 자동 작업 분담
        CG->>C1: XREADGROUP<br/>메시지 배치 읽기 (100개)

        C1->>C1: 배치 트랜잭션 준비
        C1->>DB: BEGIN TRANSACTION

        loop 배치 내 모든 메시지
            C1->>DB: INSERT INTO executions
        end

        C1->>DB: COMMIT
        DB->>C1: 저장 완료

        C1->>Redis: XACK executions<br/>execution_processors<br/>message_ids

    and
        CG->>C2: XREADGROUP<br/>메시지 배치 읽기 (100개)
        C2->>DB: 배치 트랜잭션 저장
        C2->>Redis: XACK

    and
        CG->>C3: XREADGROUP<br/>메시지 배치 읽기 (100개)
        C3->>DB: 배치 트랜잭션 저장
        C3->>Redis: XACK
    end

    Note over Redis: ACK된 메시지는 안전하게 처리됨<br/>미처리 메시지는 자동 재할당
```

---

## RabbitMQ WebSocket 알림 흐름

### Topic Exchange 기반 라우팅

```mermaid
sequenceDiagram
    participant Sequencer as 주문 시퀀서
    participant RabbitMQ as RabbitMQ<br/>Exchange: market-notifications
    participant Q1 as Queue: execution.*
    participant Q2 as Queue: orderbook.*
    participant Q3 as Queue: ticker.*
    participant WS1 as WebSocket 서버 1
    participant WS2 as WebSocket 서버 2
    participant WS3 as WebSocket 서버 3
    participant Client as 클라이언트들

    Note over Sequencer,Client: RabbitMQ Topic Exchange 라우팅

    Sequencer->>RabbitMQ: Publish<br/>Routing Key: execution.BTC-KRW<br/>Message: WebSocketMessage::Execution

    RabbitMQ->>RabbitMQ: Topic Exchange 라우팅

    par Topic 패턴 매칭
        RabbitMQ->>Q1: execution.* 매칭<br/>→ execution.BTC-KRW
        Q1->>WS1: 메시지 전달
        WS1->>Client: WebSocket 푸시<br/>(BTC-KRW 체결 알림)

    and
        RabbitMQ->>Q2: orderbook.* 매칭 안됨<br/>→ 전달하지 않음

    and
        RabbitMQ->>Q3: ticker.* 매칭 안됨<br/>→ 전달하지 않음
    end

    Note over RabbitMQ: 라우팅 키 패턴:<br/>execution.*: 체결 알림<br/>orderbook.*: 호가창 업데이트<br/>ticker.*: 시세 알림
```

---

## 전체 시스템 통합 흐름

### 하이브리드 End-to-End 흐름

```mermaid
sequenceDiagram
    participant Client as 클라이언트 HTS
    participant API as REST API
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant Redis as Redis Streams
    participant Kafka as Apache Kafka
    participant RabbitMQ as RabbitMQ
    participant WS as WebSocket 서버
    participant External as 외부 시스템

    Note over Client,External: 하이브리드 전체 시스템 통합 흐름

    rect rgb(230, 230, 250)
        Note over Client,API: 1단계: 주문 제출 및 즉시 응답 (0-50ms)
        Client->>API: POST /v1/order
        API->>Sequencer: 주문 검증 후 전달
        API->>Client: 200 OK {order_id, status: "ACCEPTED"}
        Note over Client: 즉시 응답으로 높은 사용자 경험
    end

    rect rgb(255, 240, 200)
        Note over Sequencer,Engine: 2단계: 비동기 매칭 처리 (50-200ms)
        Sequencer->>Engine: FIFO 순서로 매칭 요청
        Engine->>Engine: 메모리 매칭 엔진 처리
        Engine->>Engine: ExecutionReport 생성
    end

    rect rgb(200, 255, 200)
        Note over Engine,Client: 3단계: 실시간 체결 결과 전달 (200-250ms)
        Engine->>Sequencer: 체결 보고서 반환
        Sequencer->>Sequencer: 체결 상태 결정 (Filled/PartiallyFilled/Pending)
        Sequencer->>WS: WebSocketMessage::Execution {execution_report, order_status}
        WS->>Client: 실시간 체결 알림
    end

    rect rgb(255, 200, 200)
        Note over Redis,RabbitMQ: 4단계: MQ 병렬 발행 (250-350ms)

        par MQ 3가지 동시 발행
            Sequencer->>Redis: XADD executions *<br/>(체결 내역 영속화)
        and
            Sequencer->>Kafka: publish(market-data)<br/>(시장 데이터 발행)
        and
            Sequencer->>RabbitMQ: publish(execution.symbol)<br/>(WebSocket 알림)
        end
    end

    rect rgb(200, 220, 255)
        Note over External: 5단계: 비동기 후처리 (350ms 이후)

        par 병렬 소비 및 처리
            Redis->>DB: Consumer Group 배치 저장<br/>(3개 Consumer 병렬)

        and
            Kafka->>External: 외부 시스템 Consumer<br/>가격 동기화, 규제 보고

        and
            RabbitMQ->>WS: WebSocket 알림 전달
            WS->>Client: 실시간 체결 알림 푸시
        end
    end

    Note over Client,External: 총 처리 시간: ~350ms<br/>클라이언트 응답: 50ms<br/>체결 알림: 250ms<br/>완전한 영속화: 1000ms
```

---

## 장애 복구 흐름

### MQ 장애 시 복구 시나리오

```mermaid
sequenceDiagram
    participant Engine as 매칭 엔진
    participant Redis as Redis Streams
    participant Kafka as Apache Kafka
    participant RabbitMQ as RabbitMQ
    participant LocalQueue as 로컬 큐<br/>(메모리)
    participant Monitor as 모니터링 시스템

    Note over Engine,Monitor: MQ 장애 감지 및 복구

    Engine->>Redis: 체결 내역 발행 시도

    alt Redis 장애 발생
        Redis-->>Engine: Connection Timeout
        Engine->>Monitor: Redis 장애 감지
        Engine->>LocalQueue: 로컬 큐에 임시 저장

        Monitor->>Redis: Health Check (5초마다)

        Redis-->>Monitor: ❌ 응답 없음
        Redis-->>Monitor: ❌ 응답 없음
        Redis-->>Monitor: ✅ 복구 완료!

        Monitor->>Engine: Redis 복구 알림

        Engine->>LocalQueue: 임시 저장 데이터 조회

        loop 로컬 큐의 모든 메시지
            Engine->>Redis: 재발행
            Redis->>Engine: 발행 성공
        end

        Engine->>LocalQueue: 로컬 큐 비우기

        Note over Engine: 데이터 손실 없이<br/>완전 복구 완료
    end

    par Kafka 장애 시 동일한 메커니즘
        Engine->>Kafka: 발행 시도
        Kafka-->>Engine: Timeout
        Engine->>LocalQueue: 임시 저장
        Note over LocalQueue: Kafka 복구 시<br/>자동 재발행

    and RabbitMQ 장애 시 동일한 메커니즘
        Engine->>RabbitMQ: 발행 시도
        RabbitMQ-->>Engine: Timeout
        Engine->>LocalQueue: 임시 저장
        Note over LocalQueue: RabbitMQ 복구 시<br/>자동 재발행
    end

    Note over Engine,Monitor: 각 MQ의 장애가<br/>다른 MQ로 전파되지 않음<br/>(장애 격리)
```

---

## 성능 목표 및 메트릭

### 하이브리드 방식 처리 시간 목표

| 단계 | 목표 시간 | 설명 |
|------|-----------|------|
| **주문 검증** | 0-10ms | REST API 파라미터 검증 |
| **즉시 응답** | 10-50ms | 클라이언트 응답 반환 |
| **FIFO 큐 추가** | 50-100ms | 시퀀서 큐 삽입 |
| **매칭 처리** | 100-200ms | 메모리 매칭 엔진 |
| **체결 알림** | 200-250ms | WebSocket 실시간 전달 |
| **MQ 발행** | 250-350ms | 3개 MQ 병렬 발행 |
| **DB 영속화** | 350-1000ms | 배치 저장 |
| **외부 연동** | 1000-3000ms | Kafka Consumer 처리 |

### 처리량 목표

| 컴포넌트 | 목표 TPS | 지연시간 |
|----------|----------|----------|
| **매칭 엔진** | 100,000+ TPS | 0.1ms |
| **Redis Streams** | 50,000 TPS | 0.2ms |
| **Apache Kafka** | 500,000 TPS | 0.05ms |
| **RabbitMQ** | 30,000 TPS | 0.5ms |
| **REST API** | 10,000 TPS | 50ms |
| **WebSocket** | 5,000 연결/서버 | 1ms |

---

## 하이브리드 방식의 장점

### 1. **최고의 사용자 경험**
- **즉시 응답**: 50ms 이내 주문 접수 확인
- **실시간 알림**: WebSocket으로 체결 결과 실시간 전달
- **상태 조회**: 언제든지 정확한 주문 상태 확인

### 2. **높은 성능**
- **높은 처리량**: API 서버 블로킹 없음
- **낮은 지연시간**: 즉시 응답으로 사용자 대기 시간 최소화
- **확장성**: 각 컴포넌트 독립적 확장 가능

### 3. **안정성**
- **장애 격리**: 한 컴포넌트 장애가 다른 컴포넌트에 영향 없음
- **데이터 보존**: Redis Streams로 체결 내역 완전 보존
- **복구 능력**: 자동 장애 감지 및 복구

### 4. **개발자 친화적**
- **단순한 API**: RESTful API로 쉬운 통합
- **명확한 상태**: 체결 상태가 명확하게 구분됨
- **유연한 구현**: 클라이언트가 필요한 방식으로 구현 가능

---

## 결론

xTrader 하이브리드 시스템은 **즉시 응답 + 실시간 알림 + 상태 조회**의 장점을 모두 가진 최적의 거래소 아키텍처입니다:

1. ✅ **초고속 응답**: 50ms 이내 클라이언트 응답
2. ✅ **실시간 알림**: WebSocket으로 체결 결과 즉시 전달
3. ✅ **정확한 상태**: Filled/PartiallyFilled/Pending 상태 명확 구분
4. ✅ **완전한 영속화**: Redis Streams를 통한 데이터 손실 방지
5. ✅ **외부 시스템 연동**: Kafka를 통한 무제한 확장
6. ✅ **장애 격리**: 각 MQ의 독립적인 장애 복구

**최종 목표 달성**:
- **처리량**: 100,000+ TPS
- **지연시간**: 50ms 이하 (API 응답)
- **체결 알림**: 250ms 이하
- **가용성**: 99.99% 이상
- **확장성**: 무제한 수평 확장