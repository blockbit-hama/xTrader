# 시장 데이터 API 문서

이 문서는 Market Data Publisher(MDP)가 제공하는 API 엔드포인트와 사용 방법에 대해 설명합니다.

## 개요

Market Data Publisher는 주문 매칭 엔진의 결과로 생성되는 시장 데이터를 관리하고 제공하는 컴포넌트입니다. 주요 기능은 다음과 같습니다:

1. 오더북 상태 관리
2. 체결 내역 저장
3. 시장 통계 계산
4. 캔들스틱(봉차트) 데이터 생성

이러한 데이터는 HTTP API를 통해 클라이언트가 조회할 수 있습니다.

## 기본 정보

- **기본 URL**: `http://127.0.0.1:3030`
- **컨텐츠 타입**: `application/json`
- **인증**: 현재 버전에서는 인증이 구현되지 않았습니다. 프로덕션 환경에서는 적절한 인증 시스템이 필요합니다.

## API 엔드포인트

### 1. 오더북 조회

특정 심볼의 현재 오더북 상태를 조회합니다.

- **URL**: `/api/v1/orderbook/{symbol}`
- **메서드**: `GET`
- **URL 파라미터**:
  - `symbol`: 오더북을 조회할 심볼 (예: BTC-KRW, ETH-KRW)

- **응답**: 오더북 데이터

```json
{
  "symbol": "BTC-KRW",
  "timestamp": 1682859310123,
  "bids": [
    { "price": 50000000, "volume": 1.5, "order_count": 3 },
    { "price": 49990000, "volume": 2.7, "order_count": 5 }
  ],
  "asks": [
    { "price": 50010000, "volume": 1.2, "order_count": 2 },
    { "price": 50020000, "volume": 3.4, "order_count": 4 }
  ]
}
```

- **상태 코드**:
  - `200 OK`: 성공
  - `404 Not Found`: 심볼을 찾을 수 없음
  - `500 Internal Server Error`: 서버 오류

### 2. 체결 내역 조회

특정 심볼의 최근 체결 내역을 조회합니다.

- **URL**: `/api/v1/executions/{symbol}`
- **메서드**: `GET`
- **URL 파라미터**:
  - `symbol`: 체결 내역을 조회할 심볼 (예: BTC-KRW, ETH-KRW)
- **쿼리 파라미터**:
  - `limit` (선택): 반환할 최대 체결 수 (기본값: 100)

- **응답**: 체결 내역 목록

```json
[
  {
    "symbol": "BTC-KRW",
    "timestamp": "2023-04-30T12:35:10.123Z",
    "price": 50000000,
    "volume": 0.5,
    "side": "Buy",
    "is_market_maker": false
  },
  {
    "symbol": "BTC-KRW",
    "timestamp": "2023-04-30T12:34:55.789Z",
    "price": 49998000,
    "volume": 0.2,
    "side": "Sell",
    "is_market_maker": true
  }
]
```

- **상태 코드**:
  - `200 OK`: 성공
  - `400 Bad Request`: 잘못된 요청
  - `500 Internal Server Error`: 서버 오류

### 3. 시장 통계 조회

특정 심볼의 24시간 시장 통계를 조회합니다.

- **URL**: `/api/v1/statistics/{symbol}`
- **메서드**: `GET`
- **URL 파라미터**:
  - `symbol`: 통계를 조회할 심볼 (예: BTC-KRW, ETH-KRW)

- **응답**: 시장 통계 데이터

```json
{
  "symbol": "BTC-KRW",
  "timestamp": "2023-04-30T12:35:10.123Z",
  "open_price_24h": 49000000,
  "high_price_24h": 51000000,
  "low_price_24h": 48500000,
  "last_price": 50000000,
  "volume_24h": 125.75,
  "price_change_24h": 2.04,
  "bid_price": 49990000,
  "ask_price": 50010000
}
```

- **상태 코드**:
  - `200 OK`: 성공
  - `404 Not Found`: 심볼을 찾을 수 없음
  - `500 Internal Server Error`: 서버 오류

### 4. 캔들스틱(봉차트) 데이터 조회

특정 심볼의 캔들스틱 데이터를 조회합니다.

- **URL**: `/api/v1/klines/{symbol}/{interval}`
- **메서드**: `GET`
- **URL 파라미터**:
  - `symbol`: 캔들스틱을 조회할 심볼 (예: BTC-KRW, ETH-KRW)
  - `interval`: 캔들스틱 간격 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w)
- **쿼리 파라미터**:
  - `limit` (선택): 반환할 최대 캔들스틱 수 (기본값: 100)

- **응답**: 캔들스틱 데이터 목록

```json
[
  {
    "symbol": "BTC-KRW",
    "open_time": "2023-04-30T12:30:00.000Z",
    "close_time": "2023-04-30T12:35:00.000Z",
    "interval": "5m",
    "open": 49900000,
    "high": 50200000,
    "low": 49800000,
    "close": 50000000,
    "volume": 3.5,
    "trade_count": 12
  },
  {
    "symbol": "BTC-KRW",
    "open_time": "2023-04-30T12:25:00.000Z",
    "close_time": "2023-04-30T12:30:00.000Z",
    "interval": "5m",
    "open": 49800000,
    "high": 50000000,
    "low": 49700000,
    "close": 49900000,
    "volume": 2.8,
    "trade_count": 9
  }
]
```

- **상태 코드**:
  - `200 OK`: 성공
  - `400 Bad Request`: 잘못된 요청 (예: 유효하지 않은 간격)
  - `404 Not Found`: 심볼을 찾을 수 없음
  - `500 Internal Server Error`: 서버 오류

## 오류 응답

오류가 발생하면 다음 형식의 JSON 응답이 반환됩니다:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "Invalid interval: 2m"
  }
}
```

## 데이터 모델

### 오더북 데이터 (OrderBook)

| 필드       | 타입     | 설명                                  |
|------------|----------|---------------------------------------|
| symbol     | string   | 거래 심볼                            |
| timestamp  | number   | 타임스탬프 (밀리초 단위 UNIX 시간)    |
| bids       | array    | 매수 호가 배열 (가격 내림차순)        |
| asks       | array    | 매도 호가 배열 (가격 오름차순)        |

### 가격 레벨 (PriceLevel)

| 필드        | 타입     | 설명                                 |
|-------------|----------|--------------------------------------|
| price       | number   | 가격 수준                           |
| volume      | number   | 해당 가격 수준의 총 수량             |
| order_count | number   | 해당 가격 수준의 주문 수             |

### 체결 데이터 (Execution)

| 필드           | 타입     | 설명                              |
|----------------|----------|-----------------------------------|
| symbol         | string   | 거래 심볼                         |
| timestamp      | string   | 체결 시간 (ISO 8601 형식)         |
| price          | number   | 체결 가격                         |
| volume         | number   | 체결 수량                         |
| side           | string   | 거래 방향 ("Buy" 또는 "Sell")     |
| is_market_maker| boolean  | 메이커 여부                       |

### 시장 통계 (Statistics)

| 필드            | 타입     | 설명                                  |
|-----------------|----------|---------------------------------------|
| symbol          | string   | 거래 심볼                            |
| timestamp       | string   | 통계 생성 시간 (ISO 8601 형식)        |
| open_price_24h  | number   | 24시간 시작가                        |
| high_price_24h  | number   | 24시간 최고가                        |
| low_price_24h   | number   | 24시간 최저가                        |
| last_price      | number   | 최근 거래가                          |
| volume_24h      | number   | 24시간 거래량                        |
| price_change_24h| number   | 24시간 가격 변화율 (%)                |
| bid_price       | number   | 최고 매수가                          |
| ask_price       | number   | 최저 매도가                          |

### 캔들스틱 (Candle)

| 필드        | 타입     | 설명                                  |
|-------------|----------|---------------------------------------|
| symbol      | string   | 거래 심볼                            |
| open_time   | string   | 시작 시간 (ISO 8601 형식)             |
| close_time  | string   | 종료 시간 (ISO 8601 형식)             |
| interval    | string   | 시간 간격 (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w) |
| open        | number   | 시가                                 |
| high        | number   | 고가                                 |
| low         | number   | 저가                                 |
| close       | number   | 종가                                 |
| volume      | number   | 거래량                               |
| trade_count | number   | 거래 횟수                            |

## 시간 간격 (Interval)

다음 시간 간격이 지원됩니다:

| 문자열 | 설명                |
|--------|---------------------|
| 1m     | 1분                 |
| 5m     | 5분                 |
| 15m    | 15분                |
| 30m    | 30분                |
| 1h     | 1시간               |
| 4h     | 4시간               |
| 1d     | 1일                 |
| 1w     | 1주일               |

## API 사용 예시

### cURL을 사용한 오더북 조회

```bash
curl -X GET "http://127.0.0.1:3030/api/v1/orderbook/BTC-KRW"
```

### cURL을 사용한 체결 내역 조회

```bash
curl -X GET "http://127.0.0.1:3030/api/v1/executions/BTC-KRW?limit=10"
```

### cURL을 사용한 시장 통계 조회

```bash
curl -X GET "http://127.0.0.1:3030/api/v1/statistics/BTC-KRW"
```

### cURL을 사용한 캔들스틱 데이터 조회

```bash
curl -X GET "http://127.0.0.1:3030/api/v1/klines/BTC-KRW/1h?limit=24"
```

## 데이터 보존 정책

각 데이터 유형별 보존 정책은 다음과 같습니다:

1. **오더북 데이터**: 최신 상태만 유지
2. **체결 내역**: 심볼당 최근 1000개 체결만 저장
3. **시장 통계**: 24시간 단위로 갱신
4. **캔들스틱 데이터**: 시간 간격별로 다음과 같이 저장
   - 1분봉: 1440개 (24시간)
   - 5분봉: 1152개 (4일)
   - 15분봉: 960개 (10일)
   - 30분봉: 1008개 (3주)
   - 1시간봉: 720개 (30일)
   - 4시간봉: 720개 (120일)
   - 일봉: 365개 (1년)
   - 주봉: 156개 (3년)

## 성능 고려사항

- API는 대량의 클라이언트 요청을 처리하도록 설계되었습니다.
- 시간 간격이 큰 캔들스틱 데이터(1h, 4h, 1d, 1w)는 계산 비용이 높으므로 캐싱이 권장됩니다.
- 빈번한 요청이 예상되는 경우, 클라이언트 측에서 적절한 캐싱 전략을 구현하는 것이 좋습니다.
