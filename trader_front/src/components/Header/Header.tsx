import React from 'react';
import { TrendingUp, Wallet, Settings, Bell, User, Activity } from 'lucide-react';
import { MarketStatistics } from '../../types/trading';

interface HeaderProps {
  statistics: MarketStatistics | null;
  balance: { [key: string]: number };
}

const Header: React.FC<HeaderProps> = ({ statistics, balance }) => {
  const formatPrice = (price: number) => {
    return new Intl.NumberFormat('ko-KR', {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(price);
  };

  const formatPercentage = (change: number) => {
    return `${change > 0 ? '+' : ''}${change.toFixed(2)}%`;
  };

  const getPriceColor = (change: number) => {
    if (change > 0) return 'text-trading-green';
    if (change < 0) return 'text-trading-red';
    return 'text-trading-text-secondary';
  };

  return (
    <header className="bg-trading-bg-secondary border-b border-trading-border px-6 py-4">
      <div className="flex items-center justify-between">
        {/* Logo and Title */}
        <div className="flex items-center space-x-6">
          <div className="flex items-center space-x-3">
            <div className="flex items-center space-x-2">
              <TrendingUp className="text-blue-500" size={24} />
              <h1 className="text-xl font-bold text-trading-text-primary">xTrader</h1>
            </div>
            <div className="text-sm text-trading-text-muted">
              Professional Trading Platform
            </div>
          </div>

          {/* Market Info */}
          {statistics && (
            <div className="flex items-center space-x-8">
              <div className="flex flex-col">
                <div className="text-sm text-trading-text-muted">현재가</div>
                <div className="text-lg font-mono font-semibold text-trading-text-primary">
                  {formatPrice(statistics.last_price)}
                  <span className="text-xs ml-1">KRW</span>
                </div>
              </div>

              <div className="flex flex-col">
                <div className="text-sm text-trading-text-muted">24h 변동</div>
                <div className={`text-lg font-mono font-semibold ${getPriceColor(statistics.price_change_24h)}`}>
                  {formatPercentage(statistics.price_change_24h)}
                </div>
              </div>

              <div className="flex flex-col">
                <div className="text-sm text-trading-text-muted">24h 고가</div>
                <div className="text-sm font-mono text-trading-text-primary">
                  {formatPrice(statistics.high_price_24h)}
                </div>
              </div>

              <div className="flex flex-col">
                <div className="text-sm text-trading-text-muted">24h 저가</div>
                <div className="text-sm font-mono text-trading-text-primary">
                  {formatPrice(statistics.low_price_24h)}
                </div>
              </div>

              <div className="flex flex-col">
                <div className="text-sm text-trading-text-muted">24h 거래량</div>
                <div className="text-sm font-mono text-trading-text-primary">
                  {statistics.volume_24h.toFixed(2)} BTC
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Right side controls */}
        <div className="flex items-center space-x-4">
          {/* Balance Display */}
          <div className="flex items-center space-x-4 bg-trading-bg-tertiary rounded-lg px-4 py-2">
            <Wallet size={18} className="text-trading-text-muted" />
            <div className="flex flex-col text-right">
              <div className="text-xs text-trading-text-muted">KRW 잔고</div>
              <div className="text-sm font-mono text-trading-text-primary">
                {new Intl.NumberFormat('ko-KR').format(balance.KRW || 0)}
              </div>
            </div>
            <div className="flex flex-col text-right">
              <div className="text-xs text-trading-text-muted">BTC 잔고</div>
              <div className="text-sm font-mono text-trading-text-primary">
                {(balance.BTC || 0).toFixed(6)}
              </div>
            </div>
          </div>

          {/* Status Indicator */}
          <div className="flex items-center space-x-2">
            <Activity size={16} className="text-trading-green animate-pulse" />
            <span className="text-sm text-trading-text-muted">Live</span>
          </div>

          {/* Action buttons */}
          <div className="flex items-center space-x-2">
            <button className="p-2 rounded-lg bg-trading-bg-tertiary hover:bg-trading-bg-hover transition-colors">
              <Bell size={18} className="text-trading-text-muted" />
            </button>
            <button className="p-2 rounded-lg bg-trading-bg-tertiary hover:bg-trading-bg-hover transition-colors">
              <Settings size={18} className="text-trading-text-muted" />
            </button>
            <button className="p-2 rounded-lg bg-trading-bg-tertiary hover:bg-trading-bg-hover transition-colors">
              <User size={18} className="text-trading-text-muted" />
            </button>
          </div>
        </div>
      </div>
    </header>
  );
};

export default Header;