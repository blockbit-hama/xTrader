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

// 하루치 캔들 데이터 생성 (1분 캔들 기준) - 실제 시장 패턴 반영
export function generateDailyCandles(basePrice: number = 50000000): CandleData[] {
  const candles: CandleData[] = [];
  const now = Date.now();
  const minutesInDay = 24 * 60; // 1440분
  
  // 시장 상태 변수들
  let currentPrice = basePrice;
  let trendDirection = 0; // -1: 하락, 0: 횡보, 1: 상승
  let trendStrength = 0; // 트렌드 강도
  let volatility = 0.001; // 기본 변동성 (0.1%)
  
  // 지지/저항선 설정
  const supportLevel = basePrice * 0.95; // 지지선 (5% 하락)
  const resistanceLevel = basePrice * 1.05; // 저항선 (5% 상승)
  
  for (let i = minutesInDay; i >= 0; i--) {
    const timestamp = now - (i * 60 * 1000);
    
    // 시간대별 변동성 조정
    const hour = new Date(timestamp).getHours();
    if (hour >= 9 && hour <= 11) {
      volatility = 0.002; // 오전 활발 (0.2%)
    } else if (hour >= 14 && hour <= 16) {
      volatility = 0.0015; // 오후 활발 (0.15%)
    } else if (hour >= 20 && hour <= 22) {
      volatility = 0.0018; // 저녁 활발 (0.18%)
    } else if (hour >= 0 && hour <= 6) {
      volatility = 0.0005; // 새벽 비활발 (0.05%)
    } else {
      volatility = 0.001; // 기본 (0.1%)
    }
    
    // 트렌드 변화 시뮬레이션 (가끔 트렌드 전환)
    if (Math.random() < 0.01) { // 1% 확률로 트렌드 전환
      trendDirection = Math.random() < 0.5 ? -1 : 1;
      trendStrength = Math.random() * 0.5 + 0.1; // 0.1~0.6
    }
    
    // 가격 변동 계산 (현실적인 범위)
    const randomWalk = (Math.random() - 0.5) * 2; // -1 ~ +1
    const trendComponent = trendDirection * trendStrength * 0.1; // 트렌드 영향
    const priceChangePercent = (randomWalk + trendComponent) * volatility;
    
    // 지지/저항선 반영
    let adjustedChange = priceChangePercent;
    if (currentPrice <= supportLevel && priceChangePercent < 0) {
      adjustedChange *= 0.3; // 지지선에서 하락 억제
    }
    if (currentPrice >= resistanceLevel && priceChangePercent > 0) {
      adjustedChange *= 0.3; // 저항선에서 상승 억제
    }
    
    // OHLC 생성 (실제 시장 패턴 반영)
    const open = currentPrice;
    const priceChange = currentPrice * adjustedChange;
    const close = Math.max(basePrice * 0.8, Math.min(basePrice * 1.2, currentPrice + priceChange));
    
    // High/Low는 Open/Close 범위 내에서 현실적으로 생성
    const priceRange = Math.abs(close - open);
    const maxWick = priceRange * 0.5; // 최대 위크 길이
    
    const highWick = Math.random() * maxWick;
    const lowWick = Math.random() * maxWick;
    
    const high = Math.max(open, close) + highWick;
    const low = Math.min(open, close) - lowWick;
    
    // 거래량 생성 (가격 변동과 연관)
    const priceVolatility = Math.abs(close - open) / open;
    const baseVolume = 30 + Math.random() * 120; // 30-150 기본 범위
    const volumeMultiplier = 1 + priceVolatility * 10; // 변동성에 따른 거래량 증가
    const volume = Math.floor(baseVolume * volumeMultiplier);
    const tradeCount = Math.floor(volume / 2) + Math.floor(Math.random() * 15);
    
    candles.push({
      open: Math.floor(open),
      high: Math.floor(high),
      low: Math.floor(low),
      close: Math.floor(close),
      volume,
      open_time: timestamp,
      close_time: timestamp + 60000,
      trade_count: tradeCount
    });
    
    currentPrice = close;
    
    // 트렌드 강도 감소 (시간이 지나면서 약해짐)
    trendStrength *= 0.999;
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

// 뉴스 이벤트 시뮬레이션 (급등/급락) - 더 현실적으로
export function addMarketEvents(candles: CandleData[]): CandleData[] {
  const events = [
    { time: 9 * 60, impact: 0.02, type: 'positive', duration: 3 }, // 오전 9시 소폭 상승
    { time: 14 * 60, impact: -0.015, type: 'negative', duration: 2 }, // 오후 2시 소폭 하락
    { time: 20 * 60, impact: 0.025, type: 'positive', duration: 4 }, // 오후 8시 소폭 상승
  ];
  
  return candles.map((candle, index) => {
    const minuteOfDay = index;
    const event = events.find(e => 
      minuteOfDay >= e.time && minuteOfDay < e.time + e.duration
    );
    
    if (event) {
      // 이벤트 강도가 시간에 따라 감소
      const eventProgress = (minuteOfDay - event.time) / event.duration;
      const currentImpact = event.impact * (1 - eventProgress * 0.5);
      
      const multiplier = 1 + currentImpact;
      return {
        ...candle,
        open: Math.floor(candle.open * multiplier),
        high: Math.floor(candle.high * multiplier),
        low: Math.floor(candle.low * multiplier),
        close: Math.floor(candle.close * multiplier),
        volume: Math.floor(candle.volume * (1.5 + Math.random() * 0.5)), // 거래량 증가
        trade_count: Math.floor(candle.trade_count * (1.2 + Math.random() * 0.3))
      };
    }
    
    return candle;
  });
}

// 가격 연속성 보장 (캔들 간 갭 최소화)
export function ensurePriceContinuity(candles: CandleData[]): CandleData[] {
  return candles.map((candle, index) => {
    if (index === 0) return candle;
    
    const prevCandle = candles[index - 1];
    const gap = Math.abs(candle.open - prevCandle.close);
    const maxGap = prevCandle.close * 0.005; // 최대 0.5% 갭
    
    if (gap > maxGap) {
      // 갭이 너무 크면 조정
      const adjustment = (candle.open - prevCandle.close) * 0.1;
      return {
        ...candle,
        open: Math.floor(candle.open - adjustment),
        high: Math.floor(candle.high - adjustment),
        low: Math.floor(candle.low - adjustment),
        close: Math.floor(candle.close - adjustment),
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
  candles = ensurePriceContinuity(candles);
  
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