import React, { useState } from 'react';
import { useTradingAPI } from '../hooks/useTradingAPI';
import TradingViewChart from './TradingViewChart';
import DepthOfMarket from './DepthOfMarket';
import { generateCompleteDailyData } from '../utils/mockDataGenerator';

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

  // Mock 호가 데이터 생성 함수
  const generateMockOrderBook = () => {
    const basePrice = 55000000; // 5500만원
    const bids: Array<{price: number; volume: number}> = [];
    const asks: Array<{price: number; volume: number}> = [];

    // 매수 호가 생성 (현재가보다 낮은 가격)
    for (let i = 0; i < 15; i++) {
      bids.push({
        price: basePrice - (i * 50000) - 50000,
        volume: Math.random() * 10 + 0.5,
      });
    }

    // 매도 호가 생성 (현재가보다 높은 가격)
    for (let i = 0; i < 15; i++) {
      asks.push({
        price: basePrice + (i * 50000) + 50000,
        volume: Math.random() * 10 + 0.5,
      });
    }

    return { bids, asks };
  };

  const mockOrderBook = generateMockOrderBook();

  // 하루치 모의 캔들 데이터 생성 (1440분 = 24시간)
  const mockCandles = generateCompleteDailyData(currentPrice);

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
    backgroundColor: '#ffffff',
    color: '#1a1a1a',
    fontFamily: 'Arial, sans-serif'
  };

  const headerStyle: React.CSSProperties = {
    backgroundColor: '#f8f9fa',
    borderBottom: '1px solid #e9ecef',
    padding: '16px 24px',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center'
  };

  const titleStyle: React.CSSProperties = {
    fontSize: '48px',
    fontWeight: 'bold',
    color: '#3b82f6'
  };

  const priceStyle: React.CSSProperties = {
    fontSize: '40px',
    fontWeight: 'bold',
    color: '#28a745'
  };

  const balanceStyle: React.CSSProperties = {
    backgroundColor: '#e9ecef',
    padding: '12px',
    borderRadius: '8px',
    fontSize: '28px',
    color: '#1a1a1a'
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
    borderLeft: '1px solid #e9ecef'
  };

  const cardStyle: React.CSSProperties = {
    backgroundColor: '#ffffff',
    border: '1px solid #e9ecef',
    borderRadius: '8px',
    padding: '16px',
    marginBottom: '16px',
    boxShadow: '0 2px 4px rgba(0,0,0,0.1)'
  };

  const chartPlaceholderStyle: React.CSSProperties = {
    ...cardStyle,
    height: '400px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    textAlign: 'center',
    color: '#6c757d'
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
    backgroundColor: '#28a745',
    color: 'white',
    marginRight: '8px'
  };

  const sellButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#dc3545',
    color: 'white'
  };

  const tabButtonStyle: React.CSSProperties = {
    padding: '8px 16px',
    border: 'none',
    background: 'transparent',
    color: '#6c757d',
    cursor: 'pointer',
    borderBottom: '2px solid transparent',
    fontSize: '28px',
    fontWeight: '500'
  };

  const activeTabButtonStyle: React.CSSProperties = {
    ...tabButtonStyle,
    color: '#1a1a1a',
    borderBottom: '2px solid #007bff'
  };

  const orderTypeButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#f8f9fa',
    color: '#6c757d',
    marginRight: '8px',
    fontSize: '24px',
    border: '1px solid #e9ecef'
  };

  const activeOrderTypeButtonStyle: React.CSSProperties = {
    ...orderTypeButtonStyle,
    backgroundColor: '#007bff',
    color: 'white',
    border: '1px solid #007bff'
  };

  const disabledButtonStyle: React.CSSProperties = {
    ...buttonStyle,
    backgroundColor: '#f8f9fa',
    color: '#6c757d',
    cursor: 'not-allowed',
    opacity: 0.6,
    border: '1px solid #e9ecef'
  };

  const inputStyle: React.CSSProperties = {
    width: '100%',
    padding: '8px 12px',
    backgroundColor: '#ffffff',
    border: '1px solid #e9ecef',
    borderRadius: '4px',
    color: '#1a1a1a',
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
            <div style={{ fontSize: '24px', color: '#6c757d' }}>Professional Trading Platform</div>
          </div>
          <div>
            <div style={{ fontSize: '24px', color: '#6c757d' }}>BTC-KRW</div>
            <div style={priceStyle}>
              {formatNumber(currentPrice)} KRW
              {displayStatistics && (
                <span style={{
                  fontSize: '28px',
                  color: displayStatistics.price_change_24h >= 0 ? '#28a745' : '#dc3545',
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
              <div style={{ fontSize: '36px', marginBottom: '8px', color: '#007bff' }}>📊 BTC-KRW 실시간 차트</div>
              <div style={{ fontSize: '28px', color: candles.length > 0 ? '#28a745' : '#ffc107', marginBottom: '8px' }}>
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
                color: '#6c757d'
              }}>
                {loading && !showMockData ? '데이터 로딩 중...' : '차트 데이터를 불러오는 중...'}
              </div>
            )}

            <div style={{ padding: '8px 16px 16px 16px', borderTop: '1px solid #e9ecef' }}>
              <div style={{ fontSize: '24px', color: '#6c757d' }}>
                고가: ₩{formatNumber(Math.max(...chartCandles.map(c => c.high || 0)))}<br/>
                저가: ₩{formatNumber(Math.min(...chartCandles.map(c => c.low || 0)))}<br/>
                거래량: {chartCandles.reduce((sum, c) => sum + (c.volume || 0), 0).toLocaleString()}
              </div>
              <div style={{ fontSize: '20px', color: '#6c757d', marginTop: '8px' }}>
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
                    borderBottom: index < 4 ? '1px solid #e9ecef' : 'none',
                    fontSize: '24px'
                  }}>
                    <span style={{ color: execution.side === 'Buy' ? '#28a745' : '#dc3545' }}>
                      {execution.side === 'Buy' ? '매수' : '매도'}
                    </span>
                    <span>₩{formatNumber(execution.price)}</span>
                    <span>{execution.quantity}</span>
                    <span style={{ color: '#6c757d' }}>
                      {new Date(execution.timestamp * 1000).toLocaleTimeString()}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <div style={{ color: '#6c757d', textAlign: 'center', padding: '20px 0' }}>
                {loading ? '체결 내역을 불러오는 중...' : '아직 체결된 거래가 없습니다.'}
              </div>
            )}
          </div>
        </div>

        {/* Right Panel - Trading Form & Order Book */}
        <div style={rightPanelStyle}>
          {/* Trading Form */}
          <div style={cardStyle} data-order-form>
            {/* 매수/매도 탭 */}
            <div style={{ display: 'flex', marginBottom: '16px', borderBottom: '1px solid #e9ecef' }}>
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
              <div style={{ fontSize: '28px', color: '#1a1a1a', marginBottom: '8px' }}>주문 타입</div>
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
                <label style={{ fontSize: '28px', color: '#1a1a1a', display: 'block', marginBottom: '4px' }}>
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
                <div style={{ fontSize: '28px', color: '#1a1a1a', marginBottom: '4px' }}>
                  예상 체결가 (KRW)
                </div>
                <div style={{
                  padding: '8px 12px',
                  backgroundColor: '#f8f9fa',
                  border: '1px solid #e9ecef',
                  borderRadius: '4px',
                  color: orderSide === 'Buy' ? '#28a745' : '#dc3545',
                  fontWeight: 'bold'
                }}>
                  ≈ {formatNumber(currentPrice)} KRW
                </div>
              </div>
            )}

            {/* 수량 입력 */}
            <div style={{ marginBottom: '12px' }}>
              <label style={{ fontSize: '28px', color: '#1a1a1a', display: 'block', marginBottom: '4px' }}>
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
            <div style={{ marginBottom: '16px', padding: '8px', backgroundColor: '#f8f9fa', borderRadius: '4px' }}>
              <div style={{ fontSize: '24px', color: '#1a1a1a' }}>
                예상 총액: {orderQuantity && parseFloat(orderQuantity) > 0 ?
                  `${formatNumber((parseFloat(orderQuantity) * (orderType === 'Market' ? currentPrice : parseFloat(orderPrice || '0'))))} KRW` :
                  '0 KRW'}
              </div>
              <div style={{ fontSize: '24px', color: '#6c757d', marginTop: '4px' }}>
                수수료 (0.1%): {orderQuantity && parseFloat(orderQuantity) > 0 ?
                  `≈ ${formatNumber((parseFloat(orderQuantity) * (orderType === 'Market' ? currentPrice : parseFloat(orderPrice || '0')) * 0.001))} KRW` :
                  '0 KRW'}
              </div>
              <div style={{ fontSize: '24px', color: '#6c757d', marginTop: '2px' }}>
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

          {/* Order Book - Depth of Market */}
          {orderBook || showMockData ? (
            <DepthOfMarket
              symbol="BTC-KRW"
              bids={orderBook ? orderBook.bids.map(bid => [bid.price, bid.volume]) : mockOrderBook.bids.map(bid => [bid.price, bid.volume])}
              asks={orderBook ? orderBook.asks.map(ask => [ask.price, ask.volume]) : mockOrderBook.asks.map(ask => [ask.price, ask.volume])}
              depth={15}
              onPriceClick={(price, side) => {
                console.log(`🎯 DOM 가격 클릭: ${price} KRW (${side})`);
                setOrderPrice(price.toString());
                setOrderSide(side === 'bid' ? 'Buy' : 'Sell');
                // 주문 폼으로 스크롤 (선택사항)
                const orderForm = document.querySelector('[data-order-form]');
                if (orderForm) {
                  orderForm.scrollIntoView({ behavior: 'smooth' });
                }
              }}
            />
          ) : (
            <div style={cardStyle}>
              <div style={{ color: '#6c757d', textAlign: 'center', padding: '40px 20px' }}>
                {loading ? '📊 호가창 데이터를 불러오는 중...' : '⚠️ 호가창 데이터를 불러올 수 없습니다.'}
              </div>
            </div>
          )}
        </div>
      </main>

      {/* Status Bar */}
      <div style={{ position: 'fixed', bottom: '16px', right: '16px' }}>
        <div style={{
          padding: '8px 12px',
          backgroundColor: '#f8f9fa',
          border: '1px solid #e9ecef',
          borderRadius: '4px',
          fontSize: '24px',
          color: loading ? '#ffc107' : error ? '#dc3545' : '#28a745'
        }}>
          {loading ? '⏳ 데이터 로딩 중...' : error ? `❌ ${error}` : '✅ 백엔드 연결됨 (포트 7000)'}
        </div>
      </div>
    </div>
  );
};

export default SimpleApp;