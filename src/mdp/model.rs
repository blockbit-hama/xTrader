use serde::{Deserialize, Serialize};

/**
* filename : model
* author : HAMA
* date: 2025. 5. 13.
* description: 
**/


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketDataEvent {
  OrderBookUpdate,
  TradeUpdate,
  CandlestickUpdate,
  Execution,
}
