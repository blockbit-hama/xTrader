# WebSocket 체결 알림 프로토콜

이 문서는 주문 매칭 엔진의 WebSocket 체결 알림 기능에 대한 상세 정보를 제공합니다.

## 개요

주문 매칭 엔진은 체결 정보를 실시간으로 클라이언트에게 푸시하기 위한 WebSocket 인터페이스를 제공합니다. 이 인터페이스는 주문 처리 파이프라인과 독립적으로 작동하므로, 많은 수의 클라이언트 연결이 핵심 매칭 기능에 영향을 주지 않습니다.

## 연결 정보

- WebSocket 기본 URL: `ws://127.0.0.1:3030`
- 프로토콜: WebSocket (RFC 6455)
- 인코딩: UTF-8
- 메시지 형식: JSON

## 체결 스트림

### 연결 URL

```
ws://127.0.0.1:3030/ws/executions
```

### 메시지 형식

서버로부터 수신하는 메시지는 체결이 발생할 때마다 전송됩니다:

```json
{
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

### 필드 설명

| 필드             | 타입     | 설명                                 |
|-----------------|----------|--------------------------------------|
| exec_id         | string   | 체결 고유 식별자                      |
| order_id        | string   | 연관된 주문 ID                       |
| symbol          | string   | 거래 심볼 (예: "BTC-KRW")            |
| side            | string   | 거래 방향 ("Buy" 또는 "Sell")        |
| price           | number   | 체결 가격                            |
| quantity        | number   | 체결 수량                            |
| fee             | number   | 거래 수수료                          |
| transaction_time | string   | 체결 발생 시간 (ISO 8601 형식)       |

### 사용 예시 (JavaScript)

```javascript
const socket = new WebSocket('ws://127.0.0.1:3030/ws/executions');

socket.onopen = () => {
  console.log('체결 스트림에 연결되었습니다.');
};

socket.onmessage = (event) => {
  const execution = JSON.parse(event.data);
  console.log(`체결 발생: ${execution.symbol} - ${execution.side}, 가격: ${execution.price}, 수량: ${execution.quantity}`);
};

socket.onclose = () => {
  console.log('체결 스트림 연결이 종료되었습니다.');
};

socket.onerror = (error) => {
  console.error('WebSocket 오류:', error);
};
```

## 주의사항

1. WebSocket 체결 알림은 실시간으로 체결이 발생할 때만 메시지를 전송합니다.
2. 과거 체결 내역은 Market Data Publisher의 HTTP API(`/api/v1/executions/{symbol}`)를 통해 조회할 수 있습니다.
3. 현재 구현에서는 클라이언트 인증 메커니즘이 없으므로, 프로덕션 환경에서는 적절한 인증 시스템을 구현해야 합니다.

## 성능 및 확장성 고려사항

### 연결 제한

현재 구현에서는 연결 수 제한이 명시적으로 설정되어 있지 않지만, 프로덕션 환경에서는 다음과 같은 제한을 고려해야 합니다:
- 클라이언트당 최대 연결 수
- IP 주소당 최대 연결 수
- 서버 전체 최대 연결 수

### 데이터 압축

대역폭 사용량을 줄이기 위해 다음과 같은 최적화를 고려할 수 있습니다:
- WebSocket 메시지 압축 (RFC 7692)
- 이진 메시지 형식 (JSON 대신 Protocol Buffers, MessagePack 등)

### 부하 분산

많은 수의 클라이언트를 지원하기 위해 다음과 같은 접근 방식을 사용할 수 있습니다:
- WebSocket 연결을 위한 별도의 서버 풀
- 메시지 브로커 사용 (Redis PubSub, Kafka 등)

## 연결 관리

### 연결 유지 (Keepalive)

WebSocket 연결은 일정 시간 동안 활동이 없을 경우 중간 프록시나 방화벽에 의해 종료될 수 있습니다. 이를 방지하기 위해:

1. 클라이언트는 주기적으로 핑(ping) 프레임을 전송할 수 있습니다.
2. 서버는 클라이언트의 핑에 대해 퐁(pong) 프레임으로 응답합니다.

대부분의 WebSocket 클라이언트 라이브러리는 이러한 핑/퐁 메커니즘을 자동으로 처리합니다.

### 재연결 전략

클라이언트 측에서는 연결이 끊어졌을 경우를 대비한 재연결 전략을 구현하는 것이 권장됩니다:

1. 지수 백오프 알고리즘을 사용하여 재시도 간격을 점진적으로 늘림
2. 최대 재시도 횟수 설정
3. 연결이 성공적으로 복구되면 필요한 구독 다시 요청

### 재연결 예시 (JavaScript)

```javascript
class WebSocketClient {
  constructor(url, options = {}) {
    this.url = url;
    this.options = {
      reconnectAttempts: 10,
      initialReconnectDelay: 1000,
      maxReconnectDelay: 30000,
      ...options
    };
    this.reconnectAttempt = 0;
    this.connect();
  }
  
  connect() {
    this.socket = new WebSocket(this.url);
    
    this.socket.onopen = () => {
      console.log('WebSocket 연결 성공');
      this.reconnectAttempt = 0;
      if (this.options.onOpen) {
        this.options.onOpen();
      }
    };
    
    this.socket.onmessage = (event) => {
      if (this.options.onMessage) {
        this.options.onMessage(event);
      }
    };
    
    this.socket.onclose = () => {
      console.log('WebSocket 연결 종료');
      this.reconnect();
    };
    
    this.socket.onerror = (error) => {
      console.error('WebSocket 오류:', error);
      if (this.options.onError) {
        this.options.onError(error);
      }
    };
  }
  
  reconnect() {
    if (this.reconnectAttempt >= this.options.reconnectAttempts) {
      console.log('최대 재연결 시도 횟수 초과');
      if (this.options.onReconnectFailed) {
        this.options.onReconnectFailed();
      }
      return;
    }
    
    this.reconnectAttempt++;
    const delay = Math.min(
      this.options.initialReconnectDelay * Math.pow(1.5, this.reconnectAttempt - 1),
      this.options.maxReconnectDelay
    );
    
    console.log(`${delay}ms 후 재연결 시도 (${this.reconnectAttempt}/${this.options.reconnectAttempts})`);
    
    setTimeout(() => {
      this.connect();
      if (this.options.onReconnect) {
        this.options.onReconnect(this.reconnectAttempt);
      }
    }, delay);
  }
  
  close() {
    this.socket.close();
  }
}

// 사용 예시
const executionsClient = new WebSocketClient('ws://127.0.0.1:3030/ws/executions', {
  onOpen: () => {
    console.log('체결 스트림 연결 성공');
  },
  onMessage: (event) => {
    const execution = JSON.parse(event.data);
    console.log(`체결 발생: ${execution.symbol} - ${execution.side}, 가격: ${execution.price}`);
  }
});
```
