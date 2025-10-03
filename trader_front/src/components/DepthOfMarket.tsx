import React, { useEffect, useState, useMemo } from 'react';
import styled from 'styled-components';

interface OrderBookLevel {
  price: number;
  quantity: number;
  cumulative: number;
  percentage: number;
}

interface DepthOfMarketProps {
  symbol: string;
  bids: [number, number][];
  asks: [number, number][];
  depth?: number; // 표시할 호가 단계 (기본 15)
  onPriceClick?: (price: number, side: 'bid' | 'ask') => void;
}

const DepthOfMarket: React.FC<DepthOfMarketProps> = ({
  symbol,
  bids,
  asks,
  depth = 15,
  onPriceClick,
}) => {
  const [processedBids, setProcessedBids] = useState<OrderBookLevel[]>([]);
  const [processedAsks, setProcessedAsks] = useState<OrderBookLevel[]>([]);
  const [spread, setSpread] = useState<number>(0);
  const [spreadPercent, setSpreadPercent] = useState<number>(0);
  const [midPrice, setMidPrice] = useState<number>(0);

  const { bestBid, bestAsk } = useMemo(() => {
    const sortedBids = [...bids].sort((a, b) => b[0] - a[0]);
    const sortedAsks = [...asks].sort((a, b) => a[0] - b[0]);
    
    return {
      bestBid: sortedBids[0]?.[0] || 0,
      bestAsk: sortedAsks[0]?.[0] || 0,
    };
  }, [bids, asks]);

  useEffect(() => {
    // 매수 호가 처리 (높은 가격순으로 정렬)
    const sortedBids = [...bids]
      .sort((a, b) => b[0] - a[0])
      .slice(0, depth);

    // 매도 호가 처리 (낮은 가격순으로 정렬)
    const sortedAsks = [...asks]
      .sort((a, b) => a[0] - b[0])
      .slice(0, depth);

    // 누적 수량 계산 (매수는 위에서 아래로, 매도는 아래에서 위로)
    let bidCumulative = 0;
    const bidsWithCumulative = sortedBids.map(([price, quantity]) => {
      bidCumulative += quantity;
      return {
        price,
        quantity,
        cumulative: bidCumulative,
        percentage: 0, // 나중에 계산
      };
    });

    let askCumulative = 0;
    const asksWithCumulative = sortedAsks.map(([price, quantity]) => {
      askCumulative += quantity;
      return {
        price,
        quantity,
        cumulative: askCumulative,
        percentage: 0,
      };
    });

    // 최대 누적량 계산 (전체 시장 깊이 기준)
    const maxCumulative = Math.max(
      ...bidsWithCumulative.map(b => b.cumulative),
      ...asksWithCumulative.map(a => a.cumulative),
      1
    );

    // 퍼센티지 계산 (전체 시장 깊이 대비)
    bidsWithCumulative.forEach(bid => {
      bid.percentage = (bid.cumulative / maxCumulative) * 100;
    });
    asksWithCumulative.forEach(ask => {
      ask.percentage = (ask.cumulative / maxCumulative) * 100;
    });

    setProcessedBids(bidsWithCumulative);
    setProcessedAsks(asksWithCumulative);

    // 스프레드 및 중간가 계산
    if (sortedBids.length > 0 && sortedAsks.length > 0) {
      const bestBidPrice = sortedBids[0][0];
      const bestAskPrice = sortedAsks[0][0];
      const spreadValue = bestAskPrice - bestBidPrice;
      const midPriceValue = (bestBidPrice + bestAskPrice) / 2;

      setSpread(spreadValue);
      setMidPrice(midPriceValue);
      setSpreadPercent((spreadValue / midPriceValue) * 100);
    }
  }, [bids, asks, depth]);

  const formatPrice = (price: number) => {
    return price.toLocaleString('ko-KR', {
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    });
  };

  const formatQuantity = (quantity: number) => {
    if (quantity >= 1) {
      return quantity.toLocaleString('ko-KR', {
        minimumFractionDigits: 0,
        maximumFractionDigits: 2,
      });
    } else {
      return quantity.toFixed(6);
    }
  };

  return (
    <Container>
      <Header>
        <Title>📊 호가창 (Depth of Market)</Title>
        <SymbolBadge>{symbol}</SymbolBadge>
      </Header>

      {/* 시장 정보 요약 */}
      <MarketInfo>
        <MarketRow>
          <MarketLabel>최고 매수가</MarketLabel>
          <MarketValue type="bid">{formatPrice(bestBid)}</MarketValue>
        </MarketRow>
        <MarketRow>
          <MarketLabel>최저 매도가</MarketLabel>
          <MarketValue type="ask">{formatPrice(bestAsk)}</MarketValue>
        </MarketRow>
        <MarketRow>
          <MarketLabel>중간가</MarketLabel>
          <MarketValue type="neutral">{formatPrice(midPrice)}</MarketValue>
        </MarketRow>
        <MarketRow>
          <MarketLabel>스프레드</MarketLabel>
          <MarketValue type="spread">
            {formatPrice(spread)} ({spreadPercent.toFixed(3)}%)
          </MarketValue>
        </MarketRow>
      </MarketInfo>

      {/* 테이블 헤더 */}
      <TableHeader>
        <HeaderCell align="right">수량 (BTC)</HeaderCell>
        <HeaderCell align="center">가격 (KRW)</HeaderCell>
        <HeaderCell align="left">누적 (BTC)</HeaderCell>
      </TableHeader>

      {/* 매도 호가 (높은 가격부터) */}
      <AskSection>
        {processedAsks.map((ask, index) => (
          <OrderRow 
            key={`ask-${ask.price}`} 
            type="ask"
            onClick={() => onPriceClick?.(ask.price, 'ask')}
          >
            <DepthBar type="ask" width={ask.percentage} />
            <QuantityCell type="ask" align="right">
              {formatQuantity(ask.quantity)}
            </QuantityCell>
            <PriceCell type="ask" align="center">
              {formatPrice(ask.price)}
            </PriceCell>
            <CumulativeCell type="ask" align="left">
              {formatQuantity(ask.cumulative)}
            </CumulativeCell>
          </OrderRow>
        ))}
      </AskSection>

      {/* 스프레드 구분선 */}
      <SpreadDivider>
        <SpreadLine />
        <SpreadText>
          스프레드: {formatPrice(spread)} ({spreadPercent.toFixed(3)}%)
        </SpreadText>
      </SpreadDivider>

      {/* 매수 호가 (높은 가격부터) */}
      <BidSection>
        {processedBids.map((bid, index) => (
          <OrderRow 
            key={`bid-${bid.price}`} 
            type="bid"
            onClick={() => onPriceClick?.(bid.price, 'bid')}
          >
            <DepthBar type="bid" width={bid.percentage} />
            <QuantityCell type="bid" align="right">
              {formatQuantity(bid.quantity)}
            </QuantityCell>
            <PriceCell type="bid" align="center">
              {formatPrice(bid.price)}
            </PriceCell>
            <CumulativeCell type="bid" align="left">
              {formatQuantity(bid.cumulative)}
            </CumulativeCell>
          </OrderRow>
        ))}
      </BidSection>

      {/* 범례 및 정보 */}
      <Legend>
        <LegendItem>
          <BidIndicator />
          <span>매수 누적량</span>
        </LegendItem>
        <LegendItem>
          <AskIndicator />
          <span>매도 누적량</span>
        </LegendItem>
        <LegendItem>
          <ClickHint>💡 가격을 클릭하여 주문</ClickHint>
        </LegendItem>
      </Legend>
    </Container>
  );
};

export default DepthOfMarket;

// Styled Components
const Container = styled.div`
  background: #ffffff;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  padding: 16px;
  color: #1a1a1a;
  font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
  max-height: 600px;
  overflow-y: auto;
  overflow-x: hidden;
  box-shadow: 0 2px 4px rgba(0,0,0,0.1);
`;

const Header = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid #e9ecef;
`;

const Title = styled.h3`
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #1a1a1a;
`;

const SymbolBadge = styled.span`
  background: #e9ecef;
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 600;
  color: #007bff;
`;

const MarketInfo = styled.div`
  background: #f8f9fa;
  border-radius: 6px;
  padding: 12px;
  margin-bottom: 8px;
  border: 1px solid #e9ecef;
`;

const MarketRow = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;

  &:last-child {
    margin-bottom: 0;
  }
`;

const MarketLabel = styled.span`
  font-size: 12px;
  color: #6c757d;
  font-weight: 500;
`;

const MarketValue = styled.span<{ type: 'bid' | 'ask' | 'neutral' | 'spread' }>`
  font-size: 13px;
  font-weight: 600;
  color: ${props => {
    switch (props.type) {
      case 'bid': return '#28a745';
      case 'ask': return '#dc3545';
      case 'spread': return '#ffc107';
      default: return '#1a1a1a';
    }
  }};
`;

const TableHeader = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  padding: 8px 0;
  border-bottom: 1px solid #e9ecef;
  margin-bottom: 4px;
`;

const HeaderCell = styled.div<{ align: 'left' | 'center' | 'right' }>`
  font-size: 11px;
  color: #6c757d;
  text-align: ${props => props.align};
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const AskSection = styled.div`
  display: flex;
  flex-direction: column;
  margin-bottom: 8px;
`;

const BidSection = styled.div`
  display: flex;
  flex-direction: column;
`;

const OrderRow = styled.div<{ type: 'bid' | 'ask' }>`
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  padding: 6px 8px;
  font-size: 12px;
  transition: all 0.2s ease;
  border-radius: 4px;
  margin-bottom: 1px;

  &:hover {
    background: rgba(0, 0, 0, 0.05);
    cursor: pointer;
    transform: translateX(${props => props.type === 'bid' ? '2px' : '-2px'});
  }
`;

const DepthBar = styled.div<{ type: 'bid' | 'ask'; width: number }>`
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: ${props => props.width}%;
  background: ${props =>
    props.type === 'bid'
      ? 'linear-gradient(90deg, rgba(40, 167, 69, 0.2) 0%, rgba(40, 167, 69, 0.05) 100%)'
      : 'linear-gradient(90deg, rgba(220, 53, 69, 0.2) 0%, rgba(220, 53, 69, 0.05) 100%)'};
  border-radius: 4px;
  transition: width 0.3s ease;
  pointer-events: none;
`;

const QuantityCell = styled.div<{ align: 'left' | 'right'; type: 'bid' | 'ask' }>`
  text-align: ${props => props.align};
  color: ${props => (props.type === 'bid' ? '#28a745' : '#dc3545')};
  position: relative;
  z-index: 1;
  font-weight: 500;
`;

const PriceCell = styled.div<{ align: 'center'; type: 'bid' | 'ask' }>`
  text-align: center;
  color: ${props => (props.type === 'bid' ? '#28a745' : '#dc3545')};
  font-weight: 600;
  position: relative;
  z-index: 1;
`;

const CumulativeCell = styled.div<{ align: 'left'; type: 'bid' | 'ask' }>`
  text-align: ${props => props.align};
  color: #6c757d;
  position: relative;
  z-index: 1;
  font-weight: 400;
  font-size: 11px;
`;

const SpreadDivider = styled.div`
  padding: 12px 0;
  margin: 4px 0;
  text-align: center;
  position: relative;
`;

const SpreadLine = styled.div`
  height: 2px;
  background: linear-gradient(
    90deg,
    #28a745 0%,
    #ffc107 50%,
    #dc3545 100%
  );
  border-radius: 1px;
  margin-bottom: 4px;
`;

const SpreadText = styled.div`
  font-size: 11px;
  color: #ffc107;
  font-weight: 500;
`;

const Legend = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
  padding-top: 6px;
  border-top: 1px solid #e9ecef;
`;

const LegendItem = styled.div`
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: #6c757d;
`;

const BidIndicator = styled.div`
  width: 12px;
  height: 12px;
  background: rgba(40, 167, 69, 0.5);
  border-radius: 2px;
`;

const AskIndicator = styled.div`
  width: 12px;
  height: 12px;
  background: rgba(220, 53, 69, 0.5);
  border-radius: 2px;
`;

const ClickHint = styled.span`
  color: #007bff;
  font-style: italic;
`;
