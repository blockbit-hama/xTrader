// 기술적 지표 계산 유틸리티

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

export interface IndicatorData {
  time: number;
  value: number;
}

// 이동평균선 (Moving Average)
export function calculateMA(candles: CandleData[], period: number): IndicatorData[] {
  const result: IndicatorData[] = [];
  
  for (let i = period - 1; i < candles.length; i++) {
    let sum = 0;
    for (let j = 0; j < period; j++) {
      sum += candles[i - j].close;
    }
    const average = sum / period;
    
    result.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: average / 1000000 // 백만원 단위로 변환
    });
  }
  
  return result;
}

// 지수이동평균 (Exponential Moving Average)
export function calculateEMA(candles: CandleData[], period: number): IndicatorData[] {
  const result: IndicatorData[] = [];
  const multiplier = 2 / (period + 1);
  
  // 첫 번째 EMA는 SMA로 시작
  let ema = candles[0].close;
  result.push({
    time: Math.floor(candles[0].open_time / 1000),
    value: ema / 1000000
  });
  
  for (let i = 1; i < candles.length; i++) {
    ema = (candles[i].close * multiplier) + (ema * (1 - multiplier));
    result.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: ema / 1000000
    });
  }
  
  return result;
}

// RSI (Relative Strength Index)
export function calculateRSI(candles: CandleData[], period: number = 14): IndicatorData[] {
  const result: IndicatorData[] = [];
  
  if (candles.length < period + 1) return result;
  
  const gains: number[] = [];
  const losses: number[] = [];
  
  // 가격 변화 계산
  for (let i = 1; i < candles.length; i++) {
    const change = candles[i].close - candles[i - 1].close;
    gains.push(change > 0 ? change : 0);
    losses.push(change < 0 ? -change : 0);
  }
  
  // 초기 평균 계산
  let avgGain = gains.slice(0, period).reduce((sum, gain) => sum + gain, 0) / period;
  let avgLoss = losses.slice(0, period).reduce((sum, loss) => sum + loss, 0) / period;
  
  // 첫 번째 RSI 계산
  const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
  const rsi = 100 - (100 / (1 + rs));
  
  result.push({
    time: Math.floor(candles[period].open_time / 1000),
    value: rsi
  });
  
  // 나머지 RSI 계산 (지수이동평균 방식)
  for (let i = period + 1; i < candles.length; i++) {
    const gain = gains[i - 1];
    const loss = losses[i - 1];
    
    avgGain = ((avgGain * (period - 1)) + gain) / period;
    avgLoss = ((avgLoss * (period - 1)) + loss) / period;
    
    const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
    const rsi = 100 - (100 / (1 + rs));
    
    result.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: rsi
    });
  }
  
  return result;
}

// MACD (Moving Average Convergence Divergence)
export function calculateMACD(candles: CandleData[], fastPeriod: number = 12, slowPeriod: number = 26, signalPeriod: number = 9): {
  macd: IndicatorData[];
  signal: IndicatorData[];
  histogram: IndicatorData[];
} {
  const fastEMA = calculateEMA(candles, fastPeriod);
  const slowEMA = calculateEMA(candles, slowPeriod);
  
  // MACD 라인 계산
  const macd: IndicatorData[] = [];
  for (let i = 0; i < fastEMA.length; i++) {
    const slowIndex = i + (slowPeriod - fastPeriod);
    if (slowIndex < slowEMA.length) {
      macd.push({
        time: fastEMA[i].time,
        value: fastEMA[i].value - slowEMA[slowIndex].value
      });
    }
  }
  
  // 시그널 라인 계산 (MACD의 EMA)
  const signal: IndicatorData[] = [];
  if (macd.length >= signalPeriod) {
    const multiplier = 2 / (signalPeriod + 1);
    let ema = macd[0].value;
    
    signal.push({
      time: macd[0].time,
      value: ema
    });
    
    for (let i = 1; i < macd.length; i++) {
      ema = (macd[i].value * multiplier) + (ema * (1 - multiplier));
      signal.push({
        time: macd[i].time,
        value: ema
      });
    }
  }
  
  // 히스토그램 계산 (MACD - Signal)
  const histogram: IndicatorData[] = [];
  for (let i = 0; i < Math.min(macd.length, signal.length); i++) {
    histogram.push({
      time: macd[i].time,
      value: macd[i].value - signal[i].value
    });
  }
  
  return { macd, signal, histogram };
}

// 볼린저 밴드 (Bollinger Bands)
export function calculateBollingerBands(candles: CandleData[], period: number = 20, stdDev: number = 2): {
  upper: IndicatorData[];
  middle: IndicatorData[];
  lower: IndicatorData[];
} {
  const middle = calculateMA(candles, period);
  const upper: IndicatorData[] = [];
  const lower: IndicatorData[] = [];
  
  for (let i = period - 1; i < candles.length; i++) {
    // 표준편차 계산
    let sum = 0;
    for (let j = 0; j < period; j++) {
      const deviation = candles[i - j].close - (middle[i - period + 1]?.value * 1000000 || 0);
      sum += deviation * deviation;
    }
    const variance = sum / period;
    const standardDeviation = Math.sqrt(variance);
    
    const middleValue = middle[i - period + 1]?.value * 1000000 || 0;
    const upperValue = middleValue + (standardDeviation * stdDev);
    const lowerValue = middleValue - (standardDeviation * stdDev);
    
    upper.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: upperValue / 1000000
    });
    
    lower.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: lowerValue / 1000000
    });
  }
  
  return { upper, lower, middle };
}

// 스토캐스틱 (Stochastic Oscillator)
export function calculateStochastic(candles: CandleData[], kPeriod: number = 14, dPeriod: number = 3): {
  k: IndicatorData[];
  d: IndicatorData[];
} {
  const k: IndicatorData[] = [];
  
  for (let i = kPeriod - 1; i < candles.length; i++) {
    let highestHigh = candles[i].high;
    let lowestLow = candles[i].low;
    
    // 기간 내 최고가/최저가 찾기
    for (let j = 0; j < kPeriod; j++) {
      highestHigh = Math.max(highestHigh, candles[i - j].high);
      lowestLow = Math.min(lowestLow, candles[i - j].low);
    }
    
    const currentClose = candles[i].close;
    const kValue = ((currentClose - lowestLow) / (highestHigh - lowestLow)) * 100;
    
    k.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: kValue
    });
  }
  
  // %D 계산 (%K의 이동평균)
  const d: IndicatorData[] = [];
  for (let i = dPeriod - 1; i < k.length; i++) {
    let sum = 0;
    for (let j = 0; j < dPeriod; j++) {
      sum += k[i - j].value;
    }
    d.push({
      time: k[i].time,
      value: sum / dPeriod
    });
  }
  
  return { k, d };
}

// 거래량 이동평균
export function calculateVolumeMA(candles: CandleData[], period: number = 20): IndicatorData[] {
  const result: IndicatorData[] = [];
  
  for (let i = period - 1; i < candles.length; i++) {
    let sum = 0;
    for (let j = 0; j < period; j++) {
      sum += candles[i - j].volume;
    }
    const average = sum / period;
    
    result.push({
      time: Math.floor(candles[i].open_time / 1000),
      value: average
    });
  }
  
  return result;
}