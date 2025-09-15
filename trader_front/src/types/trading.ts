export interface OrderBookLevel {
  price: number;
  volume: number;
  order_count: number;
}

export interface OrderBook {
  symbol: string;
  timestamp: number;
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}

export interface Execution {
  execution_id: string;
  order_id: string;
  symbol: string;
  side: 'Buy' | 'Sell';
  price: number;
  quantity: number;
  remaining_quantity: number;
  timestamp: number;
  counterparty_id: string;
  is_maker: boolean;
}

export interface MarketStatistics {
  symbol: string;
  timestamp: number;
  open_price_24h: number;
  high_price_24h: number;
  low_price_24h: number;
  last_price: number;
  volume_24h: number;
  price_change_24h: number;
  bid_price: number | null;
  ask_price: number | null;
}

export interface Candle {
  open_time: number;
  close_time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  trade_count: number;
}

export interface Order {
  id: string;
  symbol: string;
  side: 'Buy' | 'Sell';
  order_type: 'Market' | 'Limit';
  price?: number;
  quantity: number;
  status: 'Pending' | 'Filled' | 'Cancelled';
  created_at: number;
}

export type TimeInterval = '1m' | '5m' | '15m' | '30m' | '1h' | '4h' | '1d' | '1w';