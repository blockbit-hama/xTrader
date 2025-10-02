import React, { useMemo } from 'react';
import { OrderBook as OrderBookType } from '../../types/trading';
import DepthOfMarket from '../DepthOfMarket';

interface OrderBookTabProps {
  orderBook: OrderBookType | null;
  loading: boolean;
  onPriceClick?: (price: number, side: 'bid' | 'ask') => void;
}

const OrderBookTab: React.FC<OrderBookTabProps> = ({ orderBook, loading, onPriceClick }) => {
  // 임시 mock 데이터 (백엔드 API 문제 해결 시 제거 예정)
  const generateMockOrderBook = () => {
    const basePrice = 55000000;
    const mockBids: [number, number][] = [];
    const mockAsks: [number, number][] = [];

    for (let i = 0; i < 15; i++) {
      mockBids.push([basePrice - (i * 50000) - 50000, Math.random() * 10 + 0.5]);
      mockAsks.push([basePrice + (i * 50000) + 50000, Math.random() * 10 + 0.5]);
    }

    return { bids: mockBids, asks: mockAsks };
  };

  const mockData = generateMockOrderBook();

  const containerStyle: React.CSSProperties = {
    height: '100%',
    overflow: 'hidden',
    display: 'flex',
    flexDirection: 'column'
  };

  // 백엔드 데이터가 있으면 사용, 없으면 mock 데이터 사용
  const displayBids = orderBook && orderBook.bids.length > 0
    ? orderBook.bids.map(bid => [bid.price, bid.volume] as [number, number])
    : mockData.bids;

  const displayAsks = orderBook && orderBook.asks.length > 0
    ? orderBook.asks.map(ask => [ask.price, ask.volume] as [number, number])
    : mockData.asks;

  return (
    <div style={containerStyle}>
      <DepthOfMarket
        symbol="BTC-KRW"
        bids={displayBids}
        asks={displayAsks}
        depth={15}
        onPriceClick={onPriceClick}
      />
    </div>
  );
};

export default OrderBookTab;