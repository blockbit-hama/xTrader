import React, { useEffect, useRef } from 'react';
import { createChart, ColorType, IChartApi, ISeriesApi, CandlestickData, Time } from 'lightweight-charts';
import { 
  calculateMA, 
  calculateEMA, 
  calculateRSI, 
  calculateMACD, 
  calculateBollingerBands, 
  calculateStochastic,
  calculateVolumeMA,
  IndicatorData 
} from '../utils/technicalIndicators';

interface CandleData {
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  open_time: number;
  close_time: number;
  trade_count: number;
}

interface TradingViewChartProps {
  data: CandleData[];
  width?: number;
  height?: number;
  enabledIndicators?: Set<string>;
}

const TradingViewChart: React.FC<TradingViewChartProps> = ({
  data,
  width = 800,
  height = 400,
  enabledIndicators = new Set()
}) => {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candlestickSeriesRef = useRef<any>(null);
  const volumeSeriesRef = useRef<any>(null);
  const indicatorSeriesRef = useRef<Map<string, any>>(new Map());

  useEffect(() => {
    if (!chartContainerRef.current) return;

    // 차트 생성
    const chart = createChart(chartContainerRef.current, {
      layout: {
        background: { type: ColorType.Solid, color: '#ffffff' },
        textColor: '#1a1a1a',
      },
      grid: {
        vertLines: { color: '#e9ecef' },
        horzLines: { color: '#e9ecef' },
      },
      crosshair: {
        mode: 1,
      },
      rightPriceScale: {
        borderColor: '#e9ecef',
      },
      timeScale: {
        borderColor: '#e9ecef',
        timeVisible: true,
        secondsVisible: false,
        rightOffset: 12,
        barSpacing: 3,
        fixLeftEdge: false,
        fixRightEdge: false,
        lockVisibleTimeRangeOnResize: true,
        rightBarStaysOnScroll: true,
        borderVisible: false,
        visible: true,
        tickMarkFormatter: (time: any, tickMarkType: any, locale: string) => {
          const date = new Date(time * 1000);
          const hours = date.getHours();
          const minutes = date.getMinutes();
          
          // 시간 표시 포맷
          if (tickMarkType === 0) { // 년/월/일
            return date.toLocaleDateString('ko-KR', { 
              month: 'short', 
              day: 'numeric' 
            });
          } else if (tickMarkType === 1) { // 시간
            return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
          } else if (tickMarkType === 2) { // 분
            return minutes.toString().padStart(2, '0');
          }
          return '';
        }
      },
      width,
      height,
    });

    chartRef.current = chart;

    // 캔들스틱 시리즈 추가
    const candlestickSeries = chart.addCandlestickSeries({
      upColor: '#28a745',
      downColor: '#dc3545',
      borderDownColor: '#dc3545',
      borderUpColor: '#28a745',
      wickDownColor: '#dc3545',
      wickUpColor: '#28a745',
    });

    candlestickSeriesRef.current = candlestickSeries;

    // 볼륨 바 시리즈 추가
    const volumeSeries = chart.addHistogramSeries({
      color: '#26a69a',
      priceFormat: {
        type: 'volume',
      },
      priceScaleId: 'volume-scale',
      scaleMargins: {
        top: 0.8,
        bottom: 0,
      },
    });

    volumeSeriesRef.current = volumeSeries;

    // 볼륨 스케일 설정
    chart.priceScale('volume-scale').applyOptions({
      scaleMargins: {
        top: 0.8,
        bottom: 0,
      },
    });

    return () => {
      chart.remove();
    };
  }, [width, height]);

  useEffect(() => {
    if (!candlestickSeriesRef.current || !volumeSeriesRef.current || !data.length) return;

    // 데이터를 TradingView 형식으로 변환
    const formattedData = data
      .map((candle) => ({
        time: Math.floor(candle.open_time / 1000) as any, // Unix timestamp in seconds
        open: candle.open, // 원래 가격 그대로 사용
        high: candle.high,
        low: candle.low,
        close: candle.close,
      }))
      // 시간순으로 정렬
      .sort((a, b) => a.time - b.time)
      // 중복된 타임스탬프 제거 (같은 시간에 여러 캔들이 있으면 마지막 것만 사용)
      .filter((candle, index, array) => {
        if (index === 0) return true;
        return candle.time !== array[index - 1].time;
      });

    // 볼륨 데이터 변환
    const volumeData = data
      .map((candle) => ({
        time: Math.floor(candle.open_time / 1000) as any,
        value: candle.volume,
        color: candle.close >= candle.open ? '#28a745' : '#dc3545', // 상승/하락에 따른 색상
      }))
      .sort((a, b) => a.time - b.time)
      .filter((volume, index, array) => {
        if (index === 0) return true;
        return volume.time !== array[index - 1].time;
      });

    console.log('📊 TradingView 차트 데이터 (정렬/중복제거):', formattedData.slice(0, 3));
    console.log('📊 볼륨 데이터:', volumeData.slice(0, 3));

    candlestickSeriesRef.current.setData(formattedData);
    volumeSeriesRef.current.setData(volumeData);
  }, [data]);

  // 차트 크기 조정
  useEffect(() => {
    if (!chartRef.current || !chartContainerRef.current) return;

    const resizeObserver = new ResizeObserver(entries => {
      const { width, height } = entries[0].contentRect;
      chartRef.current?.applyOptions({ width, height });
    });

    resizeObserver.observe(chartContainerRef.current);

    return () => resizeObserver.disconnect();
  }, []);

  // 기술적 지표 추가/제거 함수
  const updateIndicators = () => {
    if (!chartRef.current || !data.length) return;

    const chart = chartRef.current;
    const seriesMap = indicatorSeriesRef.current;

    // 기존 지표 시리즈들 정리
    seriesMap.forEach((series, key) => {
      if (!enabledIndicators.has(key)) {
        chart.removeSeries(series);
        seriesMap.delete(key);
      }
    });

    // 새로운 지표들 추가
    enabledIndicators.forEach(indicatorKey => {
      if (seriesMap.has(indicatorKey)) return; // 이미 존재하면 스킵

      let series: any = null;
      let indicatorData: IndicatorData[] = [];

      switch (indicatorKey) {
        case 'ma5':
          indicatorData = calculateMA(data, 5);
          series = chart.addLineSeries({ color: '#ff6b6b', lineWidth: 1, title: 'MA5' });
          break;
        case 'ma10':
          indicatorData = calculateMA(data, 10);
          series = chart.addLineSeries({ color: '#4ecdc4', lineWidth: 1, title: 'MA10' });
          break;
        case 'ma20':
          indicatorData = calculateMA(data, 20);
          series = chart.addLineSeries({ color: '#45b7d1', lineWidth: 1, title: 'MA20' });
          break;
        case 'ma50':
          indicatorData = calculateMA(data, 50);
          series = chart.addLineSeries({ color: '#96ceb4', lineWidth: 1, title: 'MA50' });
          break;
        case 'ma200':
          indicatorData = calculateMA(data, 200);
          series = chart.addLineSeries({ color: '#feca57', lineWidth: 1, title: 'MA200' });
          break;
        case 'ema12':
          indicatorData = calculateEMA(data, 12);
          series = chart.addLineSeries({ color: '#ff9ff3', lineWidth: 1, title: 'EMA12' });
          break;
        case 'ema26':
          indicatorData = calculateEMA(data, 26);
          series = chart.addLineSeries({ color: '#54a0ff', lineWidth: 1, title: 'EMA26' });
          break;
        case 'rsi':
          indicatorData = calculateRSI(data, 14);
          series = chart.addLineSeries({ 
            color: '#5f27cd', 
            lineWidth: 1, 
            title: 'RSI',
            priceScaleId: 'rsi-scale'
          });
          // RSI용 별도 스케일 설정
          chart.priceScale('rsi-scale').applyOptions({
            scaleMargins: { top: 0.1, bottom: 0.1 },
            mode: 1, // Percentage
          });
          break;
        case 'stoch':
          const stochData = calculateStochastic(data, 14, 3);
          series = chart.addLineSeries({ 
            color: '#00d2d3', 
            lineWidth: 1, 
            title: 'Stoch %K',
            priceScaleId: 'stoch-scale'
          });
          indicatorData = stochData.k;
          // Stochastic용 별도 스케일 설정
          chart.priceScale('stoch-scale').applyOptions({
            scaleMargins: { top: 0.1, bottom: 0.1 },
            mode: 1, // Percentage
          });
          break;
        case 'bb':
          const bbData = calculateBollingerBands(data, 20, 2);
          // 상단 밴드
          const upperSeries = chart.addLineSeries({ 
            color: '#2ed573', 
            lineWidth: 1, 
            title: 'BB Upper',
            lineStyle: 2 // Dashed
          });
          upperSeries.setData(bbData.upper.map(d => ({ ...d, time: d.time as any })));
          seriesMap.set('bb_upper', upperSeries);
          
          // 하단 밴드
          const lowerSeries = chart.addLineSeries({ 
            color: '#2ed573', 
            lineWidth: 1, 
            title: 'BB Lower',
            lineStyle: 2 // Dashed
          });
          lowerSeries.setData(bbData.lower.map(d => ({ ...d, time: d.time as any })));
          seriesMap.set('bb_lower', lowerSeries);
          
          // 중간선 (MA20)
          series = chart.addLineSeries({ 
            color: '#2ed573', 
            lineWidth: 1, 
            title: 'BB Middle' 
          });
          indicatorData = bbData.middle;
          break;
        case 'macd':
          const macdData = calculateMACD(data, 12, 26, 9);
          // MACD 라인
          series = chart.addLineSeries({ 
            color: '#ff6348', 
            lineWidth: 1, 
            title: 'MACD',
            priceScaleId: 'macd-scale'
          });
          indicatorData = macdData.macd;
          
          // 시그널 라인
          const signalSeries = chart.addLineSeries({ 
            color: '#ff6348', 
            lineWidth: 1, 
            title: 'Signal',
            priceScaleId: 'macd-scale',
            lineStyle: 2 // Dashed
          });
          signalSeries.setData(macdData.signal.map(d => ({ ...d, time: d.time as any })));
          seriesMap.set('macd_signal', signalSeries);
          
          // 히스토그램
          const histogramSeries = chart.addHistogramSeries({ 
            color: '#ff6348', 
            title: 'MACD Histogram',
            priceScaleId: 'macd-scale'
          });
          histogramSeries.setData(macdData.histogram.map(d => ({ 
            ...d, 
            time: d.time as any, 
            color: d.value >= 0 ? '#28a745' : '#dc3545' 
          })));
          seriesMap.set('macd_histogram', histogramSeries);
          
          // MACD용 별도 스케일 설정
          chart.priceScale('macd-scale').applyOptions({
            scaleMargins: { top: 0.1, bottom: 0.1 },
          });
          break;
        case 'vol_ma':
          indicatorData = calculateVolumeMA(data, 20);
          series = chart.addLineSeries({ 
            color: '#ffa502', 
            lineWidth: 1, 
            title: 'Vol MA',
            priceScaleId: 'volume-scale'
          });
          // 볼륨용 별도 스케일 설정
          chart.priceScale('volume-scale').applyOptions({
            scaleMargins: { top: 0.8, bottom: 0 },
          });
          break;
      }

      if (series && indicatorData.length > 0) {
        series.setData(indicatorData.map(d => ({ ...d, time: d.time as any })));
        seriesMap.set(indicatorKey, series);
      }
    });
  };

  // 지표 업데이트
  useEffect(() => {
    updateIndicators();
  }, [enabledIndicators, data]);

  return (
    <div style={{ position: 'relative' }}>
      <div
        ref={chartContainerRef}
        style={{
          width: '100%',
          height: `${height}px`,
          borderRadius: '4px',
          overflow: 'hidden'
        }}
      />
      <div style={{
        position: 'absolute',
        top: '10px',
        left: '10px',
        background: 'rgba(0,0,0,0.7)',
        color: '#ffffff',
        padding: '4px 8px',
        borderRadius: '4px',
        fontSize: '12px',
        zIndex: 10
      }}>
        TradingView Chart
      </div>
    </div>
  );
};

export default TradingViewChart;