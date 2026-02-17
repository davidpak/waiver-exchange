# Waiver Exchange API Documentation

**Version**: 1.0  
**Base URL**: `http://localhost:8083/api`  
**WebSocket URL**: `ws://localhost:8081/ws`  
**Last Updated**: February 2026

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [REST API Endpoints](#rest-api-endpoints)
4. [WebSocket API](#websocket-api)
5. [Data Models](#data-models)
6. [Error Handling](#error-handling)
7. [Rate Limits](#rate-limits)
8. [Mobile Development Guide](#mobile-development-guide)
9. [Component Integration Examples](#component-integration-examples)

---

## Overview

The Waiver Exchange API provides comprehensive access to a fantasy football trading platform. This API enables mobile developers to build trading applications with real-time market data, account management, and order placement capabilities.

### Key Features
- **Real-time Market Data**: Live order books, price history, and trade data
- **Account Management**: Portfolio tracking, equity history, and position management
- **Order Placement**: Buy/sell orders with multiple order types
- **Player Information**: Comprehensive NFL player data and projections
- **Historical Data**: Price charts and equity performance tracking

### Architecture
- **REST API**: For data retrieval and account information
- **WebSocket**: For real-time updates and order placement
- **Polling Strategy**: 1-second intervals for real-time updates

---

## Authentication

### WebSocket Authentication
All WebSocket connections require authentication before placing orders or accessing account data.

```typescript
// Authentication Request
{
  "id": "1",
  "method": "auth.login",
  "params": {
    "api_key": "ak_test_1234567890abcdef",
    "api_secret": "sk_test_abcdef1234567890"
  }
}

// Authentication Response
{
  "id": "1",
  "result": {
    "authenticated": true,
    "permissions": ["trade", "market_data"],
    "rate_limits": {
      "burst_limit": 10,
      "market_data_per_second": 1000,
      "orders_per_second": 100
    },
    "user_id": "user123"
  }
}
```

### Test API Keys
For development and testing, use these pre-configured API keys:

| Account | API Key | API Secret | Description |
|---------|---------|------------|-------------|
| Test Account 7 | `ak_test_7_abcdef1234567890` | `sk_test_7_1234567890abcdef` | Test account with $50,000 |
| Test Account 8 | `ak_test_1234567890abcdef` | `sk_test_abcdef1234567890` | Test account with $10,000 |
| Admin Account | `ak_admin_abcdef1234567890` | `sk_admin_1234567890abcdef` | Admin account with full permissions |

---

## REST API Endpoints

### Base URL
All REST endpoints are prefixed with `/api`

### 1. Symbol Information

Get detailed information about a specific player/symbol.

**Endpoint**: `GET /api/symbol/{symbol_id}/info`

**Parameters**:
- `symbol_id` (path): The unique symbol ID for the player

**Response**:
```typescript
{
  "name": "Josh Allen",
  "position": "QB",
  "team": "BUF",
  "projected_points": 24.5,
  "last_updated": "2025-01-26T10:30:00.000Z"
}
```

**Example**:
```bash
curl "http://localhost:8083/api/symbol/764/info"
```

### 2. Price History

Get historical price data for charting and analysis.

**Endpoint**: `GET /api/price-history/{symbol_id}`

**Query Parameters**:
- `period` (string): Time period (`1d`, `1w`, `1m`, `3m`, `1y`, `5y`)
- `interval` (string): Data interval (`1m`, `5m`, `15m`, `1h`, `1d`)

**Response**:
```typescript
{
  "symbol_id": "764",
  "period": "1d",
  "interval": "5m",
  "candles": [
    {
      "timestamp": "2025-01-26T09:30:00.000Z",
      "open": 1600,
      "high": 1650,
      "low": 1580,
      "close": 1620,
      "volume": 100
    }
  ]
}
```

**Example**:
```bash
curl "http://localhost:8083/api/price-history/764?period=1d&interval=5m"
```

### 3. Account Summary

Get comprehensive account information including balance, equity, and performance.

**Endpoint**: `GET /api/account/summary`

**Query Parameters**:
- `account_id` (number): The account ID to retrieve

**Response**:
```typescript
{
  "account_id": 8,
  "balance": 1016000,        // In cents ($10,160.00)
  "total_equity": 1032000,   // In cents ($10,320.00)
  "day_change": 32000,       // In cents ($320.00)
  "day_change_percent": 3.2,
  "buying_power": 1016000,   // In cents
  "last_updated": "2025-01-26T10:30:00.000Z"
}
```

**Example**:
```bash
curl "http://localhost:8083/api/account/summary?account_id=8"
```

### 4. Equity History

Get historical equity performance for portfolio tracking.

**Endpoint**: `GET /api/account/equity-history`

**Query Parameters**:
- `account_id` (number): The account ID
- `start_date` (string, optional): Start date in YYYY-MM-DD format
- `end_date` (string, optional): End date in YYYY-MM-DD format

**Response**:
```typescript
{
  "account_id": 8,
  "snapshots": [
    {
      "date": "2025-01-26",
      "total_equity": 1032000,    // In cents
      "cash_balance": 1016000,    // In cents
      "position_value": 16000,    // In cents
      "day_change": 32000,        // In cents
      "day_change_percent": 3.2
    }
  ],
  "total_days": 1
}
```

**Example**:
```bash
curl "http://localhost:8083/api/account/equity-history?account_id=8&start_date=2025-01-01&end_date=2025-01-31"
```

### 5. Current Market Snapshot

Get real-time market data including order books and system state.

**Endpoint**: `GET /api/snapshot/current`

**Response**:
```typescript
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2025-01-26T10:30:00.000Z",
  "tick": 2125000,
  "state": {
    "order_books": {
      "764": {
        "symbol_id": 764,
        "buy_orders": {
          "1600": 10,
          "1595": 5
        },
        "sell_orders": {
          "1610": 8,
          "1615": 12
        },
        "last_trade_price": 1600,
        "last_trade_quantity": 10,
        "last_trade_timestamp": "2025-01-26T10:29:45.000Z"
      }
    },
    "active_symbols": [764, 765, 766],
    "config": {
      "max_symbols": 100,
      "max_accounts": 1000,
      "tick_duration_ns": 1000000
    },
    "stats": {
      "total_orders": 1250,
      "total_trades": 89,
      "total_volume": 1250,
      "current_tick": 2125000,
      "uptime_seconds": 3600
    }
  },
  "metadata": {
    "version": "1.0.0",
    "created_at": "2025-01-26T10:30:00.000Z"
  }
}
```

**Example**:
```bash
curl "http://localhost:8083/api/snapshot/current"
```

### 6. All Players (Bulk)

Get all available players/symbols for search and listing.

**Endpoint**: `GET /api/symbols/all`

**Response**: Array of symbol info objects
```typescript
[
  {
    "symbol_id": 764,
    "name": "Josh Allen",
    "position": "QB",
    "team": "BUF",
    "projected_points": 24.5,
    "last_updated": "2025-01-26T10:30:00.000Z"
  },
  // ... all 467 players
]
```

**Example**:
```bash
curl "http://localhost:8083/api/symbols/all"
```

### 7. Current Price (Single Symbol)

Get the current price for a specific symbol. Checks fair prices (RPE engine) first, then price history, then falls back to a default.

**Endpoint**: `GET /api/symbol/{symbol_id}/price`

**Response**:
```typescript
{
  "symbol_id": 764,
  "price": 3500,             // Price in cents ($35.00)
  "source": "fair_price",    // "fair_price", "price_history", or "default"
  "last_updated": "2025-01-26T10:30:00.000Z"
}
```

**Example**:
```bash
curl "http://localhost:8083/api/symbol/764/price"
```

### 8. Bulk Prices

Get current prices for all symbols in a single request. Prices come from the RPE fair price engine.

**Endpoint**: `GET /api/symbols/prices`

**Response**:
```typescript
{
  "prices": {
    "764": 3500,    // symbol_id -> price in cents
    "765": 2800,
    "766": 4200
    // ... all symbols with fair prices
  },
  "last_updated": "2025-01-26T10:30:00.000Z"
}
```

**Example**:
```bash
curl "http://localhost:8083/api/symbols/prices"
```

### 9. Health Check

Check if the REST API server is healthy.

**Endpoint**: `GET /health`

**Response**:
```typescript
{
  "status": "healthy",
  "timestamp": "2025-01-26T10:30:00.000Z"
}
```

---

## WebSocket API

### Connection
Connect to the WebSocket endpoint for real-time updates and order placement.

```typescript
const ws = new WebSocket('ws://localhost:8081/ws');
```

### Message Format
All WebSocket messages follow this structure:

```typescript
{
  "id": "unique_message_id",
  "method": "method_name",
  "params": { /* method parameters */ }
}
```

### 1. Authentication

**Method**: `auth.login`

**Parameters**:
```typescript
{
  "api_key": "ak_test_1234567890abcdef",
  "api_secret": "sk_test_abcdef1234567890"
}
```

**Response**:
```typescript
{
  "id": "1",
  "result": {
    "authenticated": true,
    "permissions": ["trade", "market_data"],
    "rate_limits": {
      "burst_limit": 10,
      "market_data_per_second": 1000,
      "orders_per_second": 100
    },
    "user_id": "user123"
  }
}
```

### 2. Order Placement

**Method**: `order.submit`

**Parameters**:
```typescript
{
  "symbol": "764",           // Symbol ID as string
  "side": "BUY",             // "BUY" or "SELL"
  "type": "LIMIT",           // "LIMIT", "MARKET", "IOC", "FOK"
  "price": 1600,             // Price in cents
  "quantity": 10,            // Number of shares
  "client_order_id": null    // Optional client ID
}
```

**Response**:
```typescript
{
  "id": "2",
  "result": {
    "client_order_id": null,
    "order_id": "ord_2279854578609150819",
    "status": "ACCEPTED",
    "timestamp": 1758928502004
  }
}
```

### 3. Account Information

**Method**: `account.info`

**Parameters**: `{}`

**Response**:
```typescript
{
  "id": "3",
  "result": {
    "account_id": 8,
    "balance": 1016000,
    "total_equity": 1032000,
    "day_change": 32000,
    "day_change_percent": 3.2
  }
}
```

### 4. Account Positions

**Method**: `account.positions`

**Parameters**: `{}`

**Response**:
```typescript
{
  "id": "4",
  "result": {
    "positions": [
      {
        "symbol_id": 764,
        "quantity": 100000,      // In basis points (10 shares)
        "avg_cost": 1500,        // In cents
        "current_price": 1600,   // In cents
        "unrealized_pnl": 1000,  // In cents
        "realized_pnl": 0        // In cents
      }
    ]
  }
}
```

### 5. Account Trades

**Method**: `account.trades`

**Parameters**: `{}`

**Response**:
```typescript
{
  "id": "5",
  "result": {
    "trades": [
      {
        "trade_id": "trade_123",
        "symbol_id": 764,
        "side": "BUY",
        "quantity": 10,
        "price": 1600,
        "timestamp": "2025-01-26T10:30:00.000Z",
        "execution_id": "exec_456"
      }
    ]
  }
}
```

### 6. Market Data Subscription

**Method**: `market_data.subscribe`

**Parameters**: `{}`

**Response**:
```typescript
{
  "id": "6",
  "result": {
    "subscribed": true,
    "symbols": ["764", "765", "766"]
  }
}
```

---

## Data Models

### Symbol Information
```typescript
interface SymbolInfo {
  name: string;              // Player name
  position: string;          // NFL position (QB, RB, WR, etc.)
  team: string;              // Team abbreviation
  projected_points: number;  // Fantasy points projection
  last_updated: string;      // ISO 8601 timestamp
}
```

### Price History
```typescript
interface PriceHistory {
  symbol_id: string;
  period: string;            // Time period
  interval: string;          // Data interval
  candles: CandleData[];
}

interface CandleData {
  timestamp: string;         // ISO 8601 timestamp
  open: number;             // Opening price in cents
  high: number;             // High price in cents
  low: number;              // Low price in cents
  close: number;            // Closing price in cents
  volume: number;           // Trading volume
}
```

### Account Summary
```typescript
interface AccountSummary {
  account_id: number;
  balance: number;          // Cash balance in cents
  total_equity: number;     // Total portfolio value in cents
  day_change: number;       // Daily change in cents
  day_change_percent: number; // Daily change percentage
  buying_power: number;     // Available buying power in cents
  last_updated: string;     // ISO 8601 timestamp
}
```

### Order Book
```typescript
interface OrderBook {
  symbol_id: number;
  buy_orders: Record<string, number>;   // Price -> Quantity
  sell_orders: Record<string, number>;  // Price -> Quantity
  last_trade_price: number | null;
  last_trade_quantity: number | null;
  last_trade_timestamp: string | null;
}
```

### Position
```typescript
interface Position {
  symbol_id: number;
  quantity: number;         // In basis points (10000 = 1 share)
  avg_cost: number;         // Average cost in cents
  current_price: number;    // Current market price in cents
  unrealized_pnl: number;   // Unrealized P&L in cents
  realized_pnl: number;     // Realized P&L in cents
}
```

### Trade
```typescript
interface Trade {
  trade_id: string;
  symbol_id: number;
  side: 'BUY' | 'SELL';
  quantity: number;         // Number of shares
  price: number;           // Price in cents
  timestamp: string;       // ISO 8601 timestamp
  execution_id: string;
}
```

---

## Error Handling

### HTTP Status Codes
- `200 OK`: Successful request
- `400 Bad Request`: Invalid request parameters
- `401 Unauthorized`: Authentication required
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `429 Too Many Requests`: Rate limit exceeded
- `500 Internal Server Error`: Server error

### Error Response Format
```typescript
{
  "error": {
    "code": "INVALID_SYMBOL",
    "message": "Symbol ID 999 not found",
    "details": {
      "symbol_id": 999,
      "available_symbols": [764, 765, 766]
    }
  },
  "timestamp": "2025-01-26T10:30:00.000Z"
}
```

### Common Error Codes
- `INVALID_SYMBOL`: Symbol ID not found
- `INVALID_ACCOUNT`: Account ID not found
- `INSUFFICIENT_FUNDS`: Not enough cash for order
- `INVALID_ORDER`: Order parameters invalid
- `RATE_LIMIT_EXCEEDED`: Too many requests
- `AUTHENTICATION_FAILED`: Invalid API credentials

---

## Rate Limits

### REST API
- **General**: 1000 requests per minute
- **Price History**: 100 requests per minute
- **Account Data**: 500 requests per minute

### WebSocket
- **Authentication**: 10 attempts per minute
- **Order Placement**: 100 orders per minute
- **Market Data**: 1000 messages per minute

### Headers
Rate limit information is included in response headers:
```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1640995200
```

---

## Mobile Development Guide

### Recommended Architecture

#### 1. State Management
Use a centralized state management solution (Redux, Zustand, etc.) to manage:
- Account data
- Market data
- Order history
- Real-time updates

#### 2. Data Fetching Strategy
```typescript
// Polling intervals for real-time updates
const POLLING_INTERVALS = {
  ACCOUNT_SUMMARY: 1000,      // 1 second
  MARKET_SNAPSHOT: 1000,      // 1 second
  PRICE_HISTORY: 5000,        // 5 seconds
  EQUITY_HISTORY: 30000,      // 30 seconds
};
```

#### 3. WebSocket Connection Management
```typescript
class WebSocketManager {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  
  connect() {
    this.ws = new WebSocket('ws://localhost:8081/ws');
    
    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      this.authenticate();
    };
    
    this.ws.onclose = () => {
      this.handleReconnect();
    };
  }
  
  private handleReconnect() {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      setTimeout(() => {
        this.reconnectAttempts++;
        this.connect();
      }, Math.pow(2, this.reconnectAttempts) * 1000);
    }
  }
}
```

#### 4. Error Handling
```typescript
class ApiError extends Error {
  constructor(
    public code: string,
    message: string,
    public statusCode: number,
    public details?: any
  ) {
    super(message);
  }
}

async function handleApiError(response: Response) {
  if (!response.ok) {
    const error = await response.json();
    throw new ApiError(
      error.error.code,
      error.error.message,
      response.status,
      error.error.details
    );
  }
}
```

#### 5. Caching Strategy
```typescript
class ApiCache {
  private cache = new Map<string, { data: any; timestamp: number }>();
  private ttl = 30000; // 30 seconds
  
  get(key: string) {
    const item = this.cache.get(key);
    if (item && Date.now() - item.timestamp < this.ttl) {
      return item.data;
    }
    return null;
  }
  
  set(key: string, data: any) {
    this.cache.set(key, { data, timestamp: Date.now() });
  }
}
```

---

## Component Integration Examples

### 1. Account Summary Component

```typescript
import React, { useState, useEffect } from 'react';

interface AccountSummaryProps {
  accountId: number;
}

export const AccountSummary: React.FC<AccountSummaryProps> = ({ accountId }) => {
  const [summary, setSummary] = useState<AccountSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchSummary = async () => {
      try {
        const response = await fetch(
          `http://localhost:8083/api/account/summary?account_id=${accountId}`
        );
        
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        setSummary(data);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error');
      } finally {
        setLoading(false);
      }
    };

    fetchSummary();
    
    // Poll for updates every second
    const interval = setInterval(fetchSummary, 1000);
    return () => clearInterval(interval);
  }, [accountId]);

  if (loading) return <div>Loading...</div>;
  if (error) return <div>Error: {error}</div>;
  if (!summary) return <div>No data</div>;

  return (
    <div className="account-summary">
      <h2>Account Summary</h2>
      <div className="balance">
        <span>Balance: ${(summary.balance / 100).toFixed(2)}</span>
      </div>
      <div className="equity">
        <span>Total Equity: ${(summary.total_equity / 100).toFixed(2)}</span>
      </div>
      <div className="day-change">
        <span className={summary.day_change >= 0 ? 'positive' : 'negative'}>
          Day Change: ${(summary.day_change / 100).toFixed(2)} ({summary.day_change_percent.toFixed(2)}%)
        </span>
      </div>
    </div>
  );
};
```

### 2. Price Chart Component

```typescript
import React, { useState, useEffect } from 'react';
import { Line } from 'react-chartjs-2';

interface PriceChartProps {
  symbolId: number;
  period?: string;
  interval?: string;
}

export const PriceChart: React.FC<PriceChartProps> = ({ 
  symbolId, 
  period = '1d', 
  interval = '5m' 
}) => {
  const [priceData, setPriceData] = useState<PriceHistory | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchPriceData = async () => {
      try {
        const response = await fetch(
          `http://localhost:8083/api/price-history/${symbolId}?period=${period}&interval=${interval}`
        );
        
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        setPriceData(data);
      } catch (err) {
        console.error('Failed to fetch price data:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchPriceData();
    
    // Poll for updates every 5 seconds
    const interval = setInterval(fetchPriceData, 5000);
    return () => clearInterval(interval);
  }, [symbolId, period, interval]);

  if (loading) return <div>Loading chart...</div>;
  if (!priceData) return <div>No price data available</div>;

  const chartData = {
    labels: priceData.candles.map(candle => 
      new Date(candle.timestamp).toLocaleTimeString()
    ),
    datasets: [{
      label: 'Price',
      data: priceData.candles.map(candle => candle.close / 100),
      borderColor: 'rgb(75, 192, 192)',
      tension: 0.1
    }]
  };

  return (
    <div className="price-chart">
      <h3>Price History - {period} ({interval})</h3>
      <Line data={chartData} />
    </div>
  );
};
```

### 3. Holdings List Component

```typescript
import React, { useState, useEffect } from 'react';

interface HoldingsListProps {
  accountId: number;
}

export const HoldingsList: React.FC<HoldingsListProps> = ({ accountId }) => {
  const [positions, setPositions] = useState<Position[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchPositions = async () => {
      try {
        // Use WebSocket to get positions
        const ws = new WebSocket('ws://localhost:8081/ws');
        
        ws.onopen = () => {
          // Authenticate first
          ws.send(JSON.stringify({
            id: '1',
            method: 'auth.login',
            params: {
              api_key: 'ak_test_1234567890abcdef',
              api_secret: 'sk_test_abcdef1234567890'
            }
          }));
        };

        ws.onmessage = (event) => {
          const message = JSON.parse(event.data);
          
          if (message.id === '1' && message.result?.authenticated) {
            // Get positions after authentication
            ws.send(JSON.stringify({
              id: '2',
              method: 'account.positions',
              params: {}
            }));
          }
          
          if (message.id === '2' && message.result?.positions) {
            setPositions(message.result.positions);
            setLoading(false);
            ws.close();
          }
        };
      } catch (err) {
        console.error('Failed to fetch positions:', err);
        setLoading(false);
      }
    };

    fetchPositions();
  }, [accountId]);

  if (loading) return <div>Loading holdings...</div>;

  return (
    <div className="holdings-list">
      <h3>Holdings</h3>
      {positions.length === 0 ? (
        <div>No positions</div>
      ) : (
        <div className="positions">
          {positions.map((position) => (
            <div key={position.symbol_id} className="position">
              <div className="symbol">Symbol: {position.symbol_id}</div>
              <div className="quantity">
                Shares: {(position.quantity / 10000).toFixed(4)}
              </div>
              <div className="avg-cost">
                Avg Cost: ${(position.avg_cost / 100).toFixed(2)}
              </div>
              <div className="current-price">
                Current: ${(position.current_price / 100).toFixed(2)}
              </div>
              <div className={`pnl ${position.unrealized_pnl >= 0 ? 'positive' : 'negative'}`}>
                Unrealized P&L: ${(position.unrealized_pnl / 100).toFixed(2)}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
```

### 4. Order Book Component

```typescript
import React, { useState, useEffect } from 'react';

interface OrderBookProps {
  symbolId: number;
}

export const OrderBook: React.FC<OrderBookProps> = ({ symbolId }) => {
  const [orderBook, setOrderBook] = useState<OrderBook | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchOrderBook = async () => {
      try {
        const response = await fetch('http://localhost:8083/api/snapshot/current');
        
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        const symbolOrderBook = data.state.order_books[symbolId.toString()];
        
        if (symbolOrderBook) {
          setOrderBook(symbolOrderBook);
        }
      } catch (err) {
        console.error('Failed to fetch order book:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchOrderBook();
    
    // Poll for updates every second
    const interval = setInterval(fetchOrderBook, 1000);
    return () => clearInterval(interval);
  }, [symbolId]);

  if (loading) return <div>Loading order book...</div>;
  if (!orderBook) return <div>No order book data</div>;

  const buyOrders = Object.entries(orderBook.buy_orders)
    .map(([price, quantity]) => ({ price: parseInt(price), quantity }))
    .sort((a, b) => b.price - a.price)
    .slice(0, 10);

  const sellOrders = Object.entries(orderBook.sell_orders)
    .map(([price, quantity]) => ({ price: parseInt(price), quantity }))
    .sort((a, b) => a.price - b.price)
    .slice(0, 10);

  return (
    <div className="order-book">
      <h3>Order Book - Symbol {symbolId}</h3>
      
      <div className="order-book-content">
        <div className="sell-orders">
          <h4>Sell Orders</h4>
          {sellOrders.map((order, index) => (
            <div key={index} className="order-row sell">
              <span className="price">${(order.price / 100).toFixed(2)}</span>
              <span className="quantity">{order.quantity}</span>
            </div>
          ))}
        </div>
        
        <div className="spread">
          {orderBook.last_trade_price && (
            <div className="last-trade">
              Last: ${(orderBook.last_trade_price / 100).toFixed(2)}
            </div>
          )}
        </div>
        
        <div className="buy-orders">
          <h4>Buy Orders</h4>
          {buyOrders.map((order, index) => (
            <div key={index} className="order-row buy">
              <span className="price">${(order.price / 100).toFixed(2)}</span>
              <span className="quantity">{order.quantity}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
```

### 5. Order Placement Component

```typescript
import React, { useState } from 'react';

interface OrderFormProps {
  symbolId: number;
  onOrderPlaced: (order: any) => void;
}

export const OrderForm: React.FC<OrderFormProps> = ({ symbolId, onOrderPlaced }) => {
  const [side, setSide] = useState<'BUY' | 'SELL'>('BUY');
  const [orderType, setOrderType] = useState<'LIMIT' | 'MARKET'>('LIMIT');
  const [price, setPrice] = useState<number>(0);
  const [quantity, setQuantity] = useState<number>(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const placeOrder = async () => {
    setLoading(true);
    setError(null);

    try {
      const ws = new WebSocket('ws://localhost:8081/ws');
      
      ws.onopen = () => {
        // Authenticate
        ws.send(JSON.stringify({
          id: '1',
          method: 'auth.login',
          params: {
            api_key: 'ak_test_1234567890abcdef',
            api_secret: 'sk_test_abcdef1234567890'
          }
        }));
      };

      ws.onmessage = (event) => {
        const message = JSON.parse(event.data);
        
        if (message.id === '1' && message.result?.authenticated) {
          // Place order after authentication
          ws.send(JSON.stringify({
            id: '2',
            method: 'order.submit',
            params: {
              symbol: symbolId.toString(),
              side,
              type: orderType,
              price: orderType === 'LIMIT' ? price : 0,
              quantity,
              client_order_id: null
            }
          }));
        }
        
        if (message.id === '2') {
          if (message.error) {
            setError(message.error.message);
          } else {
            onOrderPlaced(message.result);
            // Reset form
            setPrice(0);
            setQuantity(0);
          }
          setLoading(false);
          ws.close();
        }
      };

      ws.onerror = () => {
        setError('WebSocket connection failed');
        setLoading(false);
      };
    } catch (err) {
      setError('Failed to place order');
      setLoading(false);
    }
  };

  return (
    <div className="order-form">
      <h3>Place Order - Symbol {symbolId}</h3>
      
      <div className="form-group">
        <label>Side:</label>
        <select value={side} onChange={(e) => setSide(e.target.value as 'BUY' | 'SELL')}>
          <option value="BUY">Buy</option>
          <option value="SELL">Sell</option>
        </select>
      </div>
      
      <div className="form-group">
        <label>Type:</label>
        <select value={orderType} onChange={(e) => setOrderType(e.target.value as 'LIMIT' | 'MARKET')}>
          <option value="LIMIT">Limit</option>
          <option value="MARKET">Market</option>
        </select>
      </div>
      
      {orderType === 'LIMIT' && (
        <div className="form-group">
          <label>Price ($):</label>
          <input
            type="number"
            step="0.01"
            value={price / 100}
            onChange={(e) => setPrice(Math.round(parseFloat(e.target.value) * 100))}
          />
        </div>
      )}
      
      <div className="form-group">
        <label>Quantity:</label>
        <input
          type="number"
          value={quantity}
          onChange={(e) => setQuantity(parseInt(e.target.value))}
        />
      </div>
      
      {error && <div className="error">{error}</div>}
      
      <button 
        onClick={placeOrder} 
        disabled={loading || quantity <= 0 || (orderType === 'LIMIT' && price <= 0)}
      >
        {loading ? 'Placing Order...' : 'Place Order'}
      </button>
    </div>
  );
};
```

---

## Testing

### Test Environment
- **Base URL**: `http://localhost:8083/api`
- **WebSocket URL**: `ws://localhost:8081/ws`
- **Test Accounts**: Use the provided test API keys above

### Sample Test Script
```typescript
// Test script for API endpoints
async function testApiEndpoints() {
  const baseUrl = 'http://localhost:8083/api';
  
  try {
    // Test symbol info
    const symbolResponse = await fetch(`${baseUrl}/symbol/764/info`);
    console.log('Symbol Info:', await symbolResponse.json());
    
    // Test price history
    const priceResponse = await fetch(`${baseUrl}/price-history/764?period=1d&interval=5m`);
    console.log('Price History:', await priceResponse.json());
    
    // Test account summary
    const accountResponse = await fetch(`${baseUrl}/account/summary?account_id=8`);
    console.log('Account Summary:', await accountResponse.json());
    
    // Test market snapshot
    const snapshotResponse = await fetch(`${baseUrl}/snapshot/current`);
    console.log('Market Snapshot:', await snapshotResponse.json());
    
  } catch (error) {
    console.error('API Test Failed:', error);
  }
}

testApiEndpoints();
```

---

## Support

For technical support or questions about the API:

1. **Documentation**: This document contains all necessary information
2. **Test Environment**: Use the provided test accounts and endpoints
3. **Error Handling**: Follow the error response format for debugging
4. **Rate Limits**: Respect the rate limits to avoid throttling

---

**Last Updated**: September 26, 2025  
**API Version**: 1.0  
**Documentation Version**: 1.0
