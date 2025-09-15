import React, { useState, useCallback } from 'react';
import { TrendingUp, TrendingDown } from 'lucide-react';

interface TradingFormProps {
  symbol: string;
  lastPrice: number;
  balance: { [key: string]: number };
  onSubmitOrder: (order: OrderData) => void;
}

interface OrderData {
  symbol: string;
  side: 'Buy' | 'Sell';
  orderType: 'Market' | 'Limit';
  price?: number;
  quantity: number;
}

const TradingForm: React.FC<TradingFormProps> = ({
  symbol,
  lastPrice,
  balance,
  onSubmitOrder
}) => {
  const [side, setSide] = useState<'Buy' | 'Sell'>('Buy');
  const [orderType, setOrderType] = useState<'Market' | 'Limit'>('Limit');
  const [price, setPrice] = useState<string>(lastPrice.toString());
  const [quantity, setQuantity] = useState<string>('');
  const [percentage, setPercentage] = useState<number>(0);

  const baseCurrency = 'BTC'; // Extract from symbol (e.g., BTC-KRW)
  const quoteCurrency = 'KRW';

  const availableBalance = balance[side === 'Buy' ? quoteCurrency : baseCurrency] || 0;
  const currentPrice = orderType === 'Market' ? lastPrice : parseFloat(price) || lastPrice;
  const orderQuantity = parseFloat(quantity) || 0;
  const totalAmount = currentPrice * orderQuantity;

  const handlePriceChange = useCallback((value: string) => {
    setPrice(value);
    setPercentage(0); // Reset percentage when price is manually changed
  }, []);

  const handleQuantityChange = useCallback((value: string) => {
    setQuantity(value);

    if (side === 'Buy' && availableBalance > 0) {
      const qty = parseFloat(value) || 0;
      const total = qty * currentPrice;
      const pct = (total / availableBalance) * 100;
      setPercentage(Math.min(pct, 100));
    } else if (side === 'Sell' && availableBalance > 0) {
      const qty = parseFloat(value) || 0;
      const pct = (qty / availableBalance) * 100;
      setPercentage(Math.min(pct, 100));
    }
  }, [side, availableBalance, currentPrice]);

  const handlePercentageClick = useCallback((pct: number) => {
    setPercentage(pct);

    if (side === 'Buy') {
      const availableAmount = availableBalance * (pct / 100);
      const qty = availableAmount / currentPrice;
      setQuantity(qty.toFixed(6));
    } else {
      const qty = availableBalance * (pct / 100);
      setQuantity(qty.toFixed(6));
    }
  }, [side, availableBalance, currentPrice]);

  const handlePriceAdjustment = useCallback((adjustment: number) => {
    const newPrice = currentPrice * (1 + adjustment / 100);
    setPrice(newPrice.toFixed(0));
  }, [currentPrice]);

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault();

    if (orderQuantity <= 0) {
      alert('수량을 입력해주세요.');
      return;
    }

    if (orderType === 'Limit' && (!price || parseFloat(price) <= 0)) {
      alert('가격을 입력해주세요.');
      return;
    }

    if (side === 'Buy' && totalAmount > availableBalance) {
      alert('잔고가 부족합니다.');
      return;
    }

    if (side === 'Sell' && orderQuantity > availableBalance) {
      alert('보유 수량이 부족합니다.');
      return;
    }

    onSubmitOrder({
      symbol,
      side,
      orderType,
      price: orderType === 'Limit' ? parseFloat(price) : undefined,
      quantity: orderQuantity,
    });

    // Reset form
    setQuantity('');
    setPercentage(0);
  }, [symbol, side, orderType, price, orderQuantity, totalAmount, availableBalance, onSubmitOrder]);

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat('ko-KR').format(num);
  };

  return (
    <div className="bg-trading-bg-secondary rounded-lg border border-trading-border">
      <div className="p-4 border-b border-trading-border">
        <h3 className="text-lg font-semibold text-trading-text-primary">주문</h3>
        <div className="text-sm text-trading-text-muted">{symbol}</div>
      </div>

      <div className="p-4">
        {/* Order Type Tabs */}
        <div className="flex space-x-1 bg-trading-bg-primary rounded p-1 mb-4">
          <button
            onClick={() => setOrderType('Limit')}
            className={`flex-1 py-2 px-3 text-sm rounded transition-colors ${
              orderType === 'Limit'
                ? 'bg-blue-600 text-white'
                : 'text-trading-text-muted hover:text-trading-text-primary'
            }`}
          >
            지정가
          </button>
          <button
            onClick={() => setOrderType('Market')}
            className={`flex-1 py-2 px-3 text-sm rounded transition-colors ${
              orderType === 'Market'
                ? 'bg-blue-600 text-white'
                : 'text-trading-text-muted hover:text-trading-text-primary'
            }`}
          >
            시장가
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Price Input (Only for Limit orders) */}
          {orderType === 'Limit' && (
            <div>
              <label className="block text-sm font-medium text-trading-text-secondary mb-2">
                가격 (KRW)
              </label>
              <div className="relative">
                <input
                  type="number"
                  value={price}
                  onChange={(e) => handlePriceChange(e.target.value)}
                  className="trading-input w-full font-mono"
                  placeholder="가격을 입력하세요"
                  step="1"
                />
                <div className="absolute right-2 top-1/2 transform -translate-y-1/2 flex space-x-1">
                  <button
                    type="button"
                    onClick={() => handlePriceAdjustment(-1)}
                    className="p-1 text-trading-red hover:bg-trading-red-bg rounded"
                  >
                    <TrendingDown size={12} />
                  </button>
                  <button
                    type="button"
                    onClick={() => handlePriceAdjustment(1)}
                    className="p-1 text-trading-green hover:bg-trading-green-bg rounded"
                  >
                    <TrendingUp size={12} />
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Quantity Input */}
          <div>
            <label className="block text-sm font-medium text-trading-text-secondary mb-2">
              수량 ({baseCurrency})
            </label>
            <input
              type="number"
              value={quantity}
              onChange={(e) => handleQuantityChange(e.target.value)}
              className="trading-input w-full font-mono"
              placeholder="수량을 입력하세요"
              step="0.000001"
            />
          </div>

          {/* Percentage Buttons */}
          <div>
            <label className="block text-sm font-medium text-trading-text-secondary mb-2">
              비율 선택
            </label>
            <div className="grid grid-cols-4 gap-2">
              {[25, 50, 75, 100].map(pct => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => handlePercentageClick(pct)}
                  className={`py-2 px-3 text-sm rounded transition-colors border ${
                    Math.abs(percentage - pct) < 1
                      ? 'border-blue-600 bg-blue-600/20 text-blue-400'
                      : 'border-trading-border text-trading-text-secondary hover:border-trading-text-muted'
                  }`}
                >
                  {pct}%
                </button>
              ))}
            </div>
          </div>

          {/* Order Summary */}
          <div className="bg-trading-bg-tertiary rounded p-3 space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-trading-text-muted">사용 가능</span>
              <span className="text-trading-text-primary font-mono">
                {formatNumber(availableBalance)} {side === 'Buy' ? quoteCurrency : baseCurrency}
              </span>
            </div>
            {orderType === 'Limit' && (
              <div className="flex justify-between text-sm">
                <span className="text-trading-text-muted">주문 총액</span>
                <span className="text-trading-text-primary font-mono">
                  {formatNumber(totalAmount)} {quoteCurrency}
                </span>
              </div>
            )}
            <div className="flex justify-between text-sm">
              <span className="text-trading-text-muted">예상 수수료</span>
              <span className="text-trading-text-primary font-mono">
                {formatNumber(totalAmount * 0.001)} {quoteCurrency}
              </span>
            </div>
          </div>

          {/* Buy/Sell Toggle and Submit */}
          <div className="grid grid-cols-2 gap-3">
            <button
              type="button"
              onClick={() => setSide('Buy')}
              className={`py-3 rounded font-medium transition-colors ${
                side === 'Buy'
                  ? 'bg-trading-green text-white'
                  : 'bg-trading-bg-primary text-trading-text-secondary border border-trading-border hover:border-trading-green'
              }`}
            >
              매수
            </button>
            <button
              type="button"
              onClick={() => setSide('Sell')}
              className={`py-3 rounded font-medium transition-colors ${
                side === 'Sell'
                  ? 'bg-trading-red text-white'
                  : 'bg-trading-bg-primary text-trading-text-secondary border border-trading-border hover:border-trading-red'
              }`}
            >
              매도
            </button>
          </div>

          <button
            type="submit"
            className={`w-full py-3 rounded font-medium transition-colors ${
              side === 'Buy'
                ? 'trading-button-buy'
                : 'trading-button-sell'
            }`}
          >
            {side === 'Buy' ? '매수' : '매도'} 주문하기
          </button>
        </form>
      </div>
    </div>
  );
};

export default TradingForm;