import React, { useEffect, useRef } from 'react';
import { createChart, ColorType, IChartApi, ISeriesApi, CandlestickData, Time } from 'lightweight-charts';

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
}

const TradingViewChart: React.FC<TradingViewChartProps> = ({
  data,
  width = 800,
  height = 400
}) => {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candlestickSeriesRef = useRef<any>(null);

  useEffect(() => {
    if (!chartContainerRef.current) return;

    // 차트 생성
    const chart = createChart(chartContainerRef.current, {
      layout: {
        background: { type: ColorType.Solid, color: '#1e1e1e' },
        textColor: '#ffffff',
      },
      grid: {
        vertLines: { color: '#2b3139' },
        horzLines: { color: '#2b3139' },
      },
      crosshair: {
        mode: 1,
      },
      rightPriceScale: {
        borderColor: '#2b3139',
      },
      timeScale: {
        borderColor: '#2b3139',
        timeVisible: true,
        secondsVisible: false,
      },
      width,
      height,
    });

    chartRef.current = chart;

    // 캔들스틱 시리즈 추가 (v4.x 방식)
    const candlestickSeries = (chart as any).addCandlestickSeries({
      upColor: '#02c076',
      downColor: '#f84960',
      borderDownColor: '#f84960',
      borderUpColor: '#02c076',
      wickDownColor: '#f84960',
      wickUpColor: '#02c076',
    });

    candlestickSeriesRef.current = candlestickSeries;

    return () => {
      chart.remove();
    };
  }, [width, height]);

  useEffect(() => {
    if (!candlestickSeriesRef.current || !data.length) return;

    // 데이터를 TradingView 형식으로 변환
    const formattedData = data
      .map((candle) => ({
        time: Math.floor(candle.open_time / 1000) as any, // Unix timestamp in seconds
        open: candle.open / 1000000, // 백만원 단위로 변환 (55백만 -> 55)
        high: candle.high / 1000000,
        low: candle.low / 1000000,
        close: candle.close / 1000000,
      }))
      // 시간순으로 정렬
      .sort((a, b) => a.time - b.time)
      // 중복된 타임스탬프 제거 (같은 시간에 여러 캔들이 있으면 마지막 것만 사용)
      .filter((candle, index, array) => {
        if (index === 0) return true;
        return candle.time !== array[index - 1].time;
      });

    console.log('📊 TradingView 차트 데이터 (정렬/중복제거):', formattedData.slice(0, 3));

    candlestickSeriesRef.current.setData(formattedData);
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