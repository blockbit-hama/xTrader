import React, { useState } from 'react';
import { useTradingAPI } from '../hooks/useTradingAPI';
import OrderTab from './Tabs/OrderTab';
import OrderBookTab from './Tabs/OrderBookTab';
import AdvancedDOMTab from './Tabs/AdvancedDOMTab';
import ChartTab from './Tabs/ChartTab';
import PortfolioTab from './Tabs/PortfolioTab';

type TabType = 'order' | 'orderbook' | 'advanced-dom' | 'chart' | 'portfolio';

const TabbedApp: React.FC = () => {
  const [activeTab, setActiveTab] = useState<TabType>('chart');
  const [balance] = useState({ BTC: 100.0, KRW: 50000000000 }); // 테스트용 대용량 잔고
  const { statistics, candles, orderBook, executions, loading, error, submitOrder } = useTradingAPI('BTC-KRW');
  const currentPrice = statistics?.last_price || 50000000;

  // 로딩 타임아웃 처리 (1초 후 강제로 mock 데이터 표시)
  const [showMockData, setShowMockData] = useState(true);

  React.useEffect(() => {
    const timer = setTimeout(() => {
      if (loading && candles.length === 0) {
        console.log('🔄 백엔드 연결 타임아웃, Mock 데이터 표시');
        setShowMockData(true);
      }
    }, 1000);

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

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat('ko-KR').format(num);
  };

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
    fontSize: '24px',
    fontWeight: 'bold',
    color: '#007bff'
  };

  const priceStyle: React.CSSProperties = {
    fontSize: '20px',
    fontWeight: 'bold',
    color: '#28a745'
  };

  const balanceStyle: React.CSSProperties = {
    backgroundColor: '#e9ecef',
    padding: '12px',
    borderRadius: '8px',
    fontSize: '14px',
    color: '#1a1a1a'
  };

  const mainStyle: React.CSSProperties = {
    display: 'flex',
    height: 'calc(100vh - 80px)'
  };

  const leftPanelStyle: React.CSSProperties = {
    flex: '1',
    padding: '16px',
    display: 'flex',
    flexDirection: 'column',
    width: '100%'
  };

  const tabContainerStyle: React.CSSProperties = {
    backgroundColor: '#ffffff',
    border: '1px solid #e9ecef',
    borderRadius: '8px',
    padding: '16px',
    flex: 1,
    overflow: 'hidden',
    boxShadow: '0 2px 4px rgba(0,0,0,0.1)'
  };

  const tabHeaderStyle: React.CSSProperties = {
    display: 'flex',
    borderBottom: '1px solid #e9ecef',
    marginBottom: '16px',
    paddingBottom: '8px'
  };

  const tabButtonStyle: React.CSSProperties = {
    padding: '8px 16px',
    border: 'none',
    background: 'transparent',
    color: '#6c757d',
    cursor: 'pointer',
    borderBottom: '2px solid transparent',
    fontSize: '14px',
    fontWeight: '500',
    marginRight: '8px',
    transition: 'all 0.2s'
  };

  const activeTabButtonStyle: React.CSSProperties = {
    ...tabButtonStyle,
    color: '#1a1a1a',
    borderBottom: '2px solid #007bff'
  };

  const tabContentStyle: React.CSSProperties = {
    height: 'calc(100% - 60px)',
    overflow: 'auto'
  };

  const statusBarStyle: React.CSSProperties = {
    position: 'fixed',
    bottom: '16px',
    right: '16px',
    padding: '8px 12px',
    backgroundColor: '#f8f9fa',
    border: '1px solid #e9ecef',
    borderRadius: '4px',
    fontSize: '12px',
    color: loading ? '#ffc107' : error ? '#dc3545' : '#28a745'
  };

  const tabs = [
    { id: 'order' as TabType, label: '💰 주문', icon: '💰' },
    { id: 'orderbook' as TabType, label: '📊 호가', icon: '📊' },
    { id: 'advanced-dom' as TabType, label: '🎯 고급DOM', icon: '🎯' },
    { id: 'chart' as TabType, label: '📈 차트', icon: '📈' },
    { id: 'portfolio' as TabType, label: '💼 자산', icon: '💼' }
  ];

  const renderTabContent = () => {
    switch (activeTab) {
      case 'order':
        return (
          <OrderTab
            currentPrice={currentPrice}
            balance={balance}
            onSubmitOrder={submitOrder}
          />
        );
      case 'orderbook':
        return (
          <OrderBookTab
            orderBook={orderBook}
            loading={loading}
            onPriceClick={(price, side) => {
              console.log(`🎯 DOM 가격 클릭: ${price} KRW (${side})`);
              // 가격 클릭 시 주문 탭으로 이동
              setActiveTab('order');
              // 실제로는 주문 폼에 가격을 설정하는 로직이 필요
            }}
          />
        );
      case 'advanced-dom':
        return (
          <AdvancedDOMTab
            orderBook={orderBook}
            statistics={statistics}
            executions={executions}
            loading={loading}
            balance={balance}
            onSubmitOrder={submitOrder}
            candles={candles}
            showMockData={showMockData}
            mockCandles={mockCandles}
            onPriceClick={(price: number, side: 'bid' | 'ask') => {
              console.log(`🎯 고급 DOM 가격 클릭: ${price} KRW (${side})`);
            }}
          />
        );
      case 'chart':
        return (
          <ChartTab
            candles={candles}
            loading={loading}
            showMockData={showMockData}
            mockCandles={mockCandles}
          />
        );
      case 'portfolio':
        return (
          <PortfolioTab
            balance={balance}
            currentPrice={currentPrice}
            statistics={statistics}
          />
        );
      default:
        return null;
    }
  };

  return (
    <div style={containerStyle}>
      {/* Header */}
      <header style={headerStyle}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '24px' }}>
          <div>
            <h1 style={titleStyle}>📈 xTrader</h1>
            <div style={{ fontSize: '12px', color: '#6c757d' }}>Professional Trading Platform</div>
          </div>
          <div>
            <div style={{ fontSize: '12px', color: '#6c757d' }}>BTC-KRW</div>
            <div style={priceStyle}>
              {formatNumber(currentPrice)} KRW
              {statistics && (
                <span style={{
                  fontSize: '14px',
                  color: statistics.price_change_24h >= 0 ? '#28a745' : '#dc3545',
                  marginLeft: '12px'
                }}>
                  {statistics.price_change_24h >= 0 ? '+' : ''}{(statistics.price_change_24h * 100).toFixed(2)}%
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
        {/* Left Panel - Main Content */}
        <div style={leftPanelStyle}>
          <div style={tabContainerStyle}>
            <div style={tabHeaderStyle}>
              {tabs.map(tab => (
                <button
                  key={tab.id}
                  style={activeTab === tab.id ? activeTabButtonStyle : tabButtonStyle}
                  onClick={() => setActiveTab(tab.id)}
                >
                  {tab.icon} {tab.label}
                </button>
              ))}
            </div>
            <div style={tabContentStyle}>
              {renderTabContent()}
            </div>
          </div>
        </div>

      </main>

      {/* Status Bar */}
      <div style={statusBarStyle}>
        {loading ? '⏳ 데이터 로딩 중...' : error ? `❌ ${error}` : '✅ 백엔드 연결됨 (포트 7000)'}
      </div>
    </div>
  );
};

export default TabbedApp;