import React, { useState, useMemo, useRef, useEffect } from 'react';
import styled from 'styled-components';
import { OrderBook, MarketStatistics, Execution } from '../../types/trading';
import TradingViewChart from '../TradingViewChart';
import TechnicalIndicatorControls from '../TechnicalIndicatorControls';

interface AdvancedDOMTabProps {
  orderBook: OrderBook | null;
  statistics: MarketStatistics | null;
  executions: Execution[];
  loading: boolean;
  balance: { BTC: number; KRW: number };
  onSubmitOrder: (orderData: any) => Promise<any>;
  onPriceClick?: (price: number, side: 'bid' | 'ask') => void;
  candles?: any[];
  showMockData?: boolean;
  mockCandles?: any[];
}

interface OrderLevel {
  price: number;
  quantity: number;
  cumulative: number;
  percentage: number;
  orders: number;
}

interface MarketDepthPoint {
  price: number;
  bidVolume: number;
  askVolume: number;
  totalVolume: number;
}

const AdvancedDOMTab: React.FC<AdvancedDOMTabProps> = ({
  orderBook,
  statistics,
  executions,
  loading,
  balance,
  onSubmitOrder,
  onPriceClick,
  candles = [],
  showMockData = true,
  mockCandles = [],
}) => {
  const [selectedPrice, setSelectedPrice] = useState<number | null>(null);
  const [selectedSide, setSelectedSide] = useState<'bid' | 'ask' | null>(null);
  const [orderQuantity, setOrderQuantity] = useState<string>('');
  const [orderType, setOrderType] = useState<'Market' | 'Limit'>('Limit');
  const [enabledIndicators, setEnabledIndicators] = useState<Set<string>>(new Set());
  const [showVolumeProfile, setShowVolumeProfile] = useState(true);
  const [showHeatmap, setShowHeatmap] = useState(true);
  const [depthLevels, setDepthLevels] = useState(20);
  const [priceIncrement, setPriceIncrement] = useState(1000);
  
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Mock data for demonstration
  const mockOrderBook = useMemo(() => {
    if (orderBook) return orderBook;
    
    const basePrice = statistics?.last_price || 55000000;
    const bids: Array<{price: number; volume: number}> = [];
    const asks: Array<{price: number; volume: number}> = [];

    // Generate realistic order book data with more volume near the current price
    for (let i = 0; i < 25; i++) {
      const distance = i * priceIncrement;
      const volumeMultiplier = Math.max(0.1, 1 - (distance / (basePrice * 0.01))); // More volume near current price
      bids.push({
        price: basePrice - distance,
        volume: (Math.random() * 3 + 0.5) * volumeMultiplier,
      });
    }

    for (let i = 0; i < 25; i++) {
      const distance = i * priceIncrement;
      const volumeMultiplier = Math.max(0.1, 1 - (distance / (basePrice * 0.01))); // More volume near current price
      asks.push({
        price: basePrice + distance,
        volume: (Math.random() * 3 + 0.5) * volumeMultiplier,
      });
    }

    return {
      symbol: 'BTC-KRW',
      bids: bids.sort((a, b) => b.price - a.price),
      asks: asks.sort((a, b) => a.price - b.price),
    };
  }, [orderBook, priceIncrement, statistics]);

  const processedData = useMemo(() => {
    const bids = mockOrderBook.bids.slice(0, depthLevels);
    const asks = mockOrderBook.asks.slice(0, depthLevels);

    // Calculate cumulative volumes
    let bidCumulative = 0;
    const processedBids: OrderLevel[] = bids.map(bid => {
      bidCumulative += bid.volume;
      return {
        price: bid.price,
        quantity: bid.volume,
        cumulative: bidCumulative,
        percentage: 0,
        orders: Math.floor(Math.random() * 10) + 1,
      };
    });

    let askCumulative = 0;
    const processedAsks: OrderLevel[] = asks.map(ask => {
      askCumulative += ask.volume;
      return {
        price: ask.price,
        quantity: ask.volume,
        cumulative: askCumulative,
        percentage: 0,
        orders: Math.floor(Math.random() * 10) + 1,
      };
    });

    // Calculate percentages
    const maxCumulative = Math.max(
      ...processedBids.map(b => b.cumulative),
      ...processedAsks.map(a => a.cumulative),
      1
    );

    processedBids.forEach(bid => {
      bid.percentage = (bid.cumulative / maxCumulative) * 100;
    });
    processedAsks.forEach(ask => {
      ask.percentage = (ask.cumulative / maxCumulative) * 100;
    });

    return { processedBids, processedAsks };
  }, [mockOrderBook, depthLevels]);

  const marketDepthData = useMemo(() => {
    const { processedBids, processedAsks } = processedData;
    const depthPoints: MarketDepthPoint[] = [];

    // Create depth chart data
    const allPrices = [
      ...processedBids.map(b => b.price),
      ...processedAsks.map(a => a.price)
    ].sort((a, b) => a - b);

    allPrices.forEach(price => {
      const bidLevel = processedBids.find(b => b.price === price);
      const askLevel = processedAsks.find(a => a.price === price);
      
      depthPoints.push({
        price,
        bidVolume: bidLevel?.cumulative || 0,
        askVolume: askLevel?.cumulative || 0,
        totalVolume: (bidLevel?.cumulative || 0) + (askLevel?.cumulative || 0),
      });
    });

    return depthPoints;
  }, [processedData]);

  // Draw market depth chart
  useEffect(() => {
    if (!showVolumeProfile || !canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Set up chart dimensions
    const padding = 40;
    const chartWidth = width - padding * 2;
    const chartHeight = height - padding * 2;

    // Find min/max values
    const minPrice = Math.min(...marketDepthData.map(d => d.price));
    const maxPrice = Math.max(...marketDepthData.map(d => d.price));
    const maxVolume = Math.max(...marketDepthData.map(d => d.totalVolume));

    // Draw axes
    ctx.strokeStyle = '#2b3139';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(padding, padding);
    ctx.lineTo(padding, height - padding);
    ctx.lineTo(width - padding, height - padding);
    ctx.stroke();

    // Draw bid volume bars
    ctx.fillStyle = 'rgba(2, 192, 118, 0.3)';
    marketDepthData.forEach((point, index) => {
      if (point.bidVolume > 0) {
        const x = padding + (point.price - minPrice) / (maxPrice - minPrice) * chartWidth;
        const barHeight = (point.bidVolume / maxVolume) * chartHeight;
        ctx.fillRect(x - 2, height - padding - barHeight, 4, barHeight);
      }
    });

    // Draw ask volume bars
    ctx.fillStyle = 'rgba(248, 73, 96, 0.3)';
    marketDepthData.forEach((point, index) => {
      if (point.askVolume > 0) {
        const x = padding + (point.price - minPrice) / (maxPrice - minPrice) * chartWidth;
        const barHeight = (point.askVolume / maxVolume) * chartHeight;
        ctx.fillRect(x - 2, height - padding - barHeight, 4, barHeight);
      }
    });

    // Draw price labels
    ctx.fillStyle = '#848e9c';
    ctx.font = '10px monospace';
    ctx.textAlign = 'center';
    for (let i = 0; i < 5; i++) {
      const price = minPrice + (maxPrice - minPrice) * (i / 4);
      const x = padding + (price - minPrice) / (maxPrice - minPrice) * chartWidth;
      ctx.fillText(formatPrice(price), x, height - padding + 15);
    }

  }, [marketDepthData, showVolumeProfile]);

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

  const handlePriceClick = (price: number, side: 'bid' | 'ask') => {
    setSelectedPrice(price);
    setSelectedSide(side);
    onPriceClick?.(price, side);
  };

  const handleOrderSubmit = async () => {
    if (!selectedPrice || !orderQuantity || !selectedSide) return;

    try {
      const orderData = {
        symbol: 'BTC-KRW',
        side: selectedSide === 'bid' ? 'Buy' : 'Sell',
        orderType: orderType,
        price: orderType === 'Limit' ? selectedPrice : undefined,
        quantity: parseFloat(orderQuantity),
        client_id: 'test_user_001', // 테스트 사용자 ID 추가
      };

      console.log('🚀 주문 제출:', orderData);
      const result = await onSubmitOrder(orderData);
      console.log('✅ 주문 성공:', result);
      
      setOrderQuantity('');
      setSelectedPrice(null);
      setSelectedSide(null);
      
      alert(`주문이 성공적으로 제출되었습니다!\n주문 ID: ${result.order_id}`);
    } catch (error) {
      console.error('❌ 주문 실패:', error);
      alert(`주문 실패: ${error instanceof Error ? error.message : '알 수 없는 오류'}`);
    }
  };

  const handleToggleIndicator = (indicator: string, enabled: boolean) => {
    const newIndicators = new Set(enabledIndicators);
    if (enabled) {
      newIndicators.add(indicator);
    } else {
      newIndicators.delete(indicator);
    }
    setEnabledIndicators(newIndicators);
  };

  // Always show the component since we have mock data available
  // No loading state needed as we can always display demo data
  
  console.log('🎯 AdvancedDOMTab 렌더링:', {
    loading,
    hasOrderBook: !!orderBook,
    hasStatistics: !!statistics,
    executionsCount: executions.length,
    mockOrderBook: mockOrderBook.symbol,
    candlesCount: candles.length,
    mockCandlesCount: mockCandles.length
  });

  return (
    <Container>
      {/* Header with Controls */}
      <Header>
        <Title>🎯 고급 Depth of Market</Title>
        <DataStatus>
          {orderBook ? (
            <StatusBadge type="live">🟢 실시간 데이터</StatusBadge>
          ) : (
            <StatusBadge type="mock">🟡 데모 데이터</StatusBadge>
          )}
        </DataStatus>
        <Controls>
          <ControlGroup>
            <Label>깊이:</Label>
            <Select value={depthLevels} onChange={(e) => setDepthLevels(Number(e.target.value))}>
              <option value={10}>10단계</option>
              <option value={15}>15단계</option>
              <option value={20}>20단계</option>
              <option value={25}>25단계</option>
            </Select>
          </ControlGroup>
          <ControlGroup>
            <Label>가격 간격:</Label>
            <Select value={priceIncrement} onChange={(e) => setPriceIncrement(Number(e.target.value))}>
              <option value={500}>500원</option>
              <option value={1000}>1,000원</option>
              <option value={5000}>5,000원</option>
              <option value={10000}>10,000원</option>
            </Select>
          </ControlGroup>
          <ControlGroup>
            <CheckboxLabel>
              <input
                type="checkbox"
                checked={showVolumeProfile}
                onChange={(e) => setShowVolumeProfile(e.target.checked)}
              />
              볼륨 프로필
            </CheckboxLabel>
          </ControlGroup>
          <ControlGroup>
            <CheckboxLabel>
              <input
                type="checkbox"
                checked={showHeatmap}
                onChange={(e) => setShowHeatmap(e.target.checked)}
              />
              히트맵
            </CheckboxLabel>
          </ControlGroup>
        </Controls>
      </Header>

      <MainContent>
        {/* Left Panel - Chart */}
        <LeftPanel>
          <ChartSection>
            <SectionHeader>
              <SectionTitle>📈 실시간 차트</SectionTitle>
            </SectionHeader>
            
            {/* 기술적 지표 컨트롤 */}
            <TechnicalIndicatorControls
              onToggleIndicator={handleToggleIndicator}
              enabledIndicators={enabledIndicators}
            />
            
            <ChartContainer>
              {candles.length > 0 || mockCandles.length > 0 ? (
                <TradingViewChart
                  data={candles.length > 0 ? candles : mockCandles}
                  width={750}
                  height={600}
                  enabledIndicators={enabledIndicators}
                />
              ) : (
                <ChartPlaceholder>
                  {loading ? '차트 데이터 로딩 중...' : '차트 데이터를 불러올 수 없습니다.'}
                </ChartPlaceholder>
              )}
            </ChartContainer>
          </ChartSection>
        </LeftPanel>

        {/* Center Panel - Order Book */}
        <CenterPanel>
          <OrderBookSection>
            <SectionHeader>
              <SectionTitle>📊 실시간 호가창</SectionTitle>
              <MarketInfo>
                <InfoItem>
                  <InfoLabel>최고 매수가:</InfoLabel>
                  <InfoValue type="bid">{formatPrice(mockOrderBook.bids[0]?.price || 0)}</InfoValue>
                </InfoItem>
                <InfoItem>
                  <InfoLabel>최저 매도가:</InfoLabel>
                  <InfoValue type="ask">{formatPrice(mockOrderBook.asks[0]?.price || 0)}</InfoValue>
                </InfoItem>
                <InfoItem>
                  <InfoLabel>스프레드:</InfoLabel>
                  <InfoValue type="spread">
                    {formatPrice((mockOrderBook.asks[0]?.price || 0) - (mockOrderBook.bids[0]?.price || 0))}
                  </InfoValue>
                </InfoItem>
              </MarketInfo>
            </SectionHeader>

            <OrderBookContent>
              {/* Traditional DOM Header */}
              <DOMHeader>
                <HeaderCell align="right">매수량</HeaderCell>
                <HeaderCell align="center">가격</HeaderCell>
                <HeaderCell align="left">매도량</HeaderCell>
              </DOMHeader>

              {/* DOM Rows - Traditional Layout with Price Ordering */}
              <DOMRows>
                {/* Create price-ordered rows */}
                {(() => {
                  const rows: React.ReactElement[] = [];
                  
                  // Create a map of all prices with their data
                  const priceMap = new Map<number, { ask: OrderLevel | null; bid: OrderLevel | null }>();
                  
                  // Add asks (sorted by price descending - highest first)
                  processedData.processedAsks.forEach(ask => {
                    priceMap.set(ask.price, { ask, bid: null });
                  });
                  
                  // Add bids (sorted by price descending - highest first)
                  processedData.processedBids.forEach(bid => {
                    const existing = priceMap.get(bid.price);
                    if (existing) {
                      existing.bid = bid;
                    } else {
                      priceMap.set(bid.price, { ask: null, bid });
                    }
                  });
                  
                  // Sort prices in descending order (highest to lowest)
                  const sortedPrices = Array.from(priceMap.keys()).sort((a, b) => b - a);
                  
                  // Create rows for each price level
                  sortedPrices.forEach((price, index) => {
                    const priceData = priceMap.get(price);
                    if (!priceData) return;
                    
                    const { ask, bid } = priceData;
                    
                    rows.push(
                      <DOMRow key={`row-${price}`}>
                        {/* Bid Side */}
                        <BidCell>
                          {bid ? (
                            <BidRow
                              onClick={() => handlePriceClick(bid.price, 'bid')}
                              selected={selectedPrice === bid.price && selectedSide === 'bid'}
                            >
                              <BidDepthBar width={bid.percentage} />
                              <BidQuantity>{formatQuantity(bid.quantity)}</BidQuantity>
                            </BidRow>
                          ) : (
                            <EmptyCell />
                          )}
                        </BidCell>
                        
                        {/* Price Column */}
                        <PriceCell>
                          {ask && (
                            <AskPrice
                              onClick={() => handlePriceClick(ask.price, 'ask')}
                              selected={selectedPrice === ask.price && selectedSide === 'ask'}
                            >
                              {formatPrice(ask.price)}
                            </AskPrice>
                          )}
                          {bid && (
                            <BidPrice
                              onClick={() => handlePriceClick(bid.price, 'bid')}
                              selected={selectedPrice === bid.price && selectedSide === 'bid'}
                            >
                              {formatPrice(bid.price)}
                            </BidPrice>
                          )}
                        </PriceCell>
                        
                        {/* Ask Side */}
                        <AskCell>
                          {ask ? (
                            <AskRow
                              onClick={() => handlePriceClick(ask.price, 'ask')}
                              selected={selectedPrice === ask.price && selectedSide === 'ask'}
                            >
                              <AskDepthBar width={ask.percentage} />
                              <AskQuantity>{formatQuantity(ask.quantity)}</AskQuantity>
                            </AskRow>
                          ) : (
                            <EmptyCell />
                          )}
                        </AskCell>
                      </DOMRow>
                    );
                  });
                  
                  return rows;
                })()}
              </DOMRows>
            </OrderBookContent>
          </OrderBookSection>
        </CenterPanel>

        {/* Right Panel - Quick Order & Executions */}
        <RightPanel>
          {/* Quick Order Section */}
          <OrderSection>
            <SectionTitle>⚡ 빠른 주문</SectionTitle>
            {selectedPrice && selectedSide ? (
              <OrderForm>
                <OrderInfo>
                  <OrderLabel>선택된 가격:</OrderLabel>
                  <OrderValue type={selectedSide}>
                    {formatPrice(selectedPrice)} ({selectedSide === 'bid' ? '매수' : '매도'})
                  </OrderValue>
                </OrderInfo>
                
                <FormGroup>
                  <Label>주문 타입:</Label>
                  <Select value={orderType} onChange={(e) => setOrderType(e.target.value as 'Market' | 'Limit')}>
                    <option value="Limit">지정가</option>
                    <option value="Market">시장가</option>
                  </Select>
                </FormGroup>

                <FormGroup>
                  <Label>수량 (BTC):</Label>
                  <Input
                    type="number"
                    value={orderQuantity}
                    onChange={(e) => setOrderQuantity(e.target.value)}
                    placeholder="수량을 입력하세요"
                    step="0.000001"
                    min="0"
                  />
                </FormGroup>

                <FormGroup>
                  <Label>예상 총액:</Label>
                  <TotalAmount>
                    {orderQuantity && parseFloat(orderQuantity) > 0
                      ? `${formatPrice(parseFloat(orderQuantity) * selectedPrice)} KRW`
                      : '0 KRW'}
                  </TotalAmount>
                </FormGroup>

                <OrderButton
                  onClick={handleOrderSubmit}
                  disabled={!orderQuantity || parseFloat(orderQuantity) <= 0}
                  type={selectedSide}
                >
                  {selectedSide === 'bid' ? '💰 매수 주문' : '💸 매도 주문'}
                </OrderButton>
              </OrderForm>
            ) : (
              <NoSelection>
                💡 호가창에서 가격을 클릭하여 주문하세요
              </NoSelection>
            )}
          </OrderSection>

          {/* Recent Executions - Extended Version */}
          <ExecutionsSection>
            <SectionTitle>📋 최근 체결 (최대 50건)</SectionTitle>
            <ExecutionsList>
              {executions.length > 0 ? (
                executions.slice(0, 50).map((execution, index) => (
                  <ExecutionRow key={index}>
                    <ExecutionSide type={execution.side}>
                      {execution.side === 'Buy' ? '매수' : '매도'}
                    </ExecutionSide>
                    <ExecutionPrice>{formatPrice(execution.price)}</ExecutionPrice>
                    <ExecutionQuantity>{formatQuantity(execution.quantity)}</ExecutionQuantity>
                    <ExecutionTime>
                      {new Date(execution.timestamp * 1000).toLocaleTimeString()}
                    </ExecutionTime>
                  </ExecutionRow>
                ))
              ) : (
                // Mock executions for demonstration
                Array.from({ length: 50 }, (_, index) => {
                  const side = Math.random() > 0.5 ? 'Buy' : 'Sell';
                  const price = (statistics?.last_price || 55000000) + (Math.random() - 0.5) * 100000;
                  const quantity = Math.random() * 2 + 0.1;
                  const timestamp = Date.now() - index * 30000; // 30 seconds apart
                  
                  return (
                    <ExecutionRow key={`mock-${index}`}>
                      <ExecutionSide type={side}>
                        {side === 'Buy' ? '매수' : '매도'}
                      </ExecutionSide>
                      <ExecutionPrice>{formatPrice(price)}</ExecutionPrice>
                      <ExecutionQuantity>{formatQuantity(quantity)}</ExecutionQuantity>
                      <ExecutionTime>
                        {new Date(timestamp).toLocaleTimeString()}
                      </ExecutionTime>
                    </ExecutionRow>
                  );
                })
              )}
            </ExecutionsList>
          </ExecutionsSection>
        </RightPanel>
      </MainContent>
    </Container>
  );
};

export default AdvancedDOMTab;

// Styled Components
const Container = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #ffffff;
  color: #1a1a1a;
  font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Roboto Mono', monospace;
`;

const Header = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px;
  border-bottom: 1px solid #e9ecef;
  background: #f8f9fa;
  gap: 16px;
`;

const DataStatus = styled.div`
  display: flex;
  align-items: center;
`;

const StatusBadge = styled.span<{ type: 'live' | 'mock' }>`
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
  background: ${props => props.type === 'live' ? 'rgba(40, 167, 69, 0.2)' : 'rgba(255, 193, 7, 0.2)'};
  color: ${props => props.type === 'live' ? '#28a745' : '#ffc107'};
  border: 1px solid ${props => props.type === 'live' ? '#28a745' : '#ffc107'};
`;

const Title = styled.h2`
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: #1a1a1a;
`;

const Controls = styled.div`
  display: flex;
  gap: 16px;
  align-items: center;
`;

const ControlGroup = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
`;

const Label = styled.span`
  font-size: 12px;
  color: #6c757d;
  font-weight: 500;
`;

const Select = styled.select`
  background: #e9ecef;
  border: 1px solid #007bff;
  border-radius: 4px;
  color: #1a1a1a;
  padding: 4px 8px;
  font-size: 12px;
  font-family: inherit;
`;

const CheckboxLabel = styled.label`
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #6c757d;
  cursor: pointer;
`;

const MainContent = styled.div`
  display: flex;
  flex: 1;
  overflow: hidden;
  gap: 16px;
`;

const LeftPanel = styled.div`
  flex: 2;
  min-width: 800px;
  padding: 16px;
  border-right: 1px solid #e9ecef;
  display: flex;
  flex-direction: column;
`;

const CenterPanel = styled.div`
  width: 350px;
  padding: 16px;
  border-right: 1px solid #e9ecef;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  flex-shrink: 0;
`;

const RightPanel = styled.div`
  width: 300px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
  flex-shrink: 0;
  
  /* Custom scrollbar styling */
  &::-webkit-scrollbar {
    width: 10px;
  }
  
  &::-webkit-scrollbar-track {
    background: #f8f9fa;
    border-radius: 5px;
  }
  
  &::-webkit-scrollbar-thumb {
    background: #007bff;
    border-radius: 5px;
  }
  
  &::-webkit-scrollbar-thumb:hover {
    background: #0056b3;
  }
`;

const ChartSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  overflow: hidden;
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
`;

const ChartContainer = styled.div`
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #ffffff;
  min-height: 300px;
`;

const ChartPlaceholder = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #6c757d;
  font-size: 14px;
`;

const OrderBookSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  overflow: hidden;
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0; /* Important for flex child to shrink */
`;

const OrderBookContent = styled.div`
  overflow-y: auto;
  flex: 1;
  
  /* Custom scrollbar styling */
  &::-webkit-scrollbar {
    width: 6px;
  }
  
  &::-webkit-scrollbar-track {
    background: #f8f9fa;
    border-radius: 3px;
  }
  
  &::-webkit-scrollbar-thumb {
    background: #007bff;
    border-radius: 3px;
  }
  
  &::-webkit-scrollbar-thumb:hover {
    background: #0056b3;
  }
`;

const SectionHeader = styled.div`
  padding: 12px 16px;
  border-bottom: 1px solid #e9ecef;
`;

const SectionTitle = styled.h3`
  margin: 0 0 8px 0;
  font-size: 14px;
  font-weight: 600;
  color: #1a1a1a;
`;

const MarketInfo = styled.div`
  display: flex;
  gap: 16px;
`;

const InfoItem = styled.div`
  display: flex;
  flex-direction: column;
  gap: 2px;
`;

const InfoLabel = styled.span`
  font-size: 10px;
  color: #6c757d;
`;

const InfoValue = styled.span<{ type: 'bid' | 'ask' | 'spread' }>`
  font-size: 12px;
  font-weight: 600;
  color: ${props => {
    switch (props.type) {
      case 'bid': return '#02c076';
      case 'ask': return '#f84960';
      case 'spread': return '#ffa500';
      default: return '#ffffff';
    }
  }};
`;

const DOMHeader = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  padding: 8px 16px;
  background: #ffffff;
  border-bottom: 1px solid #e9ecef;
`;

const HeaderCell = styled.div<{ align: 'left' | 'center' | 'right' }>`
  font-size: 11px;
  color: #6c757d;
  font-weight: 600;
  text-transform: uppercase;
  text-align: ${props => props.align};
`;

const DOMRows = styled.div`
  display: flex;
  flex-direction: column;
`;

const DOMRow = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  min-height: 32px;
  border-bottom: 1px solid #e9ecef;
  
  &:last-child {
    border-bottom: none;
  }
`;

const BidCell = styled.div`
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 4px 8px;
  position: relative;
`;

const PriceCell = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 4px 8px;
  border-left: 1px solid #e9ecef;
  border-right: 1px solid #e9ecef;
  gap: 2px;
`;

const AskCell = styled.div`
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 4px 8px;
  position: relative;
`;

const BidRow = styled.div<{ selected?: boolean }>`
  position: relative;
  width: 100%;
  padding: 4px 8px;
  cursor: pointer;
  border-radius: 4px;
  background: ${props => props.selected ? 'rgba(40, 167, 69, 0.2)' : 'transparent'};
  border: ${props => props.selected ? '1px solid #28a745' : '1px solid transparent'};
  
  &:hover {
    background: rgba(40, 167, 69, 0.1);
  }
`;

const AskRow = styled.div<{ selected?: boolean }>`
  position: relative;
  width: 100%;
  padding: 4px 8px;
  cursor: pointer;
  border-radius: 4px;
  background: ${props => props.selected ? 'rgba(220, 53, 69, 0.2)' : 'transparent'};
  border: ${props => props.selected ? '1px solid #dc3545' : '1px solid transparent'};
  
  &:hover {
    background: rgba(220, 53, 69, 0.1);
  }
`;

const BidDepthBar = styled.div<{ width: number }>`
  position: absolute;
  top: 0;
  right: 0;
  height: 100%;
  width: ${props => props.width}%;
  background: linear-gradient(90deg, rgba(2, 192, 118, 0.2) 0%, rgba(2, 192, 118, 0.05) 100%);
  border-radius: 4px;
  pointer-events: none;
`;

const AskDepthBar = styled.div<{ width: number }>`
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: ${props => props.width}%;
  background: linear-gradient(90deg, rgba(248, 73, 96, 0.2) 0%, rgba(248, 73, 96, 0.05) 100%);
  border-radius: 4px;
  pointer-events: none;
`;

const BidQuantity = styled.div`
  color: #28a745;
  font-weight: 500;
  font-size: 12px;
  position: relative;
  z-index: 1;
`;

const AskQuantity = styled.div`
  color: #dc3545;
  font-weight: 500;
  font-size: 12px;
  position: relative;
  z-index: 1;
`;

const BidPrice = styled.div<{ selected?: boolean }>`
  color: #28a745;
  font-weight: 600;
  font-size: 12px;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 2px;
  background: ${props => props.selected ? 'rgba(40, 167, 69, 0.2)' : 'transparent'};
  
  &:hover {
    background: rgba(40, 167, 69, 0.1);
  }
`;

const AskPrice = styled.div<{ selected?: boolean }>`
  color: #dc3545;
  font-weight: 600;
  font-size: 12px;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 2px;
  background: ${props => props.selected ? 'rgba(220, 53, 69, 0.2)' : 'transparent'};
  
  &:hover {
    background: rgba(220, 53, 69, 0.1);
  }
`;

const EmptyCell = styled.div`
  height: 24px;
`;

const AskSection = styled.div`
  display: flex;
  flex-direction: column;
`;

const BidSection = styled.div`
  display: flex;
  flex-direction: column;
`;

const OrderRow = styled.div<{ type: 'bid' | 'ask'; selected?: boolean }>`
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1fr;
  padding: 6px 16px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  border-left: ${props => props.selected ? '3px solid #007bff' : '3px solid transparent'};
  background: ${props => props.selected ? 'rgba(0, 123, 255, 0.1)' : 'transparent'};

  &:hover {
    background: rgba(0, 0, 0, 0.05);
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
      ? 'linear-gradient(90deg, rgba(2, 192, 118, 0.2) 0%, rgba(2, 192, 118, 0.05) 100%)'
      : 'linear-gradient(90deg, rgba(248, 73, 96, 0.2) 0%, rgba(248, 73, 96, 0.05) 100%)'};
  transition: width 0.3s ease;
  pointer-events: none;
`;

const Cell = styled.div<{ type: 'bid' | 'ask' }>`
  color: ${props => (props.type === 'bid' ? '#28a745' : '#dc3545')};
  font-weight: 500;
  position: relative;
  z-index: 1;
`;

const SpreadDivider = styled.div`
  padding: 12px 16px;
  text-align: center;
  background: #ffffff;
  border-top: 1px solid #e9ecef;
  border-bottom: 1px solid #e9ecef;
`;

const SpreadLine = styled.div`
  height: 2px;
  background: linear-gradient(90deg, #28a745 0%, #ffc107 50%, #dc3545 100%);
  border-radius: 1px;
  margin-bottom: 8px;
`;

const SpreadText = styled.div`
  font-size: 11px;
  color: #ffc107;
  font-weight: 500;
`;


const OrderSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  padding: 16px;
  flex-shrink: 0;
`;

const OrderForm = styled.div`
  display: flex;
  flex-direction: column;
  gap: 12px;
`;

const OrderInfo = styled.div`
  padding: 8px;
  background: #ffffff;
  border-radius: 4px;
  border: 1px solid #e9ecef;
`;

const OrderLabel = styled.div`
  font-size: 11px;
  color: #6c757d;
  margin-bottom: 4px;
`;

const OrderValue = styled.div<{ type: 'bid' | 'ask' }>`
  font-size: 14px;
  font-weight: 600;
  color: ${props => (props.type === 'bid' ? '#28a745' : '#dc3545')};
`;

const FormGroup = styled.div`
  display: flex;
  flex-direction: column;
  gap: 4px;
`;

const Input = styled.input`
  background: #e9ecef;
  border: 1px solid #007bff;
  border-radius: 4px;
  color: #1a1a1a;
  padding: 8px 12px;
  font-size: 12px;
  font-family: inherit;

  &:focus {
    outline: none;
    border-color: #0056b3;
  }
`;

const TotalAmount = styled.div`
  font-size: 12px;
  color: #1a1a1a;
  font-weight: 500;
`;

const OrderButton = styled.button<{ type: 'bid' | 'ask' }>`
  background: ${props => (props.type === 'bid' ? '#28a745' : '#dc3545')};
  border: none;
  border-radius: 4px;
  color: white;
  padding: 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover:not(:disabled) {
    background: ${props => (props.type === 'bid' ? '#1e7e34' : '#c82333')};
    transform: translateY(-1px);
  }

  &:disabled {
    background: #e9ecef;
    color: #6c757d;
    cursor: not-allowed;
  }
`;

const NoSelection = styled.div`
  text-align: center;
  color: #6c757d;
  font-size: 12px;
  padding: 20px;
`;

const ExecutionsSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  padding: 16px;
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 500px; /* Much larger minimum height */
  max-height: 800px; /* Much larger maximum height */
`;

const ExecutionsList = styled.div`
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow-y: auto;
  flex: 1;
  
  /* Custom scrollbar styling */
  &::-webkit-scrollbar {
    width: 6px;
  }
  
  &::-webkit-scrollbar-track {
    background: #f8f9fa;
    border-radius: 3px;
  }
  
  &::-webkit-scrollbar-thumb {
    background: #007bff;
    border-radius: 3px;
  }
  
  &::-webkit-scrollbar-thumb:hover {
    background: #0056b3;
  }
`;

const ExecutionRow = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1fr;
  padding: 6px 0;
  font-size: 11px;
  border-bottom: 1px solid #e9ecef;

  &:last-child {
    border-bottom: none;
  }
`;

const ExecutionSide = styled.div<{ type: 'Buy' | 'Sell' }>`
  color: ${props => (props.type === 'Buy' ? '#02c076' : '#f84960')};
  font-weight: 500;
`;

const ExecutionPrice = styled.div`
  color: #1a1a1a;
  font-weight: 500;
`;

const ExecutionQuantity = styled.div`
  color: #c7c9cb;
`;

const ExecutionTime = styled.div`
  color: #6c757d;
  font-size: 10px;
`;

const LoadingMessage = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: #6c757d;
  font-size: 14px;
`;