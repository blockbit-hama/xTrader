import { useState, useEffect, useCallback } from 'react';
import { OrderBook, MarketStatistics, Candle, Execution, Order } from '../types/trading';

const API_BASE_URL = 'http://127.0.0.1:7000';
const WS_BASE_URL = 'ws://127.0.0.1:7001';

export const useTradingAPI = (symbol: string) => {
  const [orderBook, setOrderBook] = useState<OrderBook | null>(null);
  const [statistics, setStatistics] = useState<MarketStatistics | null>(null);
  const [candles, setCandles] = useState<Candle[]>([]);
  const [executions, setExecutions] = useState<Execution[]>([]);
  const [orders] = useState<Order[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch order book
  const fetchOrderBook = useCallback(async (isBackground = false) => {
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/orderbook/${symbol}`);
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const data = await response.json();
      setOrderBook(data);
      if (!isBackground) {
        setError(null);
      }
    } catch (err) {
      console.error('Failed to fetch order book:', err);
      if (!isBackground) {
        setError('주문서 데이터를 불러오지 못했습니다.');
      }
    }
  }, [symbol]);

  // Fetch market statistics
  const fetchStatistics = useCallback(async (isBackground = false) => {
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/statistics/${symbol}`);
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const data = await response.json();
      setStatistics(data);
      if (!isBackground) {
        setError(null);
      }
    } catch (err) {
      console.error('Failed to fetch statistics:', err);
      if (!isBackground) {
        setError('시장 통계 데이터를 불러오지 못했습니다.');
      }
    }
  }, [symbol]);

  // Fetch candles
  const fetchCandles = useCallback(async (interval: string = '5m', limit: number = 100) => {
    const url = `${API_BASE_URL}/api/v1/klines/${symbol}/${interval}?limit=${limit}`;
    console.log('🔄 캔들 API 요청:', url);

    try {
      const response = await fetch(url);
      console.log('📡 캔들 API 응답:', response.status, response.statusText);

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const data = await response.json();
      console.log('📊 캔들 데이터 수신:', data);

      setCandles(data.candles || []);
      setError(null);
    } catch (err) {
      console.error('❌ Failed to fetch candles:', err);
      setError('차트 데이터를 불러오지 못했습니다.');
    }
  }, [symbol]);

  // Fetch executions
  const fetchExecutions = useCallback(async (limit: number = 100) => {
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/executions/${symbol}?limit=${limit}`);
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const data = await response.json();
      setExecutions(data);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch executions:', err);
      setError('체결 내역을 불러오지 못했습니다.');
    }
  }, [symbol]);

  // Submit order
  const submitOrder = useCallback(async (orderData: {
    symbol: string;
    side: 'Buy' | 'Sell';
    orderType: 'Market' | 'Limit';
    price?: number;
    quantity: number;
  }) => {
    try {
      setLoading(true);
      const response = await fetch(`${API_BASE_URL}/v1/order`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          symbol: orderData.symbol,
          side: orderData.side,
          order_type: orderData.orderType,
          price: orderData.price,
          quantity: orderData.quantity,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
        throw new Error(errorData.error || `HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      // Refresh data after successful order
      await Promise.all([
        fetchOrderBook(false),
        fetchStatistics(false),
        fetchExecutions(),
      ]);

      setError(null);
      return result;
    } catch (err) {
      console.error('Failed to submit order:', err);
      setError(err instanceof Error ? err.message : '주문 실행에 실패했습니다.');
      throw err;
    } finally {
      setLoading(false);
    }
  }, [fetchOrderBook, fetchStatistics, fetchExecutions]);

  // Cancel order
  const cancelOrder = useCallback(async (orderId: string) => {
    try {
      setLoading(true);
      const response = await fetch(`${API_BASE_URL}/v1/order/cancel`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ order_id: orderId }),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
        throw new Error(errorData.error || `HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      // Refresh data after cancellation
      await Promise.all([
        fetchOrderBook(),
        fetchStatistics(),
      ]);

      setError(null);
      return result;
    } catch (err) {
      console.error('Failed to cancel order:', err);
      setError(err instanceof Error ? err.message : '주문 취소에 실패했습니다.');
      throw err;
    } finally {
      setLoading(false);
    }
  }, [fetchOrderBook, fetchStatistics]);

  // Initial data load
  useEffect(() => {
    const loadInitialData = async () => {
      console.log('🚀 초기 데이터 로드 시작');
      setLoading(true);

      try {
        await Promise.all([
          fetchOrderBook(),
          fetchStatistics(),
          fetchCandles('5m', 100),
          fetchExecutions(50),
        ]);
        console.log('✅ 초기 데이터 로드 완료');
        setLoading(false);
      } catch (error) {
        console.error('❌ 초기 데이터 로드 실패:', error);
        setLoading(false); // 에러가 발생해도 로딩 상태 해제
      }
    };

    loadInitialData();
  }, [fetchOrderBook, fetchStatistics, fetchCandles, fetchExecutions]);

  // Auto-refresh data (background refresh without loading state)
  useEffect(() => {
    const interval = setInterval(() => {
      // 백그라운드 새로고침은 로딩 상태를 변경하지 않음
      fetchOrderBook(true);
      fetchStatistics(true);
    }, 2000); // Refresh every 2 seconds

    return () => clearInterval(interval);
  }, [fetchOrderBook, fetchStatistics]);

  return {
    orderBook,
    statistics,
    candles,
    executions,
    orders,
    loading,
    error,
    submitOrder,
    cancelOrder,
    fetchCandles,
    refreshData: () => {
      fetchOrderBook(true);
      fetchStatistics(true);
      fetchExecutions();
    }
  };
};