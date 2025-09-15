# xTrader 시스템 시퀀스 다이어그램 (WebSocket 기반)

이 문서는 xTrader 거래소 시스템의 주요 시나리오에 대한 상세한 시퀀스 다이어그램을 제공합니다. 모든 실시간 데이터는 WebSocket을 통해 푸시됩니다.

## 1. 주문 제출 및 체결 시퀀스 (WebSocket 푸시)

```mermaid
sequenceDiagram
    participant Client as 클라이언트
    participant Frontend as React 프론트엔드
    participant REST as Axum REST API
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant OrderBook as 주문장
    participant MDP as 시장 데이터 발행자
    participant WS as WebSocket 서버

    Client->>Frontend: 주문 제출 (매수/매도)
    Frontend->>REST: POST /v1/order
    Note over REST: 주문 유효성 검사<br/>- 수량 > 0<br/>- 지정가 주문 시 가격 필수<br/>- 지원 심볼 확인

    REST->>REST: 주문 ID 생성 및 비동기 큐 전송
    REST-->>Frontend: 즉시 "ACCEPTED" 응답 (논블로킹)
    Frontend->>Client: 주문 접수 확인

    Note over Sequencer: 비동기 처리 시작
    REST->>Sequencer: 주문 채널 수신
    Sequencer->>Sequencer: 주문 순서 보장 (FIFO)
    Sequencer->>Engine: 순서 보장된 주문 전달
    Engine->>OrderBook: 주문 매칭 시도

    alt 매칭 성공 (완전 체결)
        OrderBook-->>Engine: 체결 결과 반환
        Engine->>Engine: 체결 보고서 생성 (Taker + Maker)
        Engine->>Sequencer: 체결 보고서 전달
        Sequencer->>WS: 브로드캐스트 채널로 체결 알림
        WS->>Frontend: WebSocket 체결 완료 알림
        Frontend->>Client: 실시간 체결 업데이트
        
        Sequencer->>MDP: 체결 데이터 전송
        MDP->>MDP: 시장 데이터 업데이트<br/>- 호가창 업데이트<br/>- 시장 통계 계산<br/>- 봉차트 업데이트
        MDP->>WS: 시장 데이터 브로드캐스트
        WS->>Frontend: 호가창/통계/차트 업데이트
        Frontend->>Client: 실시간 시장 데이터 업데이트

    else 매칭 성공 (부분 체결)
        OrderBook-->>Engine: 부분 체결 결과
        Engine->>Engine: 체결 보고서 생성
        Engine->>Sequencer: 체결 보고서 전달
        Sequencer->>WS: 브로드캐스트 채널로 부분 체결 알림
        Engine->>OrderBook: 남은 수량 주문장 추가
        WS->>Frontend: WebSocket 부분 체결 알림
        Frontend->>Client: 부분 체결 업데이트
        
        Sequencer->>MDP: 체결 데이터 전송
        MDP->>WS: 시장 데이터 브로드캐스트
        WS->>Frontend: 호가창/통계 업데이트
        Frontend->>Client: 실시간 시장 데이터 업데이트

    else 매칭 실패 (지정가 주문)
        OrderBook->>OrderBook: 주문장에 주문 추가
        Sequencer->>MDP: 주문장 변경 알림
        MDP->>WS: 호가창 업데이트 브로드캐스트
        WS->>Frontend: WebSocket 주문장 변경 알림
        Frontend->>Client: 주문장 실시간 업데이트
    end
```

## 2. WebSocket 실시간 연결 및 데이터 스트림

```mermaid
sequenceDiagram
    participant Client as 클라이언트
    participant Frontend as React 프론트엔드
    participant WS as WebSocket 서버
    participant Sequencer as 주문 시퀀서
    participant MDP as 시장 데이터 발행자
    participant Engine as 매칭 엔진

    Client->>Frontend: 페이지 로드
    Frontend->>WS: WebSocket 연결 요청 (ws://127.0.0.1:7001/ws)
    WS-->>Frontend: 연결 성공
    
    Note over Frontend: 초기 데이터 로드 (REST API)
    Frontend->>Frontend: REST API로 초기 데이터 조회<br/>- 주문서<br/>- 시장 통계<br/>- 봉차트<br/>- 체결 내역
    
    loop 실시간 데이터 스트림
        Engine->>Engine: 체결 이벤트 발생
        Engine->>Sequencer: 체결 보고서 전달
        Sequencer->>WS: 체결 알림 브로드캐스트
        WS->>Frontend: 체결 알림 전송 (JSON)
        Frontend->>Client: 실시간 체결 업데이트
        
        Sequencer->>MDP: 체결 데이터 전송
        MDP->>MDP: 시장 데이터 업데이트<br/>- 호가창 스냅샷<br/>- 시장 통계 계산<br/>- 봉차트 업데이트<br/>- 체결 내역 저장
        
        MDP->>WS: 시장 데이터 브로드캐스트
        WS->>Frontend: 시장 데이터 업데이트 전송<br/>- OrderBookUpdate<br/>- MarketStatistics<br/>- CandlestickUpdate
        Frontend->>Client: 실시간 시장 데이터 업데이트
    end

    Client->>Frontend: 페이지 종료
    Frontend->>WS: 연결 종료
    WS-->>Frontend: 연결 해제 확인
```

## 3. 주문 취소 시퀀스 (WebSocket 알림)

```mermaid
sequenceDiagram
    participant Client as 클라이언트
    participant Frontend as React 프론트엔드
    participant REST as Axum REST API
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant OrderBook as 주문장
    participant MDP as 시장 데이터 발행자
    participant WS as WebSocket 서버

    Client->>Frontend: 주문 취소 요청
    Frontend->>REST: POST /v1/order/cancel
    Note over REST: 취소 주문 유효성 검사<br/>- 주문 ID 존재 확인<br/>- 주문 상태 확인
    
    REST->>Sequencer: 취소 주문 처리 요청
    Sequencer->>Sequencer: 취소 주문 순서 보장 (FIFO)
    Sequencer->>Engine: 순서 보장된 취소 주문 전달
    Engine->>OrderBook: 주문장에서 주문 제거
    
    alt 주문 존재 및 취소 가능
        OrderBook-->>Engine: 취소 성공
        Engine->>Sequencer: 취소 완료 알림
        Sequencer->>WS: 취소 알림 브로드캐스트
        Sequencer->>MDP: 주문장 변경 알림
        MDP->>WS: 호가창 업데이트 브로드캐스트
        
        Sequencer-->>REST: 취소 확인
        REST-->>Frontend: 취소 성공 응답
        Frontend->>Client: 취소 완료 알림
        
        WS->>Frontend: WebSocket 취소 알림
        WS->>Frontend: 호가창 업데이트 알림
        Frontend->>Client: 실시간 주문장 업데이트
        
    else 주문 없음 또는 이미 체결됨
        Engine-->>Sequencer: 주문 없음 오류
        Sequencer-->>REST: 주문 없음 오류
        REST-->>Frontend: 오류 응답 (404)
        Frontend->>Client: 오류 메시지 표시
    end
```

## 4. 시장 데이터 실시간 업데이트 시퀀스 (WebSocket 푸시)

```mermaid
sequenceDiagram
    participant Engine as 매칭 엔진
    participant Sequencer as 주문 시퀀서
    participant MDP as 시장 데이터 발행자
    participant WS as WebSocket 서버
    participant Frontend as React 프론트엔드
    participant Client as 클라이언트

    Engine->>Engine: 체결 이벤트 발생
    Engine->>Sequencer: 체결 보고서 전달
    
    Sequencer->>MDP: 체결 데이터 전송
    MDP->>MDP: 체결 내역 저장소 업데이트
    MDP->>MDP: 봉차트 데이터 업데이트<br/>(1m, 5m, 15m, 30m, 1h, 4h, 1d)
    MDP->>MDP: 시장 통계 계산<br/>(24시간 OHLCV, 가격 변화율)
    MDP->>MDP: 호가창 스냅샷 업데이트
    
    Note over MDP: 원형 버퍼를 사용한<br/>효율적인 데이터 관리
    
    MDP->>WS: 시장 데이터 브로드캐스트<br/>- OrderBookUpdate<br/>- MarketStatistics<br/>- CandlestickUpdate
    WS->>Frontend: 실시간 시장 데이터 전송
    Frontend->>Client: UI 실시간 업데이트<br/>- 호가창<br/>- 시장 통계<br/>- 차트<br/>- 체결 내역
```

## 5. 시스템 초기화 시퀀스 (WebSocket 서버 포함)

```mermaid
sequenceDiagram
    participant Server as 서버 시작
    participant Engine as 매칭 엔진
    participant OrderBook as 주문장
    participant Sequencer as 주문 시퀀서
    participant MDP as 시장 데이터 발행자
    participant REST as REST API 서버
    participant WS as WebSocket 서버

    Server->>Engine: 매칭 엔진 초기화
    Engine->>OrderBook: 지원 심볼별 주문장 생성<br/>(BTC-KRW, ETH-KRW, AAPL)
    OrderBook-->>Engine: 초기화 완료
    
    Server->>MDP: 시장 데이터 발행자 초기화
    MDP->>MDP: 체결 내역 저장소 생성<br/>봉차트 데이터 저장소 생성<br/>시장 통계 저장소 생성
    MDP-->>Server: 초기화 완료
    
    Server->>Sequencer: 주문 시퀀서 초기화
    Sequencer->>Sequencer: 채널 생성<br/>- 주문 입력 채널<br/>- 체결 출력 채널<br/>- 브로드캐스트 채널
    Sequencer-->>Server: 초기화 완료
    
    Server->>REST: REST API 서버 시작 (포트 7000)
    REST->>Engine: 매칭 엔진 연결
    REST->>MDP: MDP 연결
    Engine-->>REST: 연결 확인
    MDP-->>REST: 연결 확인
    
    Server->>WS: WebSocket 서버 시작 (포트 7001)
    WS->>Sequencer: 브로드캐스트 채널 구독
    WS->>MDP: 시장 데이터 브로드캐스트 채널 구독
    Sequencer-->>WS: 연결 확인
    MDP-->>WS: 연결 확인
    
    Server->>Server: 모든 서비스 준비 완료
    Note over Server: 클라이언트 연결 대기<br/>REST API: http://127.0.0.1:7000<br/>WebSocket: ws://127.0.0.1:7001
```

## 6. WebSocket 메시지 타입별 처리 시퀀스

```mermaid
sequenceDiagram
    participant Frontend as React 프론트엔드
    participant WS as WebSocket 서버
    participant Sequencer as 주문 시퀀서
    participant MDP as 시장 데이터 발행자

    Frontend->>WS: WebSocket 연결
    WS-->>Frontend: 연결 성공
    
    loop 실시간 메시지 처리
        Sequencer->>WS: Execution 메시지 브로드캐스트
        WS->>Frontend: {"type": "Execution", "exec_id": "...", "order_id": "...", "symbol": "BTC-KRW", "side": "Buy", "price": 50000000, "quantity": 0.5, "fee": 0.05, "transaction_time": "2023-04-30T12:35:10.123Z"}
        Frontend->>Frontend: 체결 내역 업데이트
        
        MDP->>WS: OrderBookUpdate 메시지 브로드캐스트
        WS->>Frontend: {"type": "OrderBookUpdate", "symbol": "BTC-KRW", "bids": [[50000000, 100]], "asks": [[50010000, 50]], "timestamp": 1682854510123}
        Frontend->>Frontend: 호가창 업데이트
        
        MDP->>WS: MarketStatistics 메시지 브로드캐스트
        WS->>Frontend: {"type": "MarketStatistics", "symbol": "BTC-KRW", "timestamp": 1682854510123, "last_price": 50000000, "price_change_24h": 2.5, "volume_24h": 1000000, "high_price_24h": 51000000, "low_price_24h": 49000000}
        Frontend->>Frontend: 시장 통계 업데이트
        
        MDP->>WS: CandlestickUpdate 메시지 브로드캐스트
        WS->>Frontend: {"type": "CandlestickUpdate", "symbol": "BTC-KRW", "interval": "1m", "candle": {"open": 49900000, "high": 50100000, "low": 49800000, "close": 50000000, "volume": 1000, "timestamp": 1682854500000}}
        Frontend->>Frontend: 차트 업데이트
    end
```

## 7. 대량 주문 처리 시퀀스 (WebSocket 실시간 알림)

```mermaid
sequenceDiagram
    participant Client as 클라이언트
    participant Frontend as React 프론트엔드
    participant REST as Axum REST API
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant OrderBook as 주문장
    participant MDP as 시장 데이터 발행자
    participant WS as WebSocket 서버

    Client->>Frontend: 대량 주문 제출 (예: 1000주)
    Frontend->>REST: POST /v1/order (대량 수량)
    REST->>Sequencer: 주문 처리 요청
    
    loop 여러 가격 레벨 매칭
        Sequencer->>Sequencer: 주문 순서 보장 (FIFO)
        Sequencer->>Engine: 순서 보장된 주문 전달
        Engine->>OrderBook: 가격 레벨별 매칭 시도
        OrderBook-->>Engine: 부분 체결 결과
        Engine->>Sequencer: 체결 보고서 전달
        Sequencer->>Sequencer: 체결 보고서 순서 보장
        Sequencer->>WS: 체결 알림 브로드캐스트
        WS->>Frontend: 실시간 체결 업데이트
        Frontend->>Client: 부분 체결 알림
        
        Sequencer->>MDP: 체결 데이터 전송
        MDP->>WS: 시장 데이터 브로드캐스트
        WS->>Frontend: 호가창/통계 업데이트
        Frontend->>Client: 실시간 시장 데이터 업데이트
    end
    
    alt 완전 체결
        Sequencer-->>REST: 완전 체결 완료
        REST-->>Frontend: 체결 완료 응답
        Frontend->>Client: 체결 완료 알림
        
    else 부분 체결 후 주문장 추가
        Engine->>OrderBook: 남은 수량 주문장 추가
        Sequencer->>Sequencer: 주문장 변경 순서 보장
        Sequencer->>MDP: 주문장 변경 알림
        MDP->>WS: 호가창 업데이트 브로드캐스트
        Sequencer-->>REST: 부분 체결 + 주문 접수
        REST-->>Frontend: 부분 체결 + 주문 접수 응답
        Frontend->>Client: 부분 체결 + 주문 접수 알림
        
        WS->>Frontend: 호가창 업데이트 알림
        Frontend->>Client: 실시간 주문장 업데이트
    end
```

## 8. 에러 처리 시퀀스 (WebSocket 에러 메시지)

```mermaid
sequenceDiagram
    participant Client as 클라이언트
    participant Frontend as React 프론트엔드
    participant REST as Axum REST API
    participant Sequencer as 주문 시퀀서
    participant Engine as 매칭 엔진
    participant WS as WebSocket 서버

    Client->>Frontend: 잘못된 주문 제출
    Frontend->>REST: POST /v1/order (잘못된 데이터)
    
    alt API 레벨 검증 실패
        REST-->>Frontend: 400 Bad Request<br/>{error: "INVALID_QUANTITY", message: "수량은 0보다 커야 합니다"}
        Frontend->>Client: 입력 오류 메시지 표시
        
    else 엔진 레벨 오류
        REST->>Sequencer: 주문 처리 요청
        Sequencer->>Sequencer: 주문 순서 보장 (FIFO)
        Sequencer->>Engine: 순서 보장된 주문 전달
        Engine-->>Sequencer: 처리 실패 (예: 지원하지 않는 심볼)
        Sequencer->>Sequencer: 에러 메시지 순서 보장
        Sequencer->>WS: 에러 메시지 브로드캐스트
        WS->>Frontend: {"type": "Error", "message": "지원하지 않는 심볼입니다"}
        Sequencer-->>REST: 처리 실패
        REST-->>Frontend: 500 Internal Server Error<br/>{error: "SYMBOL_NOT_FOUND", message: "지원하지 않는 심볼입니다"}
        Frontend->>Client: 서버 오류 메시지 표시
        
    else 네트워크 오류
        REST-->>Frontend: 연결 타임아웃
        WS->>Frontend: {"type": "Error", "message": "네트워크 연결 오류"}
        Frontend->>Client: 네트워크 오류 메시지 표시
    end

    Note over Frontend: 사용자에게 적절한 오류 메시지 표시<br/>로깅 및 모니터링 시스템에 오류 전송
```

## 주요 컴포넌트 설명 (WebSocket 기반)

### 주문 시퀀서 (Order Sequencer)
- **핵심 기능**: 주문 순서 보장 및 체결 보고서 브로드캐스트
- **채널 관리**: 주문 입력, 체결 출력, 브로드캐스트 채널 관리
- **순서 보장**: FIFO 방식으로 주문 처리 순서 보장 (매칭엔진 앞단)
- **브로드캐스트**: 체결 정보를 WebSocket 클라이언트들에게 실시간 전송
- **중간 계층**: REST API와 매칭엔진 사이의 중간 계층 역할

### 매칭 엔진 (Matching Engine)
- **핵심 기능**: 주문 매칭 알고리즘의 핵심
- **우선순위**: 가격-시간 우선순위 원칙 적용
- **주문 타입**: 지정가/시장가 주문 처리
- **체결 관리**: 부분 체결 및 완전 체결 관리
- **스레드 안전**: Arc<Mutex>를 사용한 동시성 처리

### 주문장 (Order Book)
- **데이터 구조**: 매수/매도 주문을 가격별로 정렬하여 관리
- **효율성**: BTreeMap을 사용한 효율적인 매칭
- **스냅샷**: 실시간 주문서 스냅샷 제공
- **가격 레벨**: 동일 가격의 주문들을 그룹화하여 관리

### 시장 데이터 발행자 (MDP)
- **실시간 관리**: 실시간 시장 데이터 관리
- **WebSocket 푸시**: 호가창, 시장 통계, 봉차트 데이터를 WebSocket으로 푸시
- **봉차트**: 다양한 시간 간격의 봉차트 데이터 생성 및 관리
- **원형 버퍼**: 메모리 효율적인 시계열 데이터 관리

### REST API (Axum 기반)
- **HTTP 통신**: 클라이언트와의 HTTP 통신 담당
- **주문 관리**: 주문 제출, 취소, 조회 기능
- **초기 데이터**: 초기 시장 데이터 조회 API
- **에러 처리**: 적절한 HTTP 상태 코드 및 에러 메시지

### WebSocket 서버
- **실시간 푸시**: 체결 정보, 호가창, 시장 통계, 봉차트를 실시간 푸시
- **브로드캐스트**: 여러 클라이언트에게 동시 알림
- **양방향 통신**: 클라이언트와 서버 간 실시간 통신
- **메시지 타입**: Execution, OrderBookUpdate, MarketStatistics, CandlestickUpdate, Error

## 데이터 흐름 요약 (WebSocket 기반)

1. **주문 흐름**: 클라이언트 → 프론트엔드 → REST API → **시퀀서** → 매칭 엔진 → 주문장
2. **체결 푸시**: 매칭 엔진 → **시퀀서** → WebSocket 브로드캐스트 → 프론트엔드 → 클라이언트
3. **시장 데이터 푸시**: 매칭 엔진 → **시퀀서** → MDP → WebSocket 브로드캐스트 → 프론트엔드 → 클라이언트
4. **초기 데이터**: 클라이언트 → 프론트엔드 → REST API → MDP/매칭 엔진
5. **실시간 업데이트**: 모든 시장 데이터는 WebSocket을 통해 실시간 푸시
6. **순서 보장**: 시퀀서가 모든 주문과 체결의 순서를 보장하여 일관성 유지

## WebSocket 메시지 형식

### 체결 알림 (Execution)
```json
{
  "type": "Execution",
  "exec_id": "e1b724c2-5e61-4aba-8b8a-47d8a5a4f111",
  "order_id": "f8c3de3d-1fea-4d7c-a8b0-29f63c4c3454",
  "symbol": "BTC-KRW",
  "side": "Buy",
  "price": 50000000,
  "quantity": 0.5,
  "fee": 0.05,
  "transaction_time": "2023-04-30T12:35:10.123Z"
}
```

### 호가창 업데이트 (OrderBookUpdate)
```json
{
  "type": "OrderBookUpdate",
  "symbol": "BTC-KRW",
  "bids": [[50000000, 100], [49900000, 200]],
  "asks": [[50010000, 50], [50100000, 75]],
  "timestamp": 1682854510123
}
```

### 시장 통계 업데이트 (MarketStatistics)
```json
{
  "type": "MarketStatistics",
  "symbol": "BTC-KRW",
  "timestamp": 1682854510123,
  "last_price": 50000000,
  "price_change_24h": 2.5,
  "volume_24h": 1000000,
  "high_price_24h": 51000000,
  "low_price_24h": 49000000
}
```

### 봉차트 업데이트 (CandlestickUpdate)
```json
{
  "type": "CandlestickUpdate",
  "symbol": "BTC-KRW",
  "interval": "1m",
  "candle": {
    "open": 49900000,
    "high": 50100000,
    "low": 49800000,
    "close": 50000000,
    "volume": 1000,
    "timestamp": 1682854500000
  }
}
```

### 에러 메시지 (Error)
```json
{
  "type": "Error",
  "message": "지원하지 않는 심볼입니다"
}
```

이러한 시퀀스 다이어그램을 통해 xTrader 시스템의 WebSocket 기반 실시간 데이터 흐름과 컴포넌트 간 상호작용을 명확히 이해할 수 있습니다. 모든 시장 데이터는 폴링 방식이 아닌 WebSocket을 통해 실시간으로 푸시됩니다.