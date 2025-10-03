// 모의 캔들 데이터 생성 유틸리티

export interface CandleData {
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  open_time: number;
  close_time: number;
  trade_count: number;
}

// 하루치 캔들 데이터 생성 (1분 캔들 기준)
export function generateDailyCandles(basePrice: number = 50000000): CandleData[] {
  const candles: CandleData[] = [];
  const now = Date.now();
  const minutesInDay = 24 * 60; // 1440분
  
  // 시작 가격 설정
  let currentPrice = basePrice;
  
  for (let i = minutesInDay; i >= 0; i--) {
    const timestamp = now - (i * 60 * 1000); // 분 단위로 역산
    
    // 가격 변동 시뮬레이션 (랜덤 워크 + 트렌드)
    const trend = Math.sin(i / 100) * 0.001; // 장기 트렌드
    const volatility = (Math.random() - 0.5) * 0.02; // 변동성
    const priceChange = currentPrice * (trend + volatility);
    
    // OHLC 생성
    const open = currentPrice;
    const close = Math.max(1000000, currentPrice + priceChange); // 최소 100만원
    const high = Math.max(open, close) * (1 + Math.random() * 0.01); // 최대 1% 상승
    const low = Math.min(open, close) * (1 - Math.random() * 0.01); // 최대 1% 하락
    
    // 거래량과 체결 건수 생성
    const baseVolume = 50 + Math.random() * 200; // 50-250 범위
    const volumeMultiplier = 1 + Math.random() * 2; // 거래량 변동
    const volume = Math.floor(baseVolume * volumeMultiplier);
    const tradeCount = Math.floor(volume / 3) + Math.floor(Math.random() * 20); // 체결 건수
    
    candles.push({
      open: Math.floor(open),
      high: Math.floor(high),
      low: Math.floor(low),
      close: Math.floor(close),
      volume,
      open_time: timestamp,
      close_time: timestamp + 60000, // 1분 후
      trade_count: tradeCount
    });
    
    currentPrice = close;
  }
  
  return candles;
}

// 시간대별 거래량 패턴 적용
export function applyTradingPatterns(candles: CandleData[]): CandleData[] {
  return candles.map((candle, index) => {
    const hour = new Date(candle.open_time).getHours();
    
    // 시간대별 거래량 조정
    let volumeMultiplier = 1;
    if (hour >= 9 && hour <= 11) {
      volumeMultiplier = 1.5; // 오전 활발
    } else if (hour >= 14 && hour <= 16) {
      volumeMultiplier = 1.3; // 오후 활발
    } else if (hour >= 20 && hour <= 22) {
      volumeMultiplier = 1.2; // 저녁 활발
    } else if (hour >= 0 && hour <= 6) {
      volumeMultiplier = 0.3; // 새벽 비활발
    }
    
    return {
      ...candle,
      volume: Math.floor(candle.volume * volumeMultiplier),
      trade_count: Math.floor(candle.trade_count * volumeMultiplier)
    };
  });
}

// 뉴스 이벤트 시뮬레이션 (급등/급락)
export function addMarketEvents(candles: CandleData[]): CandleData[] {
  const events = [
    { time: 9 * 60, impact: 0.05, type: 'positive' }, // 오전 9시 급등
    { time: 14 * 60, impact: -0.03, type: 'negative' }, // 오후 2시 급락
    { time: 20 * 60, impact: 0.04, type: 'positive' }, // 오후 8시 급등
  ];
  
  return candles.map((candle, index) => {
    const minuteOfDay = index;
    const event = events.find(e => Math.abs(minuteOfDay - e.time) < 5);
    
    if (event) {
      const multiplier = 1 + event.impact;
      return {
        ...candle,
        open: Math.floor(candle.open * multiplier),
        high: Math.floor(candle.high * multiplier),
        low: Math.floor(candle.low * multiplier),
        close: Math.floor(candle.close * multiplier),
        volume: Math.floor(candle.volume * 2), // 이벤트 시 거래량 증가
        trade_count: Math.floor(candle.trade_count * 1.5)
      };
    }
    
    return candle;
  });
}

// 완전한 하루치 데이터 생성
export function generateCompleteDailyData(basePrice: number = 50000000): CandleData[] {
  let candles = generateDailyCandles(basePrice);
  candles = applyTradingPatterns(candles);
  candles = addMarketEvents(candles);
  
  return candles;
}

// 실시간 업데이트용 다음 캔들 생성
export function generateNextCandle(lastCandle: CandleData): CandleData {
  const now = Date.now();
  const priceChange = lastCandle.close * (Math.random() - 0.5) * 0.01; // ±1% 변동
  const newClose = Math.max(1000000, lastCandle.close + priceChange);
  
  return {
    open: lastCandle.close,
    high: Math.max(lastCandle.close, newClose) * (1 + Math.random() * 0.005),
    low: Math.min(lastCandle.close, newClose) * (1 - Math.random() * 0.005),
    close: newClose,
    volume: 50 + Math.random() * 150,
    open_time: now,
    close_time: now + 60000,
    trade_count: Math.floor(Math.random() * 30) + 10
  };
}