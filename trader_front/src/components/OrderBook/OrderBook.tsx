import React, { useMemo } from 'react';
import { OrderBook as OrderBookType, OrderBookLevel } from '../../types/trading';

interface OrderBookProps {
  orderBook: OrderBookType | null;
  onPriceClick?: (price: number) => void;
}

const OrderBook: React.FC<OrderBookProps> = ({ orderBook, onPriceClick }) => {
  const { formattedAsks, formattedBids } = useMemo(() => {
    if (!orderBook) {
      return { formattedAsks: [], formattedBids: [], maxVolume: 0 };
    }

    // Sort and limit to top 10 levels
    const asks = [...orderBook.asks]
      .sort((a, b) => a.price - b.price)
      .slice(0, 10);

    const bids = [...orderBook.bids]
      .sort((a, b) => b.price - a.price)
      .slice(0, 10);

    // Calculate max volume for bar chart visualization
    const allVolumes = [...asks, ...bids].map(level => level.volume);
    const maxVol = Math.max(...allVolumes, 1);

    // Add cumulative volumes
    let cumulativeBidVolume = 0;
    const formattedBids = bids.map(level => {
      cumulativeBidVolume += level.volume;
      return {
        ...level,
        cumulative: cumulativeBidVolume,
        percentage: (level.volume / maxVol) * 100
      };
    });

    let cumulativeAskVolume = 0;
    const formattedAsks = asks.reverse().map(level => {
      cumulativeAskVolume += level.volume;
      return {
        ...level,
        cumulative: cumulativeAskVolume,
        percentage: (level.volume / maxVol) * 100
      };
    });

    return { formattedAsks, formattedBids };
  }, [orderBook]);

  const formatPrice = (price: number) => {
    return new Intl.NumberFormat('ko-KR', {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(price);
  };

  const formatVolume = (volume: number) => {
    return volume.toFixed(4);
  };

  if (!orderBook) {
    return (
      <div className="bg-trading-bg-secondary rounded-lg border border-trading-border p-4">
        <div className="text-center text-trading-text-muted">
          주문서 데이터를 불러오는 중...
        </div>
      </div>
    );
  }

  return (
    <div className="bg-trading-bg-secondary rounded-lg border border-trading-border">
      <div className="p-4 border-b border-trading-border">
        <h3 className="text-lg font-semibold text-trading-text-primary">주문서</h3>
        <div className="text-sm text-trading-text-muted">{orderBook.symbol}</div>
      </div>

      <div className="p-2">
        {/* Header */}
        <div className="grid grid-cols-3 gap-2 mb-2 text-xs text-trading-text-muted font-mono">
          <div className="text-right">가격(KRW)</div>
          <div className="text-right">수량</div>
          <div className="text-right">누적</div>
        </div>

        {/* Asks (매도) */}
        <div className="space-y-0.5 mb-3">
          {formattedAsks.map((level, index) => (
            <OrderBookRow
              key={`ask-${level.price}`}
              level={level}
              side="ask"
              onClick={() => onPriceClick?.(level.price)}
            />
          ))}
        </div>

        {/* Spread */}
        {orderBook.bids.length > 0 && orderBook.asks.length > 0 && (
          <div className="py-2 border-y border-trading-border my-2">
            <div className="text-center">
              <div className="text-sm font-mono text-trading-text-primary">
                {formatPrice(orderBook.asks[0]?.price - orderBook.bids[0]?.price || 0)}
              </div>
              <div className="text-xs text-trading-text-muted">스프레드</div>
            </div>
          </div>
        )}

        {/* Bids (매수) */}
        <div className="space-y-0.5">
          {formattedBids.map((level, index) => (
            <OrderBookRow
              key={`bid-${level.price}`}
              level={level}
              side="bid"
              onClick={() => onPriceClick?.(level.price)}
            />
          ))}
        </div>
      </div>
    </div>
  );
};

interface OrderBookRowProps {
  level: OrderBookLevel & { percentage: number; cumulative: number };
  side: 'bid' | 'ask';
  onClick: () => void;
}

const OrderBookRow: React.FC<OrderBookRowProps> = ({ level, side, onClick }) => {
  const formatPrice = (price: number) => {
    return new Intl.NumberFormat('ko-KR', {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(price);
  };

  const formatVolume = (volume: number) => {
    return volume.toFixed(4);
  };

  return (
    <div
      className="relative grid grid-cols-3 gap-2 text-xs font-mono cursor-pointer hover:bg-trading-bg-hover py-0.5 px-1 rounded"
      onClick={onClick}
    >
      {/* Background bar */}
      <div
        className={`absolute inset-0 rounded ${
          side === 'bid'
            ? 'bg-trading-green-bg'
            : 'bg-trading-red-bg'
        }`}
        style={{ width: `${level.percentage}%` }}
      />

      {/* Content */}
      <div className={`text-right relative z-10 ${
        side === 'bid' ? 'text-trading-green' : 'text-trading-red'
      }`}>
        {formatPrice(level.price)}
      </div>
      <div className="text-right relative z-10 text-trading-text-primary">
        {formatVolume(level.volume)}
      </div>
      <div className="text-right relative z-10 text-trading-text-secondary">
        {formatVolume(level.cumulative)}
      </div>
    </div>
  );
};

export default OrderBook;