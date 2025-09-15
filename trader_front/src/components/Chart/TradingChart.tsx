import React, { useState } from 'react';
import { Candle, TimeInterval } from '../../types/trading';

interface TradingChartProps {
  symbol: string;
  interval: TimeInterval;
  data: Candle[];
  height?: number;
}

const TradingChart: React.FC<TradingChartProps> = ({
  symbol,
  interval,
  data,
  height = 400
}) => {
  return (
    <div className="w-full bg-trading-bg-secondary rounded-lg border border-trading-border">
      <div className="flex items-center justify-between p-4 border-b border-trading-border">
        <div className="flex items-center space-x-4">
          <h3 className="text-lg font-semibold text-trading-text-primary">{symbol}</h3>
          <span className="text-sm text-trading-text-muted">{interval}</span>
        </div>
        <div className="flex items-center space-x-2">
          <IntervalSelector currentInterval={interval} />
          <ChartTypeSelector />
        </div>
      </div>
      <div
        className="w-full flex items-center justify-center text-trading-text-muted"
        style={{ height: `${height}px` }}
      >
        <div className="text-center">
          <div className="text-2xl mb-2">📈</div>
          <div>차트가 여기에 표시됩니다</div>
          <div className="text-sm mt-2">
            데이터: {data.length}개 캔들
          </div>
        </div>
      </div>
    </div>
  );
};

const IntervalSelector: React.FC<{ currentInterval: TimeInterval }> = ({ currentInterval }) => {
  const intervals: TimeInterval[] = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w'];

  return (
    <div className="flex space-x-1 bg-trading-bg-primary rounded p-1">
      {intervals.map(interval => (
        <button
          key={interval}
          className={`px-2 py-1 text-xs rounded transition-colors ${
            currentInterval === interval
              ? 'bg-blue-600 text-white'
              : 'text-trading-text-muted hover:text-trading-text-primary hover:bg-trading-bg-hover'
          }`}
        >
          {interval}
        </button>
      ))}
    </div>
  );
};

const ChartTypeSelector: React.FC = () => {
  const [chartType, setChartType] = useState<'candlestick' | 'line'>('candlestick');

  return (
    <div className="flex space-x-1 bg-trading-bg-primary rounded p-1">
      <button
        onClick={() => setChartType('candlestick')}
        className={`px-2 py-1 text-xs rounded transition-colors ${
          chartType === 'candlestick'
            ? 'bg-blue-600 text-white'
            : 'text-trading-text-muted hover:text-trading-text-primary hover:bg-trading-bg-hover'
        }`}
      >
        Candles
      </button>
      <button
        onClick={() => setChartType('line')}
        className={`px-2 py-1 text-xs rounded transition-colors ${
          chartType === 'line'
            ? 'bg-blue-600 text-white'
            : 'text-trading-text-muted hover:text-trading-text-primary hover:bg-trading-bg-hover'
        }`}
      >
        Line
      </button>
    </div>
  );
};

export default TradingChart;