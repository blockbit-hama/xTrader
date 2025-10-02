import React, { useState } from 'react';
import TradingViewChart from '../TradingViewChart';

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

interface ChartTabProps {
  candles: CandleData[];
  loading: boolean;
  showMockData: boolean;
  mockCandles: CandleData[];
}

const ChartTab: React.FC<ChartTabProps> = ({ candles, loading, showMockData, mockCandles }) => {
  const [selectedInterval, setSelectedInterval] = useState<string>('1m');
  const [chartType, setChartType] = useState<'candlestick' | 'line'>('candlestick');

  // 실제 데이터가 없을 때 mock 데이터 사용
  const chartCandles = candles.length > 0 ? candles : (showMockData || !loading ? mockCandles : []);

  const intervals = [
    { value: '1m', label: '1분' },
    { value: '5m', label: '5분' },
    { value: '15m', label: '15분' },
    { value: '30m', label: '30분' },
    { value: '1h', label: '1시간' },
    { value: '4h', label: '4시간' },
    { value: '1d', label: '1일' },
    { value: '1w', label: '1주' }
  ];

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat('ko-KR').format(num);
  };

  const containerStyle: React.CSSProperties = {
    backgroundColor: '#161a20',
    border: '1px solid #2b3139',
    borderRadius: '8px',
    padding: '16px',
    height: '100%',
    display: 'flex',
    flexDirection: 'column'
  };

  const headerStyle: React.CSSProperties = {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: '16px',
    paddingBottom: '12px',
    borderBottom: '1px solid #2b3139'
  };

  const titleStyle: React.CSSProperties = {
    margin: 0,
    color: '#ffffff',
    fontSize: '18px',
    fontWeight: '600'
  };

  const controlsStyle: React.CSSProperties = {
    display: 'flex',
    gap: '8px',
    alignItems: 'center'
  };

  const buttonGroupStyle: React.CSSProperties = {
    display: 'flex',
    backgroundColor: '#1e2329',
    borderRadius: '4px',
    padding: '2px'
  };

  const buttonStyle: React.CSSProperties = {
    padding: '6px 12px',
    border: 'none',
    background: 'transparent',
    color: '#848e9c',
    cursor: 'pointer',
    fontSize: '12px',
    borderRadius: '2px',
    transition: 'all 0.2s'
  };

  const activeButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#3b82f6',
    color: '#ffffff'
  };

  const chartContainerStyle: React.CSSProperties = {
    flex: 1,
    minHeight: '400px',
    position: 'relative'
  };

  const loadingStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    height: '400px',
    color: '#848e9c',
    fontSize: '14px'
  };

  const emptyStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    height: '400px',
    color: '#848e9c',
    fontSize: '14px',
    textAlign: 'center'
  };

  const statsStyle: React.CSSProperties = {
    marginTop: '12px',
    padding: '12px',
    backgroundColor: '#1e2329',
    borderRadius: '4px',
    fontSize: '12px',
    color: '#c7c9cb'
  };

  const statsGridStyle: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: 'repeat(3, 1fr)',
    gap: '12px'
  };

  const statusStyle: React.CSSProperties = {
    fontSize: '12px',
    color: candles.length > 0 ? '#02c076' : '#fbbf24',
    marginBottom: '8px'
  };

  if (loading && candles.length === 0 && !showMockData) {
    return (
      <div style={containerStyle}>
        <div style={headerStyle}>
          <h3 style={titleStyle}>📈 실시간 차트</h3>
        </div>
        <div style={loadingStyle}>
          ⏳ 차트 데이터를 불러오는 중...
        </div>
      </div>
    );
  }

  if (chartCandles.length === 0) {
    return (
      <div style={containerStyle}>
        <div style={headerStyle}>
          <h3 style={titleStyle}>📈 실시간 차트</h3>
        </div>
        <div style={emptyStyle}>
          ❌ 차트 데이터를 불러올 수 없습니다
        </div>
      </div>
    );
  }

  // Calculate statistics
  const highPrice = Math.max(...chartCandles.map(c => c.high || 0));
  const lowPrice = Math.min(...chartCandles.map(c => c.low || 0));
  const totalVolume = chartCandles.reduce((sum, c) => sum + (c.volume || 0), 0);
  const lastPrice = chartCandles[chartCandles.length - 1]?.close || 0;
  const firstPrice = chartCandles[0]?.open || 0;
  const priceChange = lastPrice - firstPrice;
  const priceChangePercent = firstPrice > 0 ? (priceChange / firstPrice) * 100 : 0;

  return (
    <div style={containerStyle}>
      <div style={headerStyle}>
        <div>
          <h3 style={titleStyle}>📈 실시간 차트</h3>
          <div style={statusStyle}>
            {candles.length > 0 ? (
              `✅ 실시간 ${chartCandles.length}개 캔들 데이터 로드됨`
            ) : chartCandles.length > 0 ? (
              `🔄 Mock ${chartCandles.length}개 캔들 데이터 표시 중`
            ) : (
              '❌ 차트 데이터를 불러올 수 없습니다'
            )}
          </div>
        </div>
        
        <div style={controlsStyle}>
          {/* Chart Type Selector */}
          <div style={buttonGroupStyle}>
            <button
              style={chartType === 'candlestick' ? activeButtonStyle : buttonStyle}
              onClick={() => setChartType('candlestick')}
            >
              캔들
            </button>
            <button
              style={chartType === 'line' ? activeButtonStyle : buttonStyle}
              onClick={() => setChartType('line')}
            >
              라인
            </button>
          </div>

          {/* Interval Selector */}
          <div style={buttonGroupStyle}>
            {intervals.map(interval => (
              <button
                key={interval.value}
                style={selectedInterval === interval.value ? activeButtonStyle : buttonStyle}
                onClick={() => setSelectedInterval(interval.value)}
              >
                {interval.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Chart */}
      <div style={chartContainerStyle}>
        <TradingViewChart
          data={chartCandles}
          width={800}
          height={400}
        />
      </div>

      {/* Statistics */}
      <div style={statsStyle}>
        <div style={statsGridStyle}>
          <div>
            <div style={{ color: '#848e9c', marginBottom: '4px' }}>고가</div>
            <div style={{ color: '#ffffff', fontWeight: '500' }}>
              ₩{formatNumber(highPrice)}
            </div>
          </div>
          <div>
            <div style={{ color: '#848e9c', marginBottom: '4px' }}>저가</div>
            <div style={{ color: '#ffffff', fontWeight: '500' }}>
              ₩{formatNumber(lowPrice)}
            </div>
          </div>
          <div>
            <div style={{ color: '#848e9c', marginBottom: '4px' }}>거래량</div>
            <div style={{ color: '#ffffff', fontWeight: '500' }}>
              {totalVolume.toLocaleString()}
            </div>
          </div>
          <div>
            <div style={{ color: '#848e9c', marginBottom: '4px' }}>변화량</div>
            <div style={{ 
              color: priceChange >= 0 ? '#02c076' : '#f84960', 
              fontWeight: '500' 
            }}>
              {priceChange >= 0 ? '+' : ''}{formatNumber(priceChange)}
            </div>
          </div>
          <div>
            <div style={{ color: '#848e9c', marginBottom: '4px' }}>변화율</div>
            <div style={{ 
              color: priceChangePercent >= 0 ? '#02c076' : '#f84960', 
              fontWeight: '500' 
            }}>
              {priceChangePercent >= 0 ? '+' : ''}{priceChangePercent.toFixed(2)}%
            </div>
          </div>
          <div>
            <div style={{ color: '#848e9c', marginBottom: '4px' }}>현재가</div>
            <div style={{ color: '#ffffff', fontWeight: '500' }}>
              ₩{formatNumber(lastPrice)}
            </div>
          </div>
        </div>
        
        <div style={{ 
          marginTop: '8px', 
          fontSize: '10px', 
          color: '#6b7280',
          textAlign: 'center'
        }}>
          {candles.length > 0 ? '🎯 TradingView 실시간 캔들스틱 차트' : '🧪 TradingView 테스트용 Mock 차트'}
        </div>
      </div>
    </div>
  );
};

export default ChartTab;