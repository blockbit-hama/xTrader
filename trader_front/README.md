# xTrader Frontend

Professional cryptocurrency trading interface built with React and TypeScript. This frontend connects to the xTrader matching engine backend to provide real-time trading capabilities.

## Features

### 🎯 Professional Trading Interface
- **Real-time Charts**: Powered by Lightweight Charts with candlestick and volume visualization
- **Live Order Book**: Real-time bid/ask display with depth visualization
- **Trading Forms**: Support for both market and limit orders with balance management
- **Trade History**: Complete execution and order history with filtering
- **Live Data**: WebSocket integration for real-time market updates

### 🎨 Modern UI/UX
- **Dark Theme**: Professional trading-focused dark theme
- **Responsive Design**: Optimized for desktop trading workflows
- **Real-time Updates**: Live price feeds and execution notifications
- **Interactive Elements**: Click-to-fill prices from order book
- **Status Indicators**: Connection status and live market data feeds

### 🚀 Technical Stack
- **React 18** with TypeScript
- **Tailwind CSS** for styling
- **Lightweight Charts** for professional charting
- **WebSocket** for real-time data
- **Styled Components** for component styling
- **Lucide React** for icons

## Quick Start

### Prerequisites
- Node.js 16+
- xTrader backend running on `http://127.0.0.1:3030`

### Installation

```bash
# Install dependencies
npm install

# Start development server
npm start
```

The application will start on `http://localhost:3000`.

## Project Structure

```
src/
├── components/           # React components
│   ├── Chart/           # Trading chart components
│   ├── Header/          # Application header
│   ├── OrderBook/       # Order book display
│   ├── TradingForm/     # Order submission forms
│   └── TradeHistory/    # Execution and order history
├── hooks/               # Custom React hooks
│   ├── useWebSocket.ts  # WebSocket connection management
│   └── useTradingAPI.ts # API calls and data management
├── types/               # TypeScript type definitions
│   └── trading.ts       # Trading-related interfaces
├── App.tsx              # Main application component
├── index.tsx            # Application entry point
└── index.css            # Global styles and Tailwind imports
```

## API Integration

### REST API Endpoints
- `GET /api/v1/orderbook/{symbol}` - Order book data
- `GET /api/v1/executions/{symbol}` - Trade history
- `GET /api/v1/statistics/{symbol}` - 24h market statistics
- `GET /api/v1/klines/{symbol}/{interval}` - Candlestick data
- `POST /v1/order` - Submit new order
- `POST /v1/order/cancel` - Cancel existing order

### WebSocket Streams
- `ws://127.0.0.1:3030/ws/executions` - Live execution feed

## Features in Detail

### Trading Chart
- **Candlestick Display**: OHLCV data with customizable intervals
- **Volume Overlay**: Volume histogram with buy/sell coloring
- **Time Intervals**: 1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w
- **Interactive**: Zoom, pan, and crosshair tools

### Order Book
- **Real-time Updates**: Live bid/ask prices and volumes
- **Depth Visualization**: Volume bars showing market depth
- **Spread Display**: Current bid-ask spread
- **Click Integration**: Click prices to auto-fill trading form

### Trading Form
- **Order Types**: Market and limit orders
- **Balance Integration**: Real-time balance checking
- **Percentage Selection**: Quick percentage-based sizing
- **Price Adjustment**: ±1% price adjustment buttons
- **Validation**: Order validation and error handling

### Trade History
- **Dual View**: Both executions and orders in tabbed interface
- **Filtering**: Filter by buy/sell side
- **Real-time Updates**: Live execution feeds via WebSocket
- **Export Ready**: Structured for CSV/Excel export

## Available Scripts

### `npm start`
Runs the app in development mode. Open [http://localhost:3000](http://localhost:3000) to view it in the browser.

### `npm run build`
Builds the app for production to the `build` folder.

### `npm test`
Launches the test runner in interactive watch mode.

### `npm run eject`
**Note: this is a one-way operation. Once you `eject`, you can't go back!**

## Browser Support

- **Chrome**: Full support (recommended)
- **Firefox**: Full support
- **Safari**: Full support
- **Edge**: Full support

---

**Note**: This frontend is designed to work specifically with the xTrader matching engine backend. Ensure the backend is running and accessible before starting the frontend application.
