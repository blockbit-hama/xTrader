import React, { useState } from 'react';
import { useTradingAPI } from '../hooks/useTradingAPI';
import TradingViewChart from './TradingViewChart';

const SimpleApp: React.FC = () => {
  const [balance] = useState({ BTC: 1.5, KRW: 50000000 });
  const { statistics, candles, orderBook, executions, loading, error, submitOrder } = useTradingAPI('BTC-KRW');
  const currentPrice = statistics?.last_price || 50000000;

  // 로딩 타임아웃 처리 (1초 후 강제로 mock 데이터 표시)
  const [showMockData, setShowMockData] = useState(true); // 즉시 Mock 데이터 표시

  React.useEffect(() => {
    const timer = setTimeout(() => {
      if (loading && candles.length === 0) {
        console.log('🔄 백엔드 연결 타임아웃, Mock 데이터 표시');
        setShowMockData(true);
      }
    }, 1000); // 1초로 단축

    return () => clearTimeout(timer);
  }, [loading, candles.length]);

  // Mock 캔들 데이터 (백엔드 연결 안될 때 테스트용)
  const mockCandles = [
    { open: 48000000, high: 51000000, low: 47000000, close: 50000000, volume: 150, open_time: Date.now() - 19 * 60 * 1000, close_time: Date.now() - 19 * 60 * 1000, trade_count: 45 },
    { open: 50000000, high: 52000000, low: 49000000, close: 51000000, volume: 180, open_time: Date.now() - 18 * 60 * 1000, close_time: Date.now() - 18 * 60 * 1000, trade_count: 52 },
    { open: 51000000, high: 53000000, low: 50000000, close: 49000000, volume: 120, open_time: Date.now() - 17 * 60 * 1000, close_time: Date.now() - 17 * 60 * 1000, trade_count: 38 },
    { open: 49000000, high: 50000000, low: 47500000, close: 48500000, volume: 200, open_time: Date.now() - 16 * 60 * 1000, close_time: Date.now() - 16 * 60 * 1000, trade_count: 65 },
    { open: 48500000, high: 51000000, low: 48000000, close: 50500000, volume: 160, open_time: Date.now() - 15 * 60 * 1000, close_time: Date.now() - 15 * 60 * 1000, trade_count: 48 },
    { open: 50500000, high: 52500000, low: 50000000, close: 52000000, volume: 190, open_time: Date.now() - 14 * 60 * 1000, close_time: Date.now() - 14 * 60 * 1000, trade_count: 58 },
    { open: 52000000, high: 53000000, low: 51000000, close: 51500000, volume: 140, open_time: Date.now() - 13 * 60 * 1000, close_time: Date.now() - 13 * 60 * 1000, trade_count: 42 },
    { open: 51500000, high: 52000000, low: 49500000, close: 50000000, volume: 175, open_time: Date.now() - 12 * 60 * 1000, close_time: Date.now() - 12 * 60 * 1000, trade_count: 55 },
    { open: 50000000, high: 51500000, low: 49000000, close: 51000000, volume: 185, open_time: Date.now() - 11 * 60 * 1000, close_time: Date.now() - 11 * 60 * 1000, trade_count: 60 },
    { open: 51000000, high: 52000000, low: 50500000, close: 51800000, volume: 155, open_time: Date.now() - 10 * 60 * 1000, close_time: Date.now() - 10 * 60 * 1000, trade_count: 47 },
    { open: 51800000, high: 53500000, low: 51000000, close: 52500000, volume: 195, open_time: Date.now() - 9 * 60 * 1000, close_time: Date.now() - 9 * 60 * 1000, trade_count: 62 },
    { open: 52500000, high: 54000000, low: 52000000, close: 53000000, volume: 165, open_time: Date.now() - 8 * 60 * 1000, close_time: Date.now() - 8 * 60 * 1000, trade_count: 50 },
    { open: 53000000, high: 53500000, low: 51500000, close: 52000000, volume: 145, open_time: Date.now() - 7 * 60 * 1000, close_time: Date.now() - 7 * 60 * 1000, trade_count: 44 },
    { open: 52000000, high: 53000000, low: 51000000, close: 52800000, volume: 170, open_time: Date.now() - 6 * 60 * 1000, close_time: Date.now() - 6 * 60 * 1000, trade_count: 53 },
    { open: 52800000, high: 54000000, low: 52500000, close: 53500000, volume: 180, open_time: Date.now() - 5 * 60 * 1000, close_time: Date.now() - 5 * 60 * 1000, trade_count: 57 },
    { open: 53500000, high: 54500000, low: 53000000, close: 54000000, volume: 200, open_time: Date.now() - 4 * 60 * 1000, close_time: Date.now() - 4 * 60 * 1000, trade_count: 64 },
    { open: 54000000, high: 54200000, low: 52500000, close: 53000000, volume: 160, open_time: Date.now() - 3 * 60 * 1000, close_time: Date.now() - 3 * 60 * 1000, trade_count: 49 },
    { open: 53000000, high: 54000000, low: 52800000, close: 53800000, volume: 175, open_time: Date.now() - 2 * 60 * 1000, close_time: Date.now() - 2 * 60 * 1000, trade_count: 56 },
    { open: 53800000, high: 55000000, low: 53500000, close: 54500000, volume: 190, open_time: Date.now() - 1 * 60 * 1000, close_time: Date.now() - 1 * 60 * 1000, trade_count: 61 },
    { open: 54500000, high: 55500000, low: 54000000, close: 55000000, volume: 210, open_time: Date.now(), close_time: Date.now(), trade_count: 68 }
  ];

  // 실제 데이터가 없을 때 mock 데이터 사용 (또는 타임아웃 시)
  const chartCandles = candles.length > 0 ? candles : (showMockData || !loading ? mockCandles : []);

  // 주문 상태 관리
  const [orderType, setOrderType] = useState<'Market' | 'Limit'>('Limit');
  const [orderSide, setOrderSide] = useState<'Buy' | 'Sell'>('Buy');
  const [orderPrice, setOrderPrice] = useState<string>(currentPrice.toString());
  const [orderQuantity, setOrderQuantity] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  
  // 통계 데이터가 null인 경우 기본값 설정
  const displayStatistics = statistics ? {
    ...statistics,
    last_price: statistics.last_price || 50000000,
    price_change_24h: statistics.price_change_24h || 0,
    high_price_24h: statistics.high_price_24h || 50000000,
    low_price_24h: statistics.low_price_24h || 50000000,
    volume_24h: statistics.volume_24h || 0
  } : null;

  // 주문 처리 함수
  const handleSubmitOrder = async () => {
    if (!orderQuantity || parseFloat(orderQuantity) <= 0) {
      alert('수량을 입력해주세요.');
      return;
    }

    if (orderType === 'Limit' && (!orderPrice || parseFloat(orderPrice) <= 0)) {
      alert('지정가 주문에는 가격을 입력해주세요.');
      return;
    }

    // 잔고 확인
    const qty = parseFloat(orderQuantity);
    const price = orderType === 'Market' ? currentPrice : parseFloat(orderPrice);

    if (orderSide === 'Buy') {
      const requiredAmount = qty * price;
      if (requiredAmount > balance.KRW) {
        alert(`잔고가 부족합니다. 필요 금액: ${formatNumber(requiredAmount)} KRW`);
        return;
      }
    } else {
      if (qty > balance.BTC) {
        alert(`보유 BTC가 부족합니다. 보유량: ${balance.BTC} BTC`);
        return;
      }
    }

    try {
      setIsSubmitting(true);
      const orderData = {
        symbol: 'BTC-KRW',
        side: orderSide,
        orderType: orderType,
        price: orderType === 'Market' ? undefined : price,
        quantity: qty,
      };

      console.log('🚀 주문 제출:', orderData);
      const result = await submitOrder(orderData);
      console.log('✅ 주문 성공:', result);

      // 주문 성공 시 폼 리셋
      setOrderQuantity('');
      if (orderType === 'Limit') {
        setOrderPrice(currentPrice.toString());
      }

      alert(`주문이 성공적으로 제출되었습니다!\n주문 ID: ${result.order_id}`);
    } catch (err) {
      console.error('❌ 주문 실패:', err);
      alert(`주문 실패: ${err instanceof Error ? err.message : '알 수 없는 오류'}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  // 주문 타입이 변경될 때 가격 필드 업데이트
  React.useEffect(() => {
    if (orderType === 'Market') {
      setOrderPrice('');
    } else {
      setOrderPrice(currentPrice.toString());
    }
  }, [orderType, currentPrice]);

  // 디버깅을 위한 console.log 추가
  console.log('🔍 SimpleApp 상태:', {
    loading,
    error,
    candlesLength: candles.length,
    executionsLength: executions.length,
    statistics,
    currentPrice
  });

  const containerStyle: React.CSSProperties = {
    minHeight: '100vh',
    backgroundColor: '#0c0f13',
    color: '#ffffff',
    fontFamily: 'Arial, sans-serif'
  };

  const headerStyle: React.CSSProperties = {
    backgroundColor: '#161a20',
    borderBottom: '1px solid #2b3139',
    padding: '16px 24px',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center'
  };

  const titleStyle: React.CSSProperties = {
    fontSize: '24px',
    fontWeight: 'bold',
    color: '#3b82f6'
  };

  const priceStyle: React.CSSProperties = {
    fontSize: '20px',
    fontWeight: 'bold',
    color: '#02c076'
  };

  const balanceStyle: React.CSSProperties = {
    backgroundColor: '#1e2329',
    padding: '12px',
    borderRadius: '8px',
    fontSize: '14px'
  };

  const mainStyle: React.CSSProperties = {
    display: 'flex',
    height: 'calc(100vh - 80px)'
  };

  const leftPanelStyle: React.CSSProperties = {
    flex: '1',
    padding: '16px'
  };

  const rightPanelStyle: React.CSSProperties = {
    width: '320px',
    padding: '16px',
    borderLeft: '1px solid #2b3139'
  };

  const cardStyle: React.CSSProperties = {
    backgroundColor: '#161a20',
    border: '1px solid #2b3139',
    borderRadius: '8px',
    padding: '16px',
    marginBottom: '16px'
  };

  const chartPlaceholderStyle: React.CSSProperties = {
    ...cardStyle,
    height: '400px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    textAlign: 'center',
    color: '#848e9c'
  };

  const buttonStyle: React.CSSProperties = {
    padding: '8px 16px',
    border: 'none',
    borderRadius: '4px',
    cursor: 'pointer',
    fontWeight: '500'
  };

  const buyButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#02c076',
    color: 'white',
    marginRight: '8px'
  };

  const sellButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#f84960',
    color: 'white'
  };

  const tabButtonStyle: React.CSSProperties = {
    padding: '8px 16px',
    border: 'none',
    background: 'transparent',
    color: '#848e9c',
    cursor: 'pointer',
    borderBottom: '2px solid transparent',
    fontSize: '14px',
    fontWeight: '500'
  };

  const activeTabButtonStyle: React.CSSProperties = {
    ...tabButtonStyle,
    color: '#ffffff',
    borderBottom: '2px solid #3b82f6'
  };

  const orderTypeButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#2b3139',
    color: '#c7c9cb',
    marginRight: '8px',
    fontSize: '12px'
  };

  const activeOrderTypeButtonStyle: React.CSSProperties = {
    ...orderTypeButtonStyle,
    backgroundColor: '#3b82f6',
    color: 'white'
  };

  const disabledButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#2b3139',
    color: '#848e9c',
    cursor: 'not-allowed',
    opacity: 0.6
  };

  const inputStyle: React.CSSProperties = {
    width: '100%',
    padding: '8px 12px',
    backgroundColor: '#161a20',
    border: '1px solid #2b3139',
    borderRadius: '4px',
    color: '#ffffff',
    marginBottom: '12px'
  };

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat('ko-KR').format(num);
  };

  return (
    <div style={containerStyle}>
      {/* Header */}
      <header style={headerStyle}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '24px' }}>
          <div>
            <h1 style={titleStyle}>📈 xTrader</h1>
            <div style={{ fontSize: '12px', color: '#848e9c' }}>Professional Trading Platform</div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: '#848e9c' }}>BTC-KRW</div>
            <div style={priceStyle}>
              {formatNumber(currentPrice)} KRW
              {displayStatistics && (
                <span style={{
                  fontSize: '14px',
                  color: displayStatistics.price_change_24h >= 0 ? '#02c076' : '#f84960',
                  marginLeft: '12px'
                }}>
                  {displayStatistics.price_change_24h >= 0 ? '+' : ''}{(displayStatistics.price_change_24h * 100).toFixed(2)}%
                </span>
              )}
            </div>
          </div>
        </div>

        <div style={balanceStyle}>
          <div>KRW: {formatNumber(balance.KRW)}</div>
          <div>BTC: {balance.BTC.toFixed(6)}</div>
        </div>
      </header>

      {/* Main Content */}
      <main style={mainStyle}>
        {/* Left Panel - Chart & History */}
        <div style={leftPanelStyle}>
          {/* Chart */}
          <div style={{...cardStyle, padding: 0}}>
            <div style={{ padding: '16px 16px 8px 16px' }}>
              <div style={{ fontSize: '18px', marginBottom: '8px', color: '#3b82f6' }}>📊 BTC-KRW 실시간 차트</div>
              <div style={{ fontSize: '14px', color: candles.length > 0 ? '#02c076' : '#fbbf24', marginBottom: '8px' }}>
                {loading && candles.length === 0 && !showMockData ? (
                  '⏳ 데이터 로딩 중...'
                ) : candles.length > 0 ? (
                  `✅ 실시간 ${chartCandles.length}개 캔들 데이터 로드됨`
                ) : chartCandles.length > 0 ? (
                  `🔄 Mock ${chartCandles.length}개 캔들 데이터 표시 중`
                ) : (
                  '❌ 차트 데이터를 불러올 수 없습니다'
                )}
              </div>
            </div>

            {chartCandles.length > 0 ? (
              <TradingViewChart
                data={chartCandles}
                width={800}
                height={300}
              />
            ) : (
              <div style={{
                height: '300px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: '#848e9c'
              }}>
                {loading && !showMockData ? '데이터 로딩 중...' : '차트 데이터를 불러오는 중...'}
              </div>
            )}

            <div style={{ padding: '8px 16px 16px 16px', borderTop: '1px solid #2b3139' }}>
              <div style={{ fontSize: '12px', color: '#848e9c' }}>
                고가: ₩{formatNumber(Math.max(...chartCandles.map(c => c.high || 0)))}<br/>
                저가: ₩{formatNumber(Math.min(...chartCandles.map(c => c.low || 0)))}<br/>
                거래량: {chartCandles.reduce((sum, c) => sum + (c.volume || 0), 0).toLocaleString()}
              </div>
              <div style={{ fontSize: '10px', color: '#6b7280', marginTop: '8px' }}>
                {candles.length > 0 ? '🎯 TradingView 실시간 캔들스틱 차트' : '🧪 TradingView 테스트용 Mock 차트'}
              </div>
            </div>
          </div>

          {/* Trade History */}
          <div style={cardStyle}>
            <h3 style={{ marginTop: 0, marginBottom: '16px' }}>최근 체결 내역</h3>
            {executions.length > 0 ? (
              <div>
                {executions.slice(0, 5).map((execution, index) => (
                  <div key={index} style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    padding: '8px 0',
                    borderBottom: index < 4 ? '1px solid #2b3139' : 'none',
                    fontSize: '12px'
                  }}>
                    <span style={{ color: execution.side === 'Buy' ? '#02c076' : '#f84960' }}>
                      {execution.side === 'Buy' ? '매수' : '매도'}
                    </span>
                    <span>₩{formatNumber(execution.price)}</span>
                    <span>{execution.quantity}</span>
                    <span style={{ color: '#848e9c' }}>
                      {new Date(execution.timestamp * 1000).toLocaleTimeString()}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <div style={{ color: '#848e9c', textAlign: 'center', padding: '20px 0' }}>
                {loading ? '체결 내역을 불러오는 중...' : '아직 체결된 거래가 없습니다.'}
              </div>
            )}
          </div>
        </div>

        {/* Right Panel - Trading Form & Order Book */}
        <div style={rightPanelStyle}>
          {/* Trading Form */}
          <div style={cardStyle}>
            {/* 매수/매도 탭 */}
            <div style={{ display: 'flex', marginBottom: '16px', borderBottom: '1px solid #2b3139' }}>
              <button
                style={orderSide === 'Buy' ? activeTabButtonStyle : tabButtonStyle}
                onClick={() => setOrderSide('Buy')}
              >
                💰 매수
              </button>
              <button
                style={orderSide === 'Sell' ? activeTabButtonStyle : tabButtonStyle}
                onClick={() => setOrderSide('Sell')}
              >
                💸 매도
              </button>
            </div>

            {/* 주문 타입 선택 */}
            <div style={{ marginBottom: '16px' }}>
              <div style={{ fontSize: '14px', color: '#c7c9cb', marginBottom: '8px' }}>주문 타입</div>
              <div style={{ display: 'flex' }}>
                <button
                  style={orderType === 'Limit' ? activeOrderTypeButtonStyle : orderTypeButtonStyle}
                  onClick={() => setOrderType('Limit')}
                >
                  지정가
                </button>
                <button
                  style={orderType === 'Market' ? activeOrderTypeButtonStyle : orderTypeButtonStyle}
                  onClick={() => setOrderType('Market')}
                >
                  시장가
                </button>
              </div>
            </div>

            {/* 가격 입력 (지정가일 때만) */}
            {orderType === 'Limit' && (
              <div style={{ marginBottom: '12px' }}>
                <label style={{ fontSize: '14px', color: '#c7c9cb', display: 'block', marginBottom: '4px' }}>
                  가격 (KRW)
                </label>
                <input
                  type="number"
                  placeholder="가격을 입력하세요"
                  style={inputStyle}
                  value={orderPrice}
                  onChange={(e) => setOrderPrice(e.target.value)}
                />
              </div>
            )}

            {/* 시장가 가격 표시 */}
            {orderType === 'Market' && (
              <div style={{ marginBottom: '12px' }}>
                <div style={{ fontSize: '14px', color: '#c7c9cb', marginBottom: '4px' }}>
                  예상 체결가 (KRW)
                </div>
                <div style={{
                  padding: '8px 12px',
                  backgroundColor: '#1e2329',
                  border: '1px solid #2b3139',
                  borderRadius: '4px',
                  color: orderSide === 'Buy' ? '#02c076' : '#f84960',
                  fontWeight: 'bold'
                }}>
                  ≈ {formatNumber(currentPrice)} KRW
                </div>
              </div>
            )}

            {/* 수량 입력 */}
            <div style={{ marginBottom: '12px' }}>
              <label style={{ fontSize: '14px', color: '#c7c9cb', display: 'block', marginBottom: '4px' }}>
                수량 (BTC)
              </label>
              <input
                type="number"
                placeholder="수량을 입력하세요"
                style={inputStyle}
                value={orderQuantity}
                onChange={(e) => setOrderQuantity(e.target.value)}
                step="0.000001"
                min="0"
              />
            </div>

            {/* 예상 총액 */}
            <div style={{ marginBottom: '16px', padding: '8px', backgroundColor: '#1e2329', borderRadius: '4px' }}>
              <div style={{ fontSize: '12px', color: '#c7c9cb' }}>
                예상 총액: {orderQuantity && parseFloat(orderQuantity) > 0 ?
                  `${formatNumber((parseFloat(orderQuantity) * (orderType === 'Market' ? currentPrice : parseFloat(orderPrice || '0'))))} KRW` :
                  '0 KRW'}
              </div>
              <div style={{ fontSize: '12px', color: '#848e9c', marginTop: '4px' }}>
                수수료 (0.1%): {orderQuantity && parseFloat(orderQuantity) > 0 ?
                  `≈ ${formatNumber((parseFloat(orderQuantity) * (orderType === 'Market' ? currentPrice : parseFloat(orderPrice || '0')) * 0.001))} KRW` :
                  '0 KRW'}
              </div>
              <div style={{ fontSize: '12px', color: '#848e9c', marginTop: '2px' }}>
                {orderSide === 'Buy' ? '필요 KRW: ' : '보유 BTC: '}
                {orderSide === 'Buy' ? `${formatNumber(balance.KRW)} KRW` : `${balance.BTC} BTC`}
              </div>
            </div>

            {/* 주문 버튼 */}
            <button
              style={isSubmitting || !orderQuantity || parseFloat(orderQuantity) <= 0 ?
                disabledButtonStyle :
                (orderSide === 'Buy' ? buyButtonStyle : sellButtonStyle)}
              onClick={handleSubmitOrder}
              disabled={isSubmitting || !orderQuantity || parseFloat(orderQuantity) <= 0}
            >
              {isSubmitting ?
                '⏳ 주문 처리 중...' :
                `${orderSide === 'Buy' ? '💰 매수' : '💸 매도'} ${orderType === 'Market' ? '(시장가)' : '(지정가)'}`}
            </button>
          </div>

          {/* Order Book */}
          <div style={cardStyle}>
            <h3 style={{ marginTop: 0, marginBottom: '16px' }}>호가창</h3>

            {orderBook ? (
              <div style={{ fontSize: '12px', fontFamily: 'monospace' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '8px', marginBottom: '8px', color: '#848e9c' }}>
                  <div>가격</div>
                  <div style={{ textAlign: 'right' }}>수량</div>
                  <div style={{ textAlign: 'right' }}>누적</div>
                </div>

                {/* Ask (매도) - 높은 가격부터 */}
                {orderBook.asks.slice(0, 5).reverse().map((ask, i) => {
                  const cumulative = orderBook.asks.slice(0, orderBook.asks.indexOf(ask) + 1)
                    .reduce((sum, a) => sum + a.volume, 0);
                  return (
                    <div key={`ask-${i}`} style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '8px', marginBottom: '2px', padding: '2px 0' }}>
                      <div style={{ color: '#f84960' }}>{formatNumber(ask.price)}</div>
                      <div style={{ textAlign: 'right', color: '#ffffff' }}>{ask.volume.toFixed(6)}</div>
                      <div style={{ textAlign: 'right', color: '#c7c9cb' }}>{cumulative.toFixed(6)}</div>
                    </div>
                  );
                })}

                {/* Spread */}
                <div style={{ textAlign: 'center', padding: '8px 0', color: '#848e9c', borderTop: '1px solid #2b3139', borderBottom: '1px solid #2b3139', margin: '8px 0' }}>
                  {orderBook.asks.length > 0 && orderBook.bids.length > 0 ?
                    `스프레드: ${formatNumber(orderBook.asks[0].price - orderBook.bids[0].price)}` :
                    '스프레드: -'}
                </div>

                {/* Bid (매수) - 높은 가격부터 */}
                {orderBook.bids.slice(0, 5).map((bid, i) => {
                  const cumulative = orderBook.bids.slice(0, i + 1)
                    .reduce((sum, b) => sum + b.volume, 0);
                  return (
                    <div key={`bid-${i}`} style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '8px', marginBottom: '2px', padding: '2px 0' }}>
                      <div style={{ color: '#02c076' }}>{formatNumber(bid.price)}</div>
                      <div style={{ textAlign: 'right', color: '#ffffff' }}>{bid.volume.toFixed(6)}</div>
                      <div style={{ textAlign: 'right', color: '#c7c9cb' }}>{cumulative.toFixed(6)}</div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div style={{ color: '#848e9c', textAlign: 'center', padding: '20px 0' }}>
                {loading ? '호가창 데이터를 불러오는 중...' : '호가창 데이터를 불러올 수 없습니다.'}
              </div>
            )}
          </div>
        </div>
      </main>

      {/* Status Bar */}
      <div style={{ position: 'fixed', bottom: '16px', right: '16px' }}>
        <div style={{
          padding: '8px 12px',
          backgroundColor: '#1e2329',
          border: '1px solid #2b3139',
          borderRadius: '4px',
          fontSize: '12px',
          color: loading ? '#ffa500' : error ? '#f84960' : '#02c076'
        }}>
          {loading ? '⏳ 데이터 로딩 중...' : error ? `❌ ${error}` : '✅ 백엔드 연결됨 (포트 7000)'}
        </div>
      </div>
    </div>
  );
};

export default SimpleApp;