import React, { useState, useEffect } from 'react';

interface PortfolioTabProps {
  balance: { BTC: number; KRW: number };
  currentPrice: number;
  statistics: any;
}

interface Asset {
  symbol: string;
  name: string;
  balance: number;
  price: number;
  value: number;
  change24h: number;
  changePercent24h: number;
}

const PortfolioTab: React.FC<PortfolioTabProps> = ({ balance, currentPrice, statistics }) => {
  const [selectedTimeframe, setSelectedTimeframe] = useState<'1h' | '24h' | '7d' | '30d'>('24h');
  const [portfolioValue, setPortfolioValue] = useState<number>(0);
  const [totalChange, setTotalChange] = useState<number>(0);
  const [totalChangePercent, setTotalChangePercent] = useState<number>(0);

  // Mock portfolio data with test balances
  const assets: Asset[] = [
    {
      symbol: 'BTC',
      name: 'Bitcoin',
      balance: balance.BTC,
      price: currentPrice,
      value: balance.BTC * currentPrice,
      change24h: statistics?.price_change_24h ? (statistics.price_change_24h * currentPrice) : 0,
      changePercent24h: statistics?.price_change_24h ? (statistics.price_change_24h * 100) : 0
    },
    {
      symbol: 'KRW',
      name: 'Korean Won',
      balance: balance.KRW,
      price: 1,
      value: balance.KRW,
      change24h: 0,
      changePercent24h: 0
    }
  ];

  // Test user info
  const testUserInfo = {
    userId: 'test_user_001',
    name: '테스트 트레이더',
    accountType: 'VIP',
    tradingLevel: 'Professional'
  };

  // Calculate portfolio metrics
  useEffect(() => {
    const totalValue = assets.reduce((sum, asset) => sum + asset.value, 0);
    const totalChangeValue = assets.reduce((sum, asset) => sum + asset.change24h, 0);
    const totalChangePercentValue = totalValue > 0 ? (totalChangeValue / totalValue) * 100 : 0;

    setPortfolioValue(totalValue);
    setTotalChange(totalChangeValue);
    setTotalChangePercent(totalChangePercentValue);
  }, [balance, currentPrice, statistics]);

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat('ko-KR').format(num);
  };

  const formatCurrency = (num: number) => {
    return new Intl.NumberFormat('ko-KR', {
      style: 'currency',
      currency: 'KRW'
    }).format(num);
  };

  const formatPercent = (num: number) => {
    return `${num >= 0 ? '+' : ''}${num.toFixed(2)}%`;
  };

  const containerStyle: React.CSSProperties = {
    backgroundColor: '#ffffff',
    border: '1px solid #e9ecef',
    borderRadius: '8px',
    padding: '16px',
    height: '100%',
    overflow: 'auto',
    boxShadow: '0 2px 4px rgba(0,0,0,0.1)'
  };

  const headerStyle: React.CSSProperties = {
    marginBottom: '20px',
    paddingBottom: '16px',
    borderBottom: '1px solid #e9ecef'
  };

  const titleStyle: React.CSSProperties = {
    margin: 0,
    color: '#1a1a1a',
    fontSize: '18px',
    fontWeight: '600',
    marginBottom: '8px'
  };

  const testAccountStyle: React.CSSProperties = {
    backgroundColor: '#e3f2fd',
    border: '1px solid #2196f3',
    borderRadius: '8px',
    padding: '12px',
    marginBottom: '16px',
    fontSize: '14px',
    color: '#1976d2'
  };

  const portfolioSummaryStyle: React.CSSProperties = {
    backgroundColor: '#f8f9fa',
    borderRadius: '8px',
    padding: '16px',
    marginBottom: '20px',
    border: '1px solid #e9ecef'
  };

  const totalValueStyle: React.CSSProperties = {
    fontSize: '24px',
    fontWeight: 'bold',
    color: '#1a1a1a',
    marginBottom: '8px'
  };

  const changeStyle: React.CSSProperties = {
    fontSize: '14px',
    fontWeight: '500',
    marginBottom: '4px'
  };

  const timeframeStyle: React.CSSProperties = {
    fontSize: '12px',
    color: '#848e9c'
  };

  const timeframeSelectorStyle: React.CSSProperties = {
    display: 'flex',
    backgroundColor: '#1e2329',
    borderRadius: '4px',
    padding: '2px',
    marginBottom: '20px'
  };

  const timeframeButtonStyle: React.CSSProperties = {
    padding: '6px 12px',
    border: 'none',
    background: 'transparent',
    color: '#848e9c',
    cursor: 'pointer',
    fontSize: '12px',
    borderRadius: '2px',
    transition: 'all 0.2s'
  };

  const activeTimeframeButtonStyle: React.CSSProperties = {
    ...timeframeButtonStyle,
    backgroundColor: '#3b82f6',
    color: '#ffffff'
  };

  const assetCardStyle: React.CSSProperties = {
    backgroundColor: '#1e2329',
    borderRadius: '8px',
    padding: '16px',
    marginBottom: '12px',
    border: '1px solid #2b3139'
  };

  const assetHeaderStyle: React.CSSProperties = {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: '12px'
  };

  const assetNameStyle: React.CSSProperties = {
    fontSize: '16px',
    fontWeight: '600',
    color: '#ffffff'
  };

  const assetSymbolStyle: React.CSSProperties = {
    fontSize: '12px',
    color: '#848e9c',
    marginTop: '2px'
  };

  const assetPriceStyle: React.CSSProperties = {
    fontSize: '14px',
    fontWeight: '500',
    color: '#ffffff',
    textAlign: 'right'
  };

  const assetChangeStyle: React.CSSProperties = {
    fontSize: '12px',
    fontWeight: '500',
    textAlign: 'right',
    marginTop: '2px'
  };

  const assetDetailsStyle: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr',
    gap: '12px',
    fontSize: '12px'
  };

  const detailLabelStyle: React.CSSProperties = {
    color: '#848e9c',
    marginBottom: '4px'
  };

  const detailValueStyle: React.CSSProperties = {
    color: '#ffffff',
    fontWeight: '500'
  };

  const emptyStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    height: '200px',
    color: '#848e9c',
    fontSize: '14px',
    textAlign: 'center'
  };

  return (
    <div style={containerStyle}>
      <div style={headerStyle}>
        <h3 style={titleStyle}>💼 포트폴리오</h3>
        
        {/* Test Account Info */}
        <div style={testAccountStyle}>
          <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>
            🧪 테스트 계정 (실제 거래 가능)
          </div>
          <div>사용자: {testUserInfo.name} ({testUserInfo.userId})</div>
          <div>계정 유형: {testUserInfo.accountType} | 거래 레벨: {testUserInfo.tradingLevel}</div>
          <div style={{ marginTop: '4px', fontSize: '12px', opacity: 0.8 }}>
            💡 이 계정으로 실제 매수/매도 주문이 가능합니다
          </div>
        </div>
        
        {/* Timeframe Selector */}
        <div style={timeframeSelectorStyle}>
          <button
            style={selectedTimeframe === '1h' ? activeTimeframeButtonStyle : timeframeButtonStyle}
            onClick={() => setSelectedTimeframe('1h')}
          >
            1시간
          </button>
          <button
            style={selectedTimeframe === '24h' ? activeTimeframeButtonStyle : timeframeButtonStyle}
            onClick={() => setSelectedTimeframe('24h')}
          >
            24시간
          </button>
          <button
            style={selectedTimeframe === '7d' ? activeTimeframeButtonStyle : timeframeButtonStyle}
            onClick={() => setSelectedTimeframe('7d')}
          >
            7일
          </button>
          <button
            style={selectedTimeframe === '30d' ? activeTimeframeButtonStyle : timeframeButtonStyle}
            onClick={() => setSelectedTimeframe('30d')}
          >
            30일
          </button>
        </div>
      </div>

      {/* Portfolio Summary */}
      <div style={portfolioSummaryStyle}>
        <div style={totalValueStyle}>
          {formatCurrency(portfolioValue)}
        </div>
        <div style={{
          ...changeStyle,
          color: totalChange >= 0 ? '#02c076' : '#f84960'
        }}>
          {totalChange >= 0 ? '+' : ''}{formatCurrency(totalChange)}
        </div>
        <div style={{
          ...changeStyle,
          color: totalChangePercent >= 0 ? '#02c076' : '#f84960'
        }}>
          {formatPercent(totalChangePercent)}
        </div>
        <div style={timeframeStyle}>
          {selectedTimeframe === '1h' ? '1시간' : 
           selectedTimeframe === '24h' ? '24시간' :
           selectedTimeframe === '7d' ? '7일' : '30일'} 기준
        </div>
      </div>

      {/* Assets List */}
      {assets.length > 0 ? (
        <div>
          {assets.map((asset, index) => (
            <div key={asset.symbol} style={assetCardStyle}>
              <div style={assetHeaderStyle}>
                <div>
                  <div style={assetNameStyle}>{asset.name}</div>
                  <div style={assetSymbolStyle}>{asset.symbol}</div>
                </div>
                <div>
                  <div style={assetPriceStyle}>
                    {asset.symbol === 'KRW' ? 
                      formatCurrency(asset.price) : 
                      `₩${formatNumber(asset.price)}`
                    }
                  </div>
                  <div style={{
                    ...assetChangeStyle,
                    color: asset.changePercent24h >= 0 ? '#02c076' : '#f84960'
                  }}>
                    {asset.changePercent24h !== 0 ? formatPercent(asset.changePercent24h) : '0.00%'}
                  </div>
                </div>
              </div>
              
              <div style={assetDetailsStyle}>
                <div>
                  <div style={detailLabelStyle}>보유량</div>
                  <div style={detailValueStyle}>
                    {asset.symbol === 'KRW' ? 
                      formatCurrency(asset.balance) : 
                      `${asset.balance.toFixed(6)} ${asset.symbol}`
                    }
                  </div>
                </div>
                <div>
                  <div style={detailLabelStyle}>총 가치</div>
                  <div style={detailValueStyle}>
                    {formatCurrency(asset.value)}
                  </div>
                </div>
                <div>
                  <div style={detailLabelStyle}>변화량</div>
                  <div style={{
                    ...detailValueStyle,
                    color: asset.change24h >= 0 ? '#02c076' : '#f84960'
                  }}>
                    {asset.change24h >= 0 ? '+' : ''}{formatCurrency(asset.change24h)}
                  </div>
                </div>
                <div>
                  <div style={detailLabelStyle}>비중</div>
                  <div style={detailValueStyle}>
                    {portfolioValue > 0 ? 
                      `${((asset.value / portfolioValue) * 100).toFixed(1)}%` : 
                      '0.0%'
                    }
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div style={emptyStyle}>
          보유 자산이 없습니다
        </div>
      )}

      {/* Additional Info */}
      <div style={{
        marginTop: '20px',
        padding: '12px',
        backgroundColor: '#1e2329',
        borderRadius: '4px',
        fontSize: '12px',
        color: '#848e9c',
        textAlign: 'center'
      }}>
        💡 실시간 가격은 BTC-KRW 기준으로 계산됩니다
      </div>
    </div>
  );
};

export default PortfolioTab;