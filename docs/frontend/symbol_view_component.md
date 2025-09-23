# Symbol View Component Specification

**Owner:** David Pak  
**Status:** Draft → Accept upon review  
**Scope:** Central trading widget for fantasy football player trading  
**Audience:** Frontend engineers, UI/UX designers, system architects  

---

## 1. Overview

The **Symbol View Component** is the central trading widget that displays detailed information about a selected fantasy football player. It combines market data, charting, and order placement functionality in a single, comprehensive interface inspired by Robinhood Legend.

**Purpose:** Provide users with a professional-grade trading interface for fantasy football players, featuring real-time price data, historical charts, and streamlined order placement.

---

## 2. Component Structure

### 2.1 Layout
```
┌─────────────────────────────────────────────────────────┐
│                    SYMBOL HEADER                        │
│  Josh Allen (QB - BUF)    $350.00  ↑$5.00 (+1.45%)      │
│  24H: $345.00 - $355.00  Volume: 1,250  [BUY] [SELL]    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│                    CHART AREA                           │ 
│  [1D] [1W] [1M] [3M] [1Y] [5Y]                          │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │                                                 │    │ 
│  │           CANDLESTICK CHART                     │    │
│  │                                                 │    │
│  │                                                 │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 2.2 Order Modal Window
```
┌─────────────────────────────────────┐
│  Buy Josh Allen (QB - BUF)      [×] │
├─────────────────────────────────────┤
│  Current Price: $350.00             │
│                                     │
│  Quantity: [___] shares             │
│  Price: [___] per share             │
│  Total: $____.00                    │
│                                     │
│  [CANCEL]        [PLACE ORDER]      │
└─────────────────────────────────────┘
```

---

## 3. Header Section

### 3.1 Symbol Information
- **Player Name**: "Josh Allen" (large, prominent)
- **Position & Team**: "QB - BUF" (smaller, secondary)
- **Current Price**: "$350.00" (large, prominent)
- **Day Change**: "$5.00 (+1.45%)" with color coding (green/red)
- **24H Range**: "24H: $345.00 - $355.00"
- **Volume**: "Volume: 1,250"

### 3.2 Action Buttons
- **Buy Button**: Primary green button, triggers order modal
- **Sell Button**: Secondary red button, triggers order modal
- **Button States**: Default, Hover, Active, Disabled

### 3.3 Data Sources
- **Player Info**: `GET /api/symbol/{symbolId}/info` (from JSON file with symbol IDs)
- **Current Price**: `GET /api/snapshot/current` (from Whistle order book)
- **Day Change**: Price history calculation (from PostgreSQL)
- **24H High/Low**: Price history calculation (from PostgreSQL)
- **Volume**: Price history calculation (from PostgreSQL)
- **Bot Activity**: Market maker bots provide initial liquidity and price discovery

---

## 4. Chart Section

### 4.1 Timeframe Selector
- **Buttons**: [1D] [1W] [1M] [3M] [1Y] [5Y]
- **Default**: 1D
- **Active State**: Highlighted button with different styling
- **Interaction**: Click to switch timeframes, updates chart data

### 4.2 Candlestick Chart
- **Library**: TradingView Lightweight Charts
- **Chart Type**: Candlestick (OHLC)
- **Data Points**:
  - **1D**: 5-minute candles (288 points)
  - **1W**: 1-hour candles (168 points)
  - **1M**: 4-hour candles (180 points)
  - **3M**: 1-day candles (90 points)
  - **1Y**: 1-week candles (52 points)
  - **5Y**: 1-month candles (60 points)

### 4.3 Chart Features
- **Color Coding**: Green candles (bullish), Red candles (bearish)
- **Interactive**: Hover for price details, zoom, pan
- **Responsive**: Adapts to container size
- **Performance**: ≤ 16ms redraw time
- **Loading State**: Skeleton loader while data loads

### 4.4 Data Sources
- **Historical Data**: `GET /api/price-history/{symbolId}?period={period}&interval={interval}`
- **Real-time Updates**: REST API polling every 1 second
- **Candle Construction**: OHLC data from price history table
- **Composite Endpoint**: `GET /api/symbol/{symbolId}/complete` (recommended for initial load)

---

## 5. Order Modal Section

### 5.1 Modal Structure
- **Header**: "Buy/Sell [Player Name] ([Position] - [Team])"
- **Close Button**: [×] in top-right corner
- **Current Price Display**: Shows live price
- **Form Fields**: Quantity and Price inputs
- **Total Calculation**: Real-time total calculation
- **Action Buttons**: [CANCEL] and [PLACE ORDER]

### 5.2 Order Form Fields
- **Quantity Input**: 
  - Type: Number input
  - Validation: Must be positive integer
  - Placeholder: "Enter quantity"
- **Price Input**:
  - Type: Number input (cents)
  - Validation: Must be within price domain (100-100000 cents)
  - Placeholder: "Enter price per share"

### 5.3 Order Validation
- **Quantity**: Must be positive integer
- **Price**: Must be within price domain (100-100000 cents)
- **Balance Check**: Ensure sufficient funds for buy orders
- **Position Check**: Ensure sufficient shares for sell orders
- **Real-time Validation**: Show errors as user types

### 5.4 Order Submission
- **API Call**: `POST /api/orders/place`
- **Loading State**: Disable form during submission
- **Success Feedback**: Toast notification + modal close
- **Error Handling**: Display error messages in modal

---

## 6. API Models

### 6.1 Symbol Info API
```typescript
// GET /api/symbol/{symbolId}/info
interface SymbolInfoResponse {
  sleeper_id: string;
  name: string;
  position: string;
  team: string;
  last_updated: string;
}

// Example Response
{
  "sleeper_id": "764",
  "name": "Josh Allen",
  "position": "QB",
  "team": "BUF",
  "last_updated": "2024-01-15T10:30:00Z"
}
```

### 6.2 Price History API
```typescript
// GET /api/price-history/{symbolId}?period=1d&interval=5m
interface PriceHistoryResponse {
  symbol_id: string;
  period: string;
  interval: string;
  candles: CandleData[];
}

interface CandleData {
  timestamp: string;  // ISO 8601 format
  open: number;       // Price in cents
  high: number;       // Price in cents
  low: number;        // Price in cents
  close: number;      // Price in cents
  volume: number;     // Number of shares
}

// Example Response
{
  "symbol_id": "764",
  "period": "1d",
  "interval": "5m",
  "candles": [
    {
      "timestamp": "2024-01-15T09:30:00Z",
      "open": 35000,
      "high": 35200,
      "low": 34900,
      "close": 35100,
      "volume": 150
    },
    {
      "timestamp": "2024-01-15T09:35:00Z",
      "open": 35100,
      "high": 35300,
      "low": 35000,
      "close": 35200,
      "volume": 200
    }
  ]
}
```

### 6.3 Current Price API
```typescript
// GET /api/snapshot/current
interface SnapshotResponse {
  id: string;
  timestamp: string;
  tick: number;
  state: {
    order_books: {
      [symbolId: string]: OrderBookData;
    };
  };
}

interface OrderBookData {
  symbol_id: number;
  buy_orders: { [price: string]: number };    // price in cents -> quantity
  sell_orders: { [price: string]: number };   // price in cents -> quantity
  last_trade_price: number | null;            // price in cents
  last_trade_quantity: number | null;         // number of shares
  last_trade_timestamp: string | null;        // ISO 8601 format
}

// Example Response
{
  "id": "3a498d8a-fe38-46f9-bd7c-29a416cfa61c",
  "timestamp": "2024-01-15T10:30:00Z",
  "tick": 912009,
  "state": {
    "order_books": {
      "764": {
        "symbol_id": 764,
        "buy_orders": {
          "35000": 5,
          "34900": 3,
          "34800": 8
        },
        "sell_orders": {
          "35100": 2,
          "35200": 4,
          "35300": 6
        },
        "last_trade_price": 35000,
        "last_trade_quantity": 2,
        "last_trade_timestamp": "2024-01-15T10:29:45Z"
      }
    }
  }
}
```

### 6.4 Order Placement API
```typescript
// POST /api/orders/place
interface OrderPlaceRequest {
  symbol_id: string;
  side: 'BUY' | 'SELL';
  quantity: number;
  price: number;        // Price in cents
  order_type: 'LIMIT' | 'MARKET';
}

interface OrderPlaceResponse {
  order_id: string;
  status: 'ACCEPTED' | 'REJECTED';
  message?: string;
  timestamp: string;
}

// Example Request
{
  "symbol_id": "764",
  "side": "BUY",
  "quantity": 5,
  "price": 35000,
  "order_type": "LIMIT"
}

// Example Success Response
{
  "order_id": "1234567890",
  "status": "ACCEPTED",
  "message": "Order placed successfully",
  "timestamp": "2024-01-15T10:30:00Z"
}

// Example Error Response
{
  "order_id": null,
  "status": "REJECTED",
  "message": "Insufficient funds",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 6.5 Account Info API
```typescript
// GET /api/account/summary
interface AccountSummaryResponse {
  account_id: number;
  currency_balance: number;    // Balance in cents
  total_equity: number;        // Total equity in cents
  day_change: number;          // Day P&L in cents
  day_change_percent: number;  // Day P&L percentage
  buying_power: number;        // Available cash in cents
  last_updated: string;
}

// Example Response
{
  "account_id": 4,
  "currency_balance": 253000,
  "total_equity": 275000,
  "day_change": 5000,
  "day_change_percent": 1.85,
  "buying_power": 253000,
  "last_updated": "2024-01-15T10:30:00Z"
}
```

### 6.6 Error Response Format
```typescript
// Standard error response for all endpoints
interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: any;
  };
  timestamp: string;
}

// Example Error Responses
{
  "error": {
    "code": "SYMBOL_NOT_FOUND",
    "message": "Symbol with ID '999' not found",
    "details": {
      "symbol_id": "999"
    }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}

{
  "error": {
    "code": "INSUFFICIENT_FUNDS",
    "message": "Insufficient funds to place order",
    "details": {
      "required": 175000,
      "available": 150000
    }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}

{
  "error": {
    "code": "INVALID_PRICE",
    "message": "Price must be between 100 and 100000 cents",
    "details": {
      "provided_price": 50,
      "min_price": 100,
      "max_price": 100000
    }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 6.7 HTTP Status Codes
- **200 OK**: Successful request
- **400 Bad Request**: Invalid request parameters
- **401 Unauthorized**: Authentication required
- **403 Forbidden**: Insufficient permissions
- **404 Not Found**: Symbol or resource not found
- **422 Unprocessable Entity**: Validation errors
- **500 Internal Server Error**: Server error

### 6.8 Data Validation Rules
```typescript
// Symbol ID validation
symbol_id: string;  // Must be valid sleeper_id format

// Price validation
price: number;      // Must be between 100-100000 cents ($1.00-$1000.00)

// Quantity validation
quantity: number;   // Must be positive integer

// Order type validation
order_type: 'LIMIT' | 'MARKET';  // Must be one of these values

// Side validation
side: 'BUY' | 'SELL';  // Must be one of these values
```

---

## 7. Data Flow

### 7.1 Initial Load
```typescript
// Option 1: Composite endpoint (recommended)
const completeData = await fetch(`/api/symbol/${symbolId}/complete`);
const { info, current_price, price_history, account_summary } = completeData;

// Option 2: Individual API calls
// 1. Load symbol metadata
const symbolInfo = await fetch(`/api/symbol/${symbolId}/info`);

// 2. Load current price from snapshot
const snapshot = await fetch('/api/snapshot/current');
const currentPrice = snapshot.state.order_books[symbolId].last_trade_price;

// 3. Load price history for chart (5-minute candles for 1D)
const priceHistory = await fetch(`/api/price-history/${symbolId}?period=1d&interval=5m`);

// 4. Load user account info for order validation
const account = await fetch('/api/account/summary');
```

### 7.2 Real-time Updates
```typescript
// Update every second
setInterval(async () => {
  // Update current price
  const snapshot = await fetch('/api/snapshot/current');
  updateCurrentPrice(snapshot.state.order_books[symbolId]);
  
  // Update chart with new data point
  const latestPrice = snapshot.state.order_books[symbolId].last_trade_price;
  addChartDataPoint(latestPrice);
}, 1000);
```

### 7.3 Timeframe Changes
```typescript
// Handle timeframe selection
const handleTimeframeChange = async (timeframe: string) => {
  setLoading(true);
  const interval = getInterval(timeframe); // 5m, 1h, 4h, 1d, 1w, 1M
  const priceHistory = await fetch(`/api/price-history/${symbolId}?period=${timeframe}&interval=${interval}`);
  updateChart(priceHistory);
  setLoading(false);
};

// Interval mapping function
const getInterval = (timeframe: string): string => {
  const intervals = {
    '1d': '5m',   // 5-minute candles
    '1w': '1h',   // 1-hour candles  
    '1m': '4h',   // 4-hour candles
    '3m': '1d',   // 1-day candles
    '1y': '1w',   // 1-week candles
    '5y': '1M'    // 1-month candles
  };
  return intervals[timeframe] || '5m';
};
```

---

## 7.4 Bot System Integration

### 7.4.1 Market Making Bots
- **Initial Liquidity**: Bots place initial buy/sell orders to create market activity
- **Price Discovery**: Bots adjust prices based on market conditions and player projections
- **Continuous Trading**: Bots provide ongoing liquidity to ensure active markets
- **Price History**: Bot activity creates the initial price history data for charts

### 7.4.2 Data Flow with Bots
```typescript
// Bot activity creates price history
// 1. Bots place initial orders
// 2. Trades occur between bots and users
// 3. ExecutionManager records price history
// 4. Frontend displays price history in charts

// Bot-generated price history ensures:
// - No empty charts on new symbols
// - Realistic price movements
// - Continuous market activity
// - Professional trading experience
```

---

## 8. State Management

### 8.1 Component State
```typescript
interface SymbolViewState {
  symbolId: string;
  symbolInfo: PlayerInfo | null;
  currentPrice: number | null;
  dayChange: number;
  dayChangePercent: number;
  priceHistory: CandleData[];
  selectedTimeframe: '1D' | '1W' | '1M' | '3M' | '1Y' | '5Y';
  orderModal: {
    isOpen: boolean;
    side: 'BUY' | 'SELL';
    quantity: number;
    price: number;
    isSubmitting: boolean;
  };
  chart: {
    isLoading: boolean;
    error: string | null;
  };
}
```

### 8.2 Data Hooks
```typescript
// Custom hooks for data management
const useSymbolInfo = (symbolId: string) => { /* ... */ };
const usePriceHistory = (symbolId: string, timeframe: string) => { /* ... */ };
const useCurrentPrice = (symbolId: string) => { /* ... */ };
const useOrderPlacement = () => { /* ... */ };
```

---

## 9. Performance Requirements

### 9.1 SLAs
- **Chart Redraw**: ≤ 16ms per frame
- **Price Updates**: ≤ 100ms after REST API response
- **Order Submission**: ≤ 500ms end-to-end
- **Data Loading**: ≤ 500ms for 1-day price history (5-minute candles)
- **Modal Open**: ≤ 100ms (instant feel)
- **API Response Time**: ≤ 100ms for cached data, ≤ 500ms for database queries
- **Polling Frequency**: 1-second intervals for real-time updates

### 9.2 Optimizations
- **Chart Virtualization**: Only render visible candles
- **Data Caching**: Cache price history in memory and Redis
- **Debounced Updates**: Prevent excessive re-renders during polling
- **Memoization**: Use React.memo for expensive calculations
- **Lazy Loading**: Load chart data only when needed
- **Composite Endpoints**: Use `/api/symbol/{symbolId}/complete` for initial load
- **Parallel Requests**: Fetch multiple data sources simultaneously
- **Error Retry**: Implement exponential backoff for failed requests

---

## 10. Error Handling

### 10.1 Error States
- **Symbol Not Found**: Display error message with retry button
- **Price Data Unavailable**: Show loading state with fallback
- **Chart Loading Failed**: Display error message with retry
- **Order Submission Failed**: Show error message with details
- **Network Issues**: Show offline indicator with retry logic

### 10.2 Fallback Behavior
- **Missing Price Data**: Show last known price with stale indicator
- **Chart Errors**: Display simple line chart as fallback
- **Modal Errors**: Show error message in modal with retry option

---

## 11. Accessibility

### 11.1 Keyboard Navigation
- **Tab Order**: Header → Timeframe buttons → Chart → Buy/Sell buttons
- **Keyboard Shortcuts**: 
  - `B` for Buy, `S` for Sell
  - `1-5` for timeframe selection
  - `Enter` to submit order (when modal is open)
  - `Escape` to close modal

### 11.2 Screen Reader Support
- **Chart Data**: Provide table alternative for chart data
- **Price Changes**: Announce price changes with context
- **Order Status**: Announce order submission results
- **Modal State**: Announce modal open/close

---

## 12. Responsive Design

### 12.1 Breakpoints
- **Desktop**: Full layout with all features
- **Tablet**: Compact header, smaller chart
- **Mobile**: Stacked layout, simplified modal

### 12.2 Mobile Adaptations
- **Chart**: Smaller height, touch-friendly controls
- **Modal**: Full-screen modal on mobile
- **Header**: Condensed information display
- **Buttons**: Larger touch targets

---

## 13. Integration Points

### 13.1 Backend Services
- **order-gateway**: Symbol info, price history, order placement APIs
- **account-service**: Account data, positions, trades APIs
- **ExecutionManager**: Price history recording, trade processing
- **PlayerRegistry**: Symbol ID assignment, player data management
- **Bot System**: Market making, initial liquidity, price discovery

### 13.2 Data Sources
- **JSON File**: Player metadata with assigned symbol IDs
- **PostgreSQL**: Account data, positions, trades, price history
- **Redis Cache**: Cached player metadata, price history, current prices
- **Snapshots**: Live order book data from Whistle engine

### 13.3 Parent Components
- **Layout Manager**: Receives symbol selection from parent
- **Order Book**: Shares symbol context for live updates
- **Account Summary**: Updates after successful orders

### 13.4 Child Components
- **Chart Component**: Handles candlestick rendering
- **Order Modal Component**: Manages order input and validation
- **Price Display Component**: Shows current price and changes

---

## 14. Testing Strategy

### 14.1 Unit Tests
- **Component Rendering**: Test all states and props
- **Data Hooks**: Test data fetching and caching
- **Order Validation**: Test all validation rules
- **Chart Integration**: Test chart data formatting
- **Modal Behavior**: Test open/close and form submission

### 14.2 Integration Tests
- **API Integration**: Test all REST endpoint calls
- **Real-time Updates**: Test price update flow
- **Order Flow**: Test complete order placement
- **Error Scenarios**: Test error handling paths

### 13.3 Visual Tests
- **Chart Rendering**: Test candlestick display
- **Responsive Layout**: Test different screen sizes
- **Loading States**: Test all loading indicators
- **Error States**: Test error message display
- **Modal States**: Test modal open/close animations

---

## 14. Done-When (Acceptance Criteria)

- User can view detailed symbol information with real-time price updates
- User can switch between different chart timeframes (1D/1W/1M/3M/1Y/5Y)
- User can open order modal by clicking Buy/Sell buttons
- User can place orders through the modal with proper validation
- All data updates every second with ≤ 200ms latency
- Chart renders smoothly at 60fps with ≤ 16ms redraw time
- Component works on desktop, tablet, and mobile devices
- All error states are handled gracefully with retry options

---

## 15. Future Enhancements

### 15.1 Advanced Chart Features
- **Technical Indicators**: MA, EMA, VWAP, RSI
- **Drawing Tools**: Trend lines, support/resistance
- **Multiple Timeframes**: Side-by-side chart comparison

### 15.2 Enhanced Order Features
- **Order Types**: Market, Limit, Stop-Loss, Take-Profit
- **Order History**: View past orders in modal
- **Quick Orders**: One-click buy/sell at market price

### 15.3 Social Features
- **Price Alerts**: Set notifications for price movements
- **Watchlist Integration**: Add to watchlist from symbol view
- **Sharing**: Share symbol view with other users

---
