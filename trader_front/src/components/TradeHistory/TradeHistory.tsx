import React, { useState } from 'react';
import { Download } from 'lucide-react';
import { Execution, Order } from '../../types/trading';

interface TradeHistoryProps {
  executions: Execution[];
  orders: Order[];
}

const TradeHistory: React.FC<TradeHistoryProps> = ({ executions, orders }) => {
  const [activeTab, setActiveTab] = useState<'executions' | 'orders'>('executions');
  const [filterSide, setFilterSide] = useState<'All' | 'Buy' | 'Sell'>('All');

  const filteredExecutions = executions.filter(exec =>
    filterSide === 'All' || exec.side === filterSide
  );

  const filteredOrders = orders.filter(order =>
    filterSide === 'All' || order.side === filterSide
  );

  return (
    <div className="bg-trading-bg-secondary rounded-lg border border-trading-border">
      <div className="p-4 border-b border-trading-border">
        <div className="flex items-center justify-between">
          <div className="flex space-x-4">
            <button
              onClick={() => setActiveTab('executions')}
              className={`text-sm font-medium transition-colors ${
                activeTab === 'executions'
                  ? 'text-trading-text-primary border-b-2 border-blue-600'
                  : 'text-trading-text-muted hover:text-trading-text-primary'
              }`}
            >
              체결 내역
            </button>
            <button
              onClick={() => setActiveTab('orders')}
              className={`text-sm font-medium transition-colors ${
                activeTab === 'orders'
                  ? 'text-trading-text-primary border-b-2 border-blue-600'
                  : 'text-trading-text-muted hover:text-trading-text-primary'
              }`}
            >
              주문 내역
            </button>
          </div>

          <div className="flex items-center space-x-2">
            <select
              value={filterSide}
              onChange={(e) => setFilterSide(e.target.value as any)}
              className="trading-input text-sm py-1"
            >
              <option value="All">전체</option>
              <option value="Buy">매수</option>
              <option value="Sell">매도</option>
            </select>
            <button className="p-2 text-trading-text-muted hover:text-trading-text-primary transition-colors">
              <Download size={16} />
            </button>
          </div>
        </div>
      </div>

      <div className="p-2">
        {activeTab === 'executions' ? (
          <ExecutionsTable executions={filteredExecutions} />
        ) : (
          <OrdersTable orders={filteredOrders} />
        )}
      </div>
    </div>
  );
};

interface ExecutionsTableProps {
  executions: Execution[];
}

const ExecutionsTable: React.FC<ExecutionsTableProps> = ({ executions }) => {
  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString('ko-KR', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const formatPrice = (price: number) => {
    return new Intl.NumberFormat('ko-KR').format(price);
  };

  const formatQuantity = (quantity: number) => {
    return quantity.toFixed(6);
  };

  if (executions.length === 0) {
    return (
      <div className="text-center py-8 text-trading-text-muted">
        체결 내역이 없습니다.
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {/* Header */}
      <div className="grid grid-cols-6 gap-2 text-xs text-trading-text-muted font-medium py-2 border-b border-trading-border">
        <div>시간</div>
        <div>심볼</div>
        <div>구분</div>
        <div className="text-right">가격</div>
        <div className="text-right">수량</div>
        <div className="text-right">수수료</div>
      </div>

      {/* Rows */}
      <div className="max-h-64 overflow-y-auto">
        {executions.map((execution) => (
          <div
            key={execution.execution_id}
            className="grid grid-cols-6 gap-2 text-xs py-2 hover:bg-trading-bg-hover rounded transition-colors"
          >
            <div className="text-trading-text-muted font-mono">
              {formatTime(execution.timestamp)}
            </div>
            <div className="text-trading-text-primary font-medium">
              {execution.symbol}
            </div>
            <div className={`font-medium ${
              execution.side === 'Buy' ? 'text-trading-green' : 'text-trading-red'
            }`}>
              {execution.side === 'Buy' ? '매수' : '매도'}
            </div>
            <div className="text-right text-trading-text-primary font-mono">
              {formatPrice(execution.price)}
            </div>
            <div className="text-right text-trading-text-primary font-mono">
              {formatQuantity(execution.quantity)}
            </div>
            <div className="text-right text-trading-text-secondary font-mono">
              {execution.remaining_quantity}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

interface OrdersTableProps {
  orders: Order[];
}

const OrdersTable: React.FC<OrdersTableProps> = ({ orders }) => {
  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString('ko-KR', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const formatPrice = (price?: number) => {
    if (!price) return '-';
    return new Intl.NumberFormat('ko-KR').format(price);
  };

  const formatQuantity = (quantity: number) => {
    return quantity.toFixed(6);
  };

  const getStatusColor = (status: Order['status']) => {
    switch (status) {
      case 'Filled':
        return 'text-trading-green';
      case 'Cancelled':
        return 'text-trading-red';
      case 'Pending':
        return 'text-trading-yellow';
      default:
        return 'text-trading-text-muted';
    }
  };

  const getStatusText = (status: Order['status']) => {
    switch (status) {
      case 'Filled':
        return '체결완료';
      case 'Cancelled':
        return '취소됨';
      case 'Pending':
        return '대기중';
      default:
        return status;
    }
  };

  if (orders.length === 0) {
    return (
      <div className="text-center py-8 text-trading-text-muted">
        주문 내역이 없습니다.
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {/* Header */}
      <div className="grid grid-cols-7 gap-2 text-xs text-trading-text-muted font-medium py-2 border-b border-trading-border">
        <div>시간</div>
        <div>심볼</div>
        <div>구분</div>
        <div>타입</div>
        <div className="text-right">가격</div>
        <div className="text-right">수량</div>
        <div>상태</div>
      </div>

      {/* Rows */}
      <div className="max-h-64 overflow-y-auto">
        {orders.map((order) => (
          <div
            key={order.id}
            className="grid grid-cols-7 gap-2 text-xs py-2 hover:bg-trading-bg-hover rounded transition-colors"
          >
            <div className="text-trading-text-muted font-mono">
              {formatTime(order.created_at)}
            </div>
            <div className="text-trading-text-primary font-medium">
              {order.symbol}
            </div>
            <div className={`font-medium ${
              order.side === 'Buy' ? 'text-trading-green' : 'text-trading-red'
            }`}>
              {order.side === 'Buy' ? '매수' : '매도'}
            </div>
            <div className="text-trading-text-secondary">
              {order.order_type === 'Market' ? '시장가' : '지정가'}
            </div>
            <div className="text-right text-trading-text-primary font-mono">
              {formatPrice(order.price)}
            </div>
            <div className="text-right text-trading-text-primary font-mono">
              {formatQuantity(order.quantity)}
            </div>
            <div className={`font-medium ${getStatusColor(order.status)}`}>
              {getStatusText(order.status)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default TradeHistory;