import React, { useState, useEffect } from 'react';

interface OrderTabProps {
  currentPrice: number;
  balance: { BTC: number; KRW: number };
  onSubmitOrder: (orderData: any) => Promise<any>;
}

const OrderTab: React.FC<OrderTabProps> = ({ currentPrice, balance, onSubmitOrder }) => {
  const [orderType, setOrderType] = useState<'Market' | 'Limit'>('Limit');
  const [orderSide, setOrderSide] = useState<'Buy' | 'Sell'>('Buy');
  const [orderPrice, setOrderPrice] = useState<string>(currentPrice.toString());
  const [orderQuantity, setOrderQuantity] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  // 주문 타입이 변경될 때 가격 필드 업데이트
  useEffect(() => {
    if (orderType === 'Market') {
      setOrderPrice('');
    } else {
      setOrderPrice(currentPrice.toString());
    }
  }, [orderType, currentPrice]);

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
      const result = await onSubmitOrder(orderData);
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

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat('ko-KR').format(num);
  };

  const containerStyle: React.CSSProperties = {
    backgroundColor: '#161a20',
    border: '1px solid #2b3139',
    borderRadius: '8px',
    padding: '16px',
    height: '100%'
  };

  const tabButtonStyle: React.CSSProperties = {
    padding: '8px 16px',
    border: 'none',
    background: 'transparent',
    color: '#848e9c',
    cursor: 'pointer',
    borderBottom: '2px solid transparent',
    fontSize: '14px',
    fontWeight: '500'
  };

  const activeTabButtonStyle: React.CSSProperties = {
    ...tabButtonStyle,
    color: '#ffffff',
    borderBottom: '2px solid #3b82f6'
  };

  const orderTypeButtonStyle: React.CSSProperties = {
    padding: '8px 16px',
    border: 'none',
    borderRadius: '4px',
    cursor: 'pointer',
    fontWeight: '500',
    backgroundColor: '#2b3139',
    color: '#c7c9cb',
    marginRight: '8px',
    fontSize: '12px'
  };

  const activeOrderTypeButtonStyle: React.CSSProperties = {
    ...orderTypeButtonStyle,
    backgroundColor: '#3b82f6',
    color: 'white'
  };

  const buyButtonStyle: React.CSSProperties = {
    padding: '12px 24px',
    border: 'none',
    borderRadius: '4px',
    cursor: 'pointer',
    fontWeight: '500',
    backgroundColor: '#02c076',
    color: 'white',
    width: '100%',
    fontSize: '16px',
    marginTop: '16px'
  };

  const sellButtonStyle: React.CSSProperties = {
    ...buyButtonStyle,
    backgroundColor: '#f84960'
  };

  const disabledButtonStyle: React.CSSProperties = {
    ...buyButtonStyle,
    backgroundColor: '#2b3139',
    color: '#848e9c',
    cursor: 'not-allowed',
    opacity: 0.6
  };

  const inputStyle: React.CSSProperties = {
    width: '100%',
    padding: '12px',
    backgroundColor: '#1e2329',
    border: '1px solid #2b3139',
    borderRadius: '4px',
    color: '#ffffff',
    marginBottom: '12px',
    fontSize: '14px'
  };

  const labelStyle: React.CSSProperties = {
    fontSize: '14px',
    color: '#c7c9cb',
    display: 'block',
    marginBottom: '8px',
    fontWeight: '500'
  };

  const infoBoxStyle: React.CSSProperties = {
    marginBottom: '16px',
    padding: '12px',
    backgroundColor: '#1e2329',
    borderRadius: '4px',
    fontSize: '12px'
  };

  return (
    <div style={containerStyle}>
      <h3 style={{ marginTop: 0, marginBottom: '20px', color: '#ffffff', fontSize: '18px' }}>
        💰 주문하기
      </h3>

      {/* 매수/매도 탭 */}
      <div style={{ display: 'flex', marginBottom: '20px', borderBottom: '1px solid #2b3139' }}>
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
      <div style={{ marginBottom: '20px' }}>
        <div style={labelStyle}>주문 타입</div>
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
        <div style={{ marginBottom: '16px' }}>
          <label style={labelStyle}>
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
        <div style={{ marginBottom: '16px' }}>
          <div style={labelStyle}>
            예상 체결가 (KRW)
          </div>
          <div style={{
            padding: '12px',
            backgroundColor: '#1e2329',
            border: '1px solid #2b3139',
            borderRadius: '4px',
            color: orderSide === 'Buy' ? '#02c076' : '#f84960',
            fontWeight: 'bold',
            fontSize: '16px',
            textAlign: 'center'
          }}>
            ≈ {formatNumber(currentPrice)} KRW
          </div>
        </div>
      )}

      {/* 수량 입력 */}
      <div style={{ marginBottom: '16px' }}>
        <label style={labelStyle}>
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

      {/* 예상 총액 및 정보 */}
      <div style={infoBoxStyle}>
        <div style={{ color: '#c7c9cb', marginBottom: '8px' }}>
          <strong>예상 총액:</strong> {orderQuantity && parseFloat(orderQuantity) > 0 ?
            `${formatNumber((parseFloat(orderQuantity) * (orderType === 'Market' ? currentPrice : parseFloat(orderPrice || '0'))))} KRW` :
            '0 KRW'}
        </div>
        <div style={{ color: '#848e9c', marginBottom: '4px' }}>
          <strong>수수료 (0.1%):</strong> {orderQuantity && parseFloat(orderQuantity) > 0 ?
            `≈ ${formatNumber((parseFloat(orderQuantity) * (orderType === 'Market' ? currentPrice : parseFloat(orderPrice || '0')) * 0.001))} KRW` :
            '0 KRW'}
        </div>
        <div style={{ color: '#848e9c' }}>
          <strong>{orderSide === 'Buy' ? '보유 KRW: ' : '보유 BTC: '}</strong>
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
  );
};

export default OrderTab;