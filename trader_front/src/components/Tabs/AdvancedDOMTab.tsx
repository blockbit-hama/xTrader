import React, { useState, useEffect } from 'react';
import styled from 'styled-components';
import TradingViewChart from '../TradingViewChart';
import TechnicalIndicatorControls from '../TechnicalIndicatorControls';
import { OrderBook, MarketStatistics, Execution, Candle } from '../../types/trading';

// Types
interface OrderLevel {
  price: number;
  quantity: number;
  percentage: number;
}

interface ProcessedOrderBook {
  bids: OrderLevel[];
  asks: OrderLevel[];
}

interface CandleData {
  time?: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  open_time?: number;
  close_time?: number;
  trade_count?: number;
}

interface Notification {
  id: string;
  type: 'success' | 'error' | 'info';
  message: string;
  timestamp: number;
}

interface UserBalance {
  BTC: number;
  KRW: number;
}

interface AdvancedDOMTabProps {
  orderBook?: OrderBook | null;
  statistics?: MarketStatistics | null;
  executions: Execution[];
  loading: boolean;
  balance: UserBalance;
  onSubmitOrder: (orderData: any) => Promise<void>;
  onPriceClick: (price: number, side: 'bid' | 'ask') => void;
  candles: Candle[];
  showMockData: boolean;
  mockCandles: Candle[];
}

// Mock data generators
const generateMockOrderBook = (): ProcessedOrderBook => {
  const basePrice = 95000000;
  const bids: OrderLevel[] = [];
  const asks: OrderLevel[] = [];
  
  // Generate bids (lower prices)
  for (let i = 0; i < 15; i++) {
    const price = basePrice - (i * 1000);
    const quantity = Math.random() * 2 + 0.1;
    bids.push({
      price,
      quantity,
      percentage: Math.random() * 100
    });
  }
  
  // Generate asks (higher prices)
  for (let i = 0; i < 15; i++) {
    const price = basePrice + ((i + 1) * 1000);
    const quantity = Math.random() * 2 + 0.1;
    asks.push({
      price,
      quantity,
      percentage: Math.random() * 100
    });
  }
  
  return { bids, asks };
};

const generateMockExecutions = (): Execution[] => {
  const executions: Execution[] = [];
  const basePrice = 95000000;
  
  for (let i = 0; i < 50; i++) {
    const side = Math.random() > 0.5 ? 'Buy' : 'Sell';
    const price = basePrice + (Math.random() - 0.5) * 100000;
    const quantity = Math.random() * 1 + 0.01;
    const timestamp = Date.now() - (i * 60000); // 1 minute intervals
    
    executions.push({
      execution_id: `exec_${i}`,
      order_id: `order_${i}`,
      symbol: 'BTC-KRW',
      side,
      price,
      quantity,
      remaining_quantity: 0,
      timestamp: Math.floor(timestamp / 1000),
      counterparty_id: `user_${Math.floor(Math.random() * 100)}`,
      is_maker: Math.random() > 0.5
    });
  }
  
  return executions.sort((a, b) => b.timestamp - a.timestamp);
};

// Utility functions
const formatPrice = (price: number): string => {
  return new Intl.NumberFormat('ko-KR').format(price);
};

const formatQuantity = (quantity: number): string => {
  return quantity.toFixed(6);
};

// Convert OrderBook to ProcessedOrderBook format
const convertOrderBook = (orderBook: OrderBook | null | undefined): ProcessedOrderBook => {
  if (!orderBook) {
    return generateMockOrderBook();
  }
  
  const bids: OrderLevel[] = orderBook.bids.map(bid => ({
    price: bid.price,
    quantity: bid.volume,
    percentage: Math.random() * 100 // Calculate based on volume if needed
  }));
  
  const asks: OrderLevel[] = orderBook.asks.map(ask => ({
    price: ask.price,
    quantity: ask.volume,
    percentage: Math.random() * 100 // Calculate based on volume if needed
  }));
  
  return { bids, asks };
};

// Convert candle data to TradingViewChart format
const convertCandleData = (candles: Candle[]) => {
  return candles.map(candle => ({
    time: candle.open_time,
    open: candle.open,
    high: candle.high,
    low: candle.low,
    close: candle.close,
    volume: candle.volume,
    open_time: candle.open_time,
    close_time: candle.close_time,
    trade_count: candle.trade_count
  }));
};

// Main Component
const AdvancedDOMTab: React.FC<AdvancedDOMTabProps> = ({
  orderBook,
  statistics,
  executions,
  loading,
  balance,
  onSubmitOrder,
  onPriceClick,
  candles,
  showMockData,
  mockCandles
}) => {
  // State management
  const [depthLevels, setDepthLevels] = useState(15);
  const [priceIncrement, setPriceIncrement] = useState(1000);
  const [showVolumeProfile, setShowVolumeProfile] = useState(false);
  const [showHeatmap, setShowHeatmap] = useState(false);
  const [selectedPrice, setSelectedPrice] = useState<number | null>(null);
  const [selectedSide, setSelectedSide] = useState<'bid' | 'ask' | null>(null);
  const [orderType, setOrderType] = useState<'Market' | 'Limit'>('Limit');
  const [orderQuantity, setOrderQuantity] = useState('');
  const [orderStatus, setOrderStatus] = useState<'idle' | 'submitting' | 'submitted' | 'executed' | 'failed'>('idle');
  const [orderNotifications, setOrderNotifications] = useState<Notification[]>([]);
  const [userBalance, setUserBalance] = useState<UserBalance>(balance);
  const [enabledIndicators, setEnabledIndicators] = useState<Set<string>>(new Set());

  // Mock data
  const mockOrderBook = generateMockOrderBook();
  const mockExecutions = generateMockExecutions();
  
  // Convert data
  const processedOrderBook = convertOrderBook(orderBook);
  const convertedCandles = convertCandleData(candles.length > 0 ? candles : mockCandles);

  // Notification management
  const addNotification = (type: 'success' | 'error' | 'info', message: string) => {
    const notification: Notification = {
      id: Date.now().toString(),
      type,
      message,
      timestamp: Date.now()
    };
    
    setOrderNotifications(prev => [...prev, notification]);
    
    // Auto remove after 5 seconds
    setTimeout(() => {
      removeNotification(notification.id);
    }, 5000);
  };

  const removeNotification = (id: string) => {
    setOrderNotifications(prev => prev.filter(n => n.id !== id));
  };

  // Balance update
  const updateBalance = (side: 'bid' | 'ask', quantity: number, price: number) => {
    setUserBalance(prev => {
      const newBalance = { ...prev };
      
      if (side === 'bid') {
        // Buying BTC with KRW
        newBalance.BTC += quantity;
        newBalance.KRW -= quantity * price;
      } else {
        // Selling BTC for KRW
        newBalance.BTC -= quantity;
        newBalance.KRW += quantity * price;
      }
      
      return newBalance;
    });
  };

  // Price click handler
  const handlePriceClick = (price: number, side: 'bid' | 'ask') => {
    setSelectedPrice(price);
    setSelectedSide(side);
    onPriceClick(price, side);
  };

  // Order submission
  const handleOrderSubmit = async () => {
    if (!selectedPrice || !selectedSide || !orderQuantity) return;
    
    const quantity = parseFloat(orderQuantity);
    if (quantity <= 0) return;

    try {
      setOrderStatus('submitting');
      addNotification('info', '주문을 접수하고 있습니다...');

      const orderData = {
        symbol: 'BTC-KRW',
        side: selectedSide === 'bid' ? 'Buy' : 'Sell',
        type: orderType,
        quantity: Math.floor(quantity * 1000000), // Convert to satoshis
        price: selectedSide === 'bid' ? selectedPrice : undefined,
        client_id: 'test_user_001'
      };

      await onSubmitOrder(orderData);
      
      setOrderStatus('submitted');
      addNotification('success', '주문이 성공적으로 접수되었습니다!');
      
      // Reset form
      setOrderQuantity('');
      
      // Simulate execution after 2-5 seconds
      const executionDelay = 2000 + Math.random() * 3000;
      setTimeout(() => {
        setOrderStatus('executed');
        addNotification('success', `주문이 체결되었습니다! ${formatPrice(selectedPrice)} KRW에 ${quantity.toFixed(6)} BTC ${selectedSide === 'bid' ? '매수' : '매도'}`);
        
        // Update balance
        updateBalance(selectedSide, quantity, selectedPrice);
        
        // Simulate orderbook update notification
        setTimeout(() => {
          addNotification('info', '오더북이 업데이트되었습니다');
        }, 1000);
        
        setOrderStatus('idle');
      }, executionDelay);
      
    } catch (error) {
      setOrderStatus('failed');
      addNotification('error', '주문 접수 중 오류가 발생했습니다.');
      console.error('Order submission error:', error);
    }
  };

  // Technical indicator toggle
  const handleToggleIndicator = (indicator: string) => {
    setEnabledIndicators(prev => {
      const newSet = new Set(prev);
      if (newSet.has(indicator)) {
        newSet.delete(indicator);
      } else {
        newSet.add(indicator);
      }
      return newSet;
    });
  };

  // Process order book data for display
  const processedData = React.useMemo(() => {
    const data = processedOrderBook;
    
    // Calculate cumulative percentages
    const processedBids = data.bids.map((bid, index) => ({
      ...bid,
      percentage: data.bids.slice(0, index + 1).reduce((sum, b) => sum + b.percentage, 0)
    }));
    
    const processedAsks = data.asks.map((ask, index) => ({
      ...ask,
      percentage: data.asks.slice(0, index + 1).reduce((sum, a) => sum + a.percentage, 0)
    }));
    
    return { processedBids, processedAsks };
  }, [processedOrderBook]);

  return (
    <>
      {/* Notification Container */}
      <NotificationContainer>
        {orderNotifications.map((notification) => (
          <NotificationBox key={notification.id} type={notification.type}>
            <NotificationHeader type={notification.type}>
              <NotificationIcon type={notification.type}>
                {notification.type === 'success' ? '✅' : 
                 notification.type === 'error' ? '❌' : 'ℹ️'}
              </NotificationIcon>
              <NotificationTitle type={notification.type}>
                {notification.type === 'success' ? '성공' : 
                 notification.type === 'error' ? '오류' : '정보'}
              </NotificationTitle>
              <CloseButton onClick={() => removeNotification(notification.id)}>
                ×
              </CloseButton>
            </NotificationHeader>
            <NotificationMessage type={notification.type}>
              {notification.message}
            </NotificationMessage>
          </NotificationBox>
        ))}
      </NotificationContainer>

      <Container>
        {/* Header */}
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
              
              {/* Technical Indicator Controls */}
              <TechnicalIndicatorControls
                onToggleIndicator={handleToggleIndicator}
                enabledIndicators={enabledIndicators}
              />
              
              <ChartContainer>
                {convertedCandles.length > 0 ? (
                  <TradingViewChart
                    data={convertedCandles}
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
                    <InfoValue type="bid">{formatPrice(processedOrderBook.bids[0]?.price || 0)}</InfoValue>
                  </InfoItem>
                  <InfoItem>
                    <InfoLabel>최저 매도가:</InfoLabel>
                    <InfoValue type="ask">{formatPrice(processedOrderBook.asks[0]?.price || 0)}</InfoValue>
                  </InfoItem>
                  <InfoItem>
                    <InfoLabel>스프레드:</InfoLabel>
                    <InfoValue type="spread">
                      {formatPrice((processedOrderBook.asks[0]?.price || 0) - (processedOrderBook.bids[0]?.price || 0))}
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
                  {(() => {
                    const rows: React.ReactElement[] = [];
                    
                    // Create a map of all prices with their data
                    const priceMap = new Map<number, { ask: OrderLevel | null; bid: OrderLevel | null }>();
                    
                    // Add asks to price map
                    processedData.processedAsks.forEach(ask => {
                      priceMap.set(ask.price, { ask, bid: null });
                    });
                    
                    // Add bids to price map
                    processedData.processedBids.forEach(bid => {
                      if (priceMap.has(bid.price)) {
                        const existing = priceMap.get(bid.price);
                        if (existing) existing.bid = bid;
                      } else {
                        priceMap.set(bid.price, { ask: null, bid });
                      }
                    });
                    
                    // Get all unique prices and sort them in descending order
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
            {/* Balance Section */}
            <OrderSection>
              <SectionTitle>💰 내 자산</SectionTitle>
              <OrderForm>
                <OrderInfo>
                  <OrderLabel>BTC:</OrderLabel>
                  <OrderValue type="bid">{userBalance.BTC.toFixed(8)} BTC</OrderValue>
                </OrderInfo>
                <OrderInfo>
                  <OrderLabel>KRW:</OrderLabel>
                  <OrderValue type="ask">{formatPrice(userBalance.KRW)} KRW</OrderValue>
                </OrderInfo>
              </OrderForm>
            </OrderSection>

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
                    disabled={!orderQuantity || parseFloat(orderQuantity) <= 0 || orderStatus !== 'idle'}
                    type={selectedSide}
                  >
                    {orderStatus === 'submitting' ? '⏳ 접수 중...' :
                     orderStatus === 'submitted' ? '✅ 접수됨' :
                     orderStatus === 'executed' ? '🎉 체결됨' :
                     orderStatus === 'failed' ? '❌ 실패' :
                     selectedSide === 'bid' ? '💰 매수 주문' : '💸 매도 주문'}
                  </OrderButton>
                </OrderForm>
              ) : (
                <NoSelection>
                  💡 호가창에서 가격을 클릭하여 주문하세요
                </NoSelection>
              )}
            </OrderSection>

            {/* Recent Executions */}
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
                  mockExecutions.map((execution, index) => {
                    const side = execution.side;
                    const price = execution.price;
                    const quantity = execution.quantity;
                    const timestamp = execution.timestamp * 1000;
                    
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
    </>
  );
};

// Styled Components
const NotificationContainer = styled.div`
  position: fixed;
  top: 20px;
  right: 20px;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  gap: 10px;
`;

const NotificationBox = styled.div<{ type: 'success' | 'error' | 'info' }>`
  background: ${props => 
    props.type === 'success' ? '#d4edda' :
    props.type === 'error' ? '#f8d7da' : '#d1ecf1'
  };
  border: 1px solid ${props => 
    props.type === 'success' ? '#c3e6cb' :
    props.type === 'error' ? '#f5c6cb' : '#bee5eb'
  };
  border-radius: 8px;
  padding: 12px;
  min-width: 300px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  animation: slideIn 0.3s ease-out;
  
  @keyframes slideIn {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
`;

const NotificationHeader = styled.div<{ type: 'success' | 'error' | 'info' }>`
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
`;

const NotificationIcon = styled.div<{ type: 'success' | 'error' | 'info' }>`
  font-size: 16px;
`;

const NotificationTitle = styled.div<{ type: 'success' | 'error' | 'info' }>`
  font-weight: 600;
  color: ${props => 
    props.type === 'success' ? '#155724' :
    props.type === 'error' ? '#721c24' : '#0c5460'
  };
  flex: 1;
`;

const CloseButton = styled.button`
  background: none;
  border: none;
  font-size: 18px;
  cursor: pointer;
  color: #6c757d;
  padding: 0;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  
  &:hover {
    color: #495057;
  }
`;

const NotificationMessage = styled.div<{ type: 'success' | 'error' | 'info' }>`
  color: ${props => 
    props.type === 'success' ? '#155724' :
    props.type === 'error' ? '#721c24' : '#0c5460'
  };
  font-size: 14px;
`;

const Container = styled.div`
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #ffffff;
  color: #1a1a1a;
`;

const Header = styled.div`
  background: #f8f9fa;
  border-bottom: 1px solid #e9ecef;
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 16px;
`;

const Title = styled.h1`
  margin: 0;
  font-size: 24px;
  font-weight: 600;
  color: #1a1a1a;
`;

const DataStatus = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
`;

const StatusBadge = styled.div<{ type: 'live' | 'mock' }>`
  padding: 4px 12px;
  border-radius: 20px;
  font-size: 12px;
  font-weight: 500;
  background: ${props => props.type === 'live' ? '#d4edda' : '#fff3cd'};
  color: ${props => props.type === 'live' ? '#155724' : '#856404'};
  border: 1px solid ${props => props.type === 'live' ? '#c3e6cb' : '#ffeaa7'};
`;

const Controls = styled.div`
  display: flex;
  align-items: center;
  gap: 24px;
  flex-wrap: wrap;
`;

const ControlGroup = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
`;

const Label = styled.label`
  font-size: 14px;
  font-weight: 500;
  color: #495057;
`;

const Select = styled.select`
  padding: 6px 12px;
  border: 1px solid #ced4da;
  border-radius: 4px;
  background: #ffffff;
  color: #495057;
  font-size: 14px;
  
  &:focus {
    outline: none;
    border-color: #80bdff;
    box-shadow: 0 0 0 0.2rem rgba(0, 123, 255, 0.25);
  }
`;

const CheckboxLabel = styled.label`
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: #495057;
  cursor: pointer;
  
  input[type="checkbox"] {
    margin: 0;
  }
`;

const MainContent = styled.div`
  display: flex;
  flex: 1;
  overflow: hidden;
`;

const LeftPanel = styled.div`
  flex: 2;
  min-width: 800px;
  display: flex;
  flex-direction: column;
  border-right: 1px solid #e9ecef;
`;

const CenterPanel = styled.div`
  width: 350px;
  display: flex;
  flex-direction: column;
  border-right: 1px solid #e9ecef;
`;

const RightPanel = styled.div`
  width: 300px;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  height: calc(100vh - 40px);
`;

const ChartSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  margin: 16px;
  padding: 16px;
  flex: 1;
  display: flex;
  flex-direction: column;
`;

const SectionHeader = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
`;

const SectionTitle = styled.h3`
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #1a1a1a;
`;

const ChartContainer = styled.div`
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #ffffff;
  border-radius: 4px;
  padding: 8px;
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
  margin: 16px;
  padding: 16px;
  flex: 1;
  display: flex;
  flex-direction: column;
`;

const MarketInfo = styled.div`
  display: flex;
  flex-direction: column;
  gap: 8px;
`;

const InfoItem = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
`;

const InfoLabel = styled.span`
  font-size: 12px;
  color: #6c757d;
`;

const InfoValue = styled.span<{ type: 'bid' | 'ask' | 'spread' }>`
  font-size: 12px;
  font-weight: 500;
  color: ${props => 
    props.type === 'bid' ? '#28a745' :
    props.type === 'ask' ? '#dc3545' : '#6c757d'
  };
`;

const OrderBookContent = styled.div`
  flex: 1;
  overflow-y: auto;
  border: 1px solid #e9ecef;
  border-radius: 4px;
  background: #ffffff;
`;

const DOMHeader = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  background: #f8f9fa;
  border-bottom: 1px solid #e9ecef;
  font-size: 12px;
  font-weight: 600;
  color: #495057;
`;

const HeaderCell = styled.div<{ align: 'left' | 'center' | 'right' }>`
  padding: 8px 12px;
  text-align: ${props => props.align};
  border-right: 1px solid #e9ecef;
  
  &:last-child {
    border-right: none;
  }
`;

const DOMRows = styled.div`
  display: flex;
  flex-direction: column;
`;

const DOMRow = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  border-bottom: 1px solid #f1f3f4;
  min-height: 32px;
  
  &:hover {
    background: #f8f9fa;
  }
`;

const BidCell = styled.div`
  padding: 4px 12px;
  display: flex;
  align-items: center;
  border-right: 1px solid #f1f3f4;
`;

const PriceCell = styled.div`
  padding: 4px 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-right: 1px solid #f1f3f4;
`;

const AskCell = styled.div`
  padding: 4px 12px;
  display: flex;
  align-items: center;
`;

const BidRow = styled.div<{ selected: boolean }>`
  display: flex;
  align-items: center;
  width: 100%;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  background: ${props => props.selected ? '#e8f5e8' : 'transparent'};
  
  &:hover {
    background: ${props => props.selected ? '#e8f5e8' : '#f8f9fa'};
  }
`;

const AskRow = styled.div<{ selected: boolean }>`
  display: flex;
  align-items: center;
  width: 100%;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  background: ${props => props.selected ? '#ffeaea' : 'transparent'};
  
  &:hover {
    background: ${props => props.selected ? '#ffeaea' : '#f8f9fa'};
  }
`;

const BidDepthBar = styled.div<{ width: number }>`
  height: 4px;
  background: #28a745;
  width: ${props => Math.min(props.width, 100)}%;
  margin-right: 8px;
  border-radius: 2px;
`;

const AskDepthBar = styled.div<{ width: number }>`
  height: 4px;
  background: #dc3545;
  width: ${props => Math.min(props.width, 100)}%;
  margin-right: 8px;
  border-radius: 2px;
`;

const BidQuantity = styled.div`
  font-size: 12px;
  color: #28a745;
  font-weight: 500;
`;

const AskQuantity = styled.div`
  font-size: 12px;
  color: #dc3545;
  font-weight: 500;
`;

const BidPrice = styled.div<{ selected: boolean }>`
  font-size: 12px;
  color: #28a745;
  font-weight: 500;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  background: ${props => props.selected ? '#e8f5e8' : 'transparent'};
  
  &:hover {
    background: #f8f9fa;
  }
`;

const AskPrice = styled.div<{ selected: boolean }>`
  font-size: 12px;
  color: #dc3545;
  font-weight: 500;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  background: ${props => props.selected ? '#ffeaea' : 'transparent'};
  
  &:hover {
    background: #f8f9fa;
  }
`;

const EmptyCell = styled.div`
  width: 100%;
  height: 20px;
`;

const OrderSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  margin: 16px;
  padding: 16px;
  flex-shrink: 0;
`;

const OrderForm = styled.div`
  display: flex;
  flex-direction: column;
  gap: 12px;
`;

const OrderInfo = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
`;

const OrderLabel = styled.span`
  font-size: 14px;
  color: #495057;
`;

const OrderValue = styled.span<{ type: 'bid' | 'ask' }>`
  font-size: 14px;
  font-weight: 500;
  color: ${props => props.type === 'bid' ? '#28a745' : '#dc3545'};
`;

const FormGroup = styled.div`
  display: flex;
  flex-direction: column;
  gap: 6px;
`;

const Input = styled.input`
  padding: 8px 12px;
  border: 1px solid #ced4da;
  border-radius: 4px;
  background: #ffffff;
  color: #495057;
  font-size: 14px;
  
  &:focus {
    outline: none;
    border-color: #80bdff;
    box-shadow: 0 0 0 0.2rem rgba(0, 123, 255, 0.25);
  }
`;

const TotalAmount = styled.div`
  font-size: 14px;
  font-weight: 600;
  color: #495057;
  padding: 8px 12px;
  background: #f8f9fa;
  border-radius: 4px;
`;

const OrderButton = styled.button<{ type: 'bid' | 'ask'; disabled: boolean }>`
  padding: 12px 16px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 600;
  cursor: ${props => props.disabled ? 'not-allowed' : 'pointer'};
  background: ${props => 
    props.disabled ? '#6c757d' :
    props.type === 'bid' ? '#28a745' : '#dc3545'
  };
  color: #ffffff;
  transition: all 0.2s ease;
  
  &:hover:not(:disabled) {
    background: ${props => props.type === 'bid' ? '#218838' : '#c82333'};
    transform: translateY(-1px);
  }
  
  &:disabled {
    opacity: 0.6;
  }
`;

const NoSelection = styled.div`
  text-align: center;
  color: #6c757d;
  font-size: 14px;
  padding: 20px;
  background: #f8f9fa;
  border-radius: 4px;
`;

const ExecutionsSection = styled.div`
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  margin: 16px;
  padding: 16px;
  min-height: 500px;
  max-height: 800px;
  display: flex;
  flex-direction: column;
`;

const ExecutionsList = styled.div`
  flex: 1;
  overflow-y: auto;
  border: 1px solid #e9ecef;
  border-radius: 4px;
  background: #ffffff;
`;

const ExecutionRow = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1fr;
  padding: 8px 12px;
  border-bottom: 1px solid #f1f3f4;
  font-size: 12px;
  
  &:hover {
    background: #f8f9fa;
  }
`;

const ExecutionSide = styled.div<{ type: 'Buy' | 'Sell' }>`
  color: ${props => props.type === 'Buy' ? '#28a745' : '#dc3545'};
  font-weight: 500;
`;

const ExecutionPrice = styled.div`
  color: #495057;
  font-weight: 500;
`;

const ExecutionQuantity = styled.div`
  color: #6c757d;
`;

const ExecutionTime = styled.div`
  color: #6c757d;
  font-size: 10px;
`;

export default AdvancedDOMTab;