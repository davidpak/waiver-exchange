
# Waiver Exchange — Frontend Master Specification (v0.1)

**Owner:** David Pak
**Status:** Draft → Accept upon review  
**Scope:** Full architecture, invariants, SLAs, and feature contracts for the Waiver Exchange WebUI.  
**Audience:** Frontend engineers, system architects, reviewers. This is the **source of truth** for UI.  
**Normative language:** MUST / SHOULD / MAY  

---

## 1. Overview & Vision

The Waiver Exchange frontend is the **human interface** into the deterministic, tick-bounded simulation core.  
It MUST uphold the same principles as the backend: **determinism, performance, observability, extensibility**.  

The frontend is not a toy; it is an **engineering showcase** equal to the backend. It renders real-time market microstructure with sub‑100ms responsiveness, while maintaining deterministic replay, user trust, and extensible layouts.

**Fantasy Football Trading Platform**: The frontend provides a professional-grade trading interface for fantasy football players, inspired by Robinhood Legend but focused on fantasy football player trading with real money converted from fantasy points.

---

## 2. Guiding Principles

1. **Deterministic UI State.**  
   - One global “tick-frame pipeline” ensures state updates are batched and rendered at consistent boundaries.  
   - Replay of events yields byte-identical UI traces.  

2. **Zero-Waste Rendering.**  
   - Every re-render is intentional. Virtualization, memoization, and batched updates enforced.  

3. **Resilient & Observable.**  
   - Offline/latency modes clearly surfaced.  
   - Widget error isolation with retries.  

4. **Extensible & Modular.**  
   - Each widget is a plugin with a stable interface: can be developed, tested, and shipped independently.  

5. **Parity with Backend Rigor.**  
   - Frontend SLAs (latency, FPS, determinism) are written, measurable, and enforced in CI.  

---

## 3. Tech Stack

- **React 18 + TypeScript**
- **Mantine** (AppShell, Grid, Modal, Notifications, Spotlight, Drawer)
- **State:** Zustand (domain slices) + TanStack Query (server state)  
- **Data transport:** REST API (primary), WebSocket (real-time updates), HTTP (snapshots/mutations)  
- **Tables:** TanStack Table + react-virtuoso (virtualization)  
- **Charts:** TradingView Lightweight Charts (canvas) - Professional trading charts with real-time updates  
- **Layout:** react-grid-layout (MVP), Golden Layout (future pro mode)  
- **Testing:** Vitest, Playwright, Storybook, Chromatic  
- **Perf tooling:** React Profiler, Web Vitals beacons, custom tick-trace overlay  

---

## 4. System Architecture

### 4.1 Data Flow

```
[Snapshots] → REST API → TanStack Query → Widgets (order book data)
[Database] → REST API → TanStack Query → Widgets (account data)  
[Price History] → REST API → TanStack Query → Widgets (chart data)
[WebSocket] → Real-time updates → Zustand slices → Widgets (live updates)
```

**Hybrid Data Sources:**
- **Order Book Data:** Latest snapshots (fast, real-time) ✅ IMPLEMENTED
- **Account Data:** PostgreSQL database via AccountService (reliable, complete) ✅ IMPLEMENTED
- **Historical Data:** Price history table (proper time-series data) ✅ IMPLEMENTED
- **Player Metadata:** JSON file with assigned symbol IDs (fast, reliable) ✅ IMPLEMENTED
- **Real-time Updates:** REST API polling every 1 second (simple, reliable) ✅ IMPLEMENTED  

### 4.2 Project Structure

```
/src
  /app         # Shell, routes, theming, layout presets
  /core        # api, models, store, hooks, utils
  /features    # watchlist, charts, options-chain, positions, orders, account, snapshots, layout
  /components  # shared UI atoms: StatTile, Sparkline, CardHeader
  /testing     # mocks, fixtures, perf harnesses
```

---

## 5. Layout & Linking

- **Layout Engine:** react-grid-layout; persisted configs `{id, x, y, w, h}`.  
- **Presets:** Default, Multi-Chart, Options Focus, Portfolio.  
- **Link Groups:** Color-coded; widgets in same group share symbol context.  
- **Persistence:** LocalStorage (MVP); account-level sync (future).  
- **Keyboard:** Hotkeys for group cycling, save/load, focus mode.  

---

## 6. Widgets (MVP Scope)

### 6.1 Account Summary
- **Total Equity:** Currency balance + positions value (large, prominent display)
- **Day Change:** Today's P&L with percentage and dollar amount (smaller, under equity)
- **Buying Power:** Available cash for trading
- **Interactive Mini Chart:** TradingView LineSeries chart of account equity over time
  - **Hover Tooltip:** Shows equity value and change percentage/amount for that time period
  - **Time Ranges:** 1D, 1W, 1M, 1Y with built-in range switcher
  - **Interactive:** Hover shows change from start of selected period to hover point
  - **Real-time Updates:** Updates every second via `update()` method
- **Data Source:** `/api/account/equity-history` endpoint
- **SLA:** Update visible in ≤ 150ms after data refresh

### 6.2 Holdings List  
- **User Positions:** Symbol name, quantity, current price, market value
- **Day Change:** Per-position P&L with percentage
- **Total P&L:** Unrealized gains/losses
- **Data Source:** Database positions + current prices from snapshots
- **SLA:** 1000+ rows at ≥ 60fps scroll

### 6.3 Symbol View (Center Component)
- **Header:** Symbol name, position, team, current price, day change, 24h high/low
- **TradingView Candlestick Chart:** Professional OHLC chart with multiple timeframes
  - **Chart Type:** CandlestickSeries with volume indicators
  - **Time Ranges:** 1D (5-min candles), 1W (1-hour), 1M (4-hour), 1Y (daily)
  - **Real-time Updates:** New candles streamed via `update()` method
  - **Interactive Features:** Zoom, pan, crosshair, price lines
  - **Theme Integration:** Dark/light mode support
- **Order Form:** Buy/sell buttons with quantity/price inputs
- **Data Sources:** `/api/symbol/{id}/info` (metadata), `/api/price-history/{id}` (chart data)
- **SLA:** ≤ 16ms per redraw; linked updates ≤ 100ms

### 6.4 Order Book (Side Component)
- **Live Bid/Ask Display:** Real-time order book with price levels
- **Volume Indicators:** Quantity at each price level
- **Best Bid/Ask:** Highlighted top of book
- **Data Source:** Latest snapshots
- **SLA:** Update visible in ≤ 100ms after snapshot refresh

### 6.5 Orders
- **Tabs:** Open, Filled, Canceled
- **Lifecycle Updates:** Real-time order status changes
- **Notifications:** Toasts on Accepted/Filled
- **Data Source:** Database + WebSocket updates
- **SLA:** Order status updates ≤ 200ms

### 6.6 Player Watchlist (Future)
- **Fantasy Football Players:** Sortable list of available players
- **Quick Actions:** Buy/Sell buttons
- **Price Alerts:** Notifications for price movements
- **Data Source:** Sleeper API + current prices
- **SLA:** 1000+ rows at ≥ 60fps scroll  

---

## 7. TradingView Lightweight Charts Integration

### 7.1 Chart Library Overview
- **Library:** TradingView Lightweight Charts v5.0
- **Size:** 35KB total (production build)
- **Technology:** HTML5 Canvas for high performance
- **Features:** Real-time updates, responsive design, accessibility compliant
- **Installation:** `npm install --save lightweight-charts`

### 7.2 Chart Types & Usage

**Account Summary Mini-Chart:**
```typescript
import { createChart, LineSeries } from 'lightweight-charts';

const chart = createChart(container, {
  width: 300,
  height: 200,
  layout: { background: { color: 'transparent' } },
  grid: { vertLines: { visible: false }, horzLines: { visible: false } },
  timeScale: { visible: false },
  rightPriceScale: { visible: false }
});

const lineSeries = chart.addSeries(LineSeries, {
  color: '#26a69a',
  lineWidth: 2
});
```

**Symbol View Candlestick Chart:**
```typescript
import { createChart, CandlestickSeries } from 'lightweight-charts';

const chart = createChart(container, {
  width: 800,
  height: 400,
  layout: { background: { color: 'var(--mantine-color-body)' } },
  grid: { vertLines: { color: 'var(--mantine-color-dimmed)' } },
  timeScale: { timeVisible: true, secondsVisible: false },
  rightPriceScale: { borderColor: 'var(--mantine-color-dimmed)' }
});

const candlestickSeries = chart.addSeries(CandlestickSeries, {
  upColor: '#26a69a',
  downColor: '#ef5350',
  borderVisible: false,
  wickUpColor: '#26a69a',
  wickDownColor: '#ef5350'
});
```

### 7.3 Data Format & Transformation

**Equity History Data (Account Summary):**
```typescript
// API Response: /api/account/equity-history
interface EquityHistoryPoint {
  date: string;        // "2025-01-15"
  total_equity: number; // 125000 (cents)
  day_change: number;   // 2500 (cents)
  day_change_percent: number; // 2.04
}

// Transform to TradingView format
const chartData = equityHistory.map(point => ({
  time: point.date,           // TradingView accepts date strings
  value: point.total_equity / 100  // Convert cents to dollars
}));
```

**Price History Data (Symbol View):**
```typescript
// API Response: /api/price-history/{symbolId}?period=1d&interval=5m
interface PriceHistoryPoint {
  timestamp: string;    // "2025-01-15T09:30:00Z"
  open: number;         // 35000 (cents)
  high: number;         // 35500 (cents)
  low: number;          // 34800 (cents)
  close: number;        // 35200 (cents)
  volume: number;       // 1000
}

// Transform to TradingView format
const chartData = priceHistory.map(point => ({
  time: point.timestamp,
  open: point.open / 100,    // Convert cents to dollars
  high: point.high / 100,
  low: point.low / 100,
  close: point.close / 100
}));
```

### 7.4 Real-time Updates

**Account Summary Updates:**
```typescript
// Update every second with new equity data
const updateAccountChart = (newEquityData: EquityHistoryPoint) => {
  const chartPoint = {
    time: newEquityData.date,
    value: newEquityData.total_equity / 100
  };
  
  // Update last point or add new point
  lineSeries.update(chartPoint);
};
```

**Symbol View Updates:**
```typescript
// Update with new candlestick data
const updatePriceChart = (newCandle: PriceHistoryPoint) => {
  const candleData = {
    time: newCandle.timestamp,
    open: newCandle.open / 100,
    high: newCandle.high / 100,
    low: newCandle.low / 100,
    close: newCandle.close / 100
  };
  
  // Update last candle or add new candle
  candlestickSeries.update(candleData);
};
```

### 7.5 Interactive Features

**Hover Tooltips (Account Summary):**
- **Equity Value:** Display current equity at hover point
- **Change Calculation:** Show change from period start to hover point
- **Time Period:** Display date/time of hover point
- **Percentage Change:** Calculate and display percentage change

**Range Switching:**
- **Built-in Controls:** TradingView provides range switcher UI
- **Time Periods:** 1D, 1W, 1M, 1Y for account summary
- **Candle Intervals:** 5-min (1D), 1-hour (1W), 4-hour (1M), daily (1Y) for symbol view

### 7.6 Theme Integration

**Dark/Light Mode Support:**
```typescript
const chartOptions = {
  layout: {
    background: { color: 'var(--mantine-color-body)' },
    textColor: 'var(--mantine-color-text)'
  },
  grid: {
    vertLines: { color: 'var(--mantine-color-dimmed)' },
    horzLines: { color: 'var(--mantine-color-dimmed)' }
  },
  timeScale: {
    borderColor: 'var(--mantine-color-dimmed)',
    timeVisible: true,
    secondsVisible: false
  },
  rightPriceScale: {
    borderColor: 'var(--mantine-color-dimmed)',
    textColor: 'var(--mantine-color-text)'
  }
};
```

### 7.7 Performance Considerations

**Optimization Strategies:**
- **Canvas Rendering:** 35KB library with HTML5 Canvas for 60fps performance
- **Data Batching:** Update charts in batches to avoid excessive re-renders
- **Memory Management:** Limit historical data points to prevent memory leaks
- **Responsive Design:** Charts automatically resize with container

**Update Frequency:**
- **Account Summary:** Update every 1 second with new equity data
- **Symbol View:** Update every 1 second with new price data
- **Chart Redraws:** ≤ 16ms per frame target maintained

---

## 8. Performance & SLAs

| Metric | Target |
| ------ | ------ |
| Account summary update | ≤ 150ms after data refresh |
| Holdings list scroll | ≥ 60fps at 1000 rows |
| Symbol view chart redraw | ≤ 16ms per frame |
| Order book update | ≤ 100ms after snapshot refresh |
| Order status updates | ≤ 200ms end-to-end |
| Price history loading | ≤ 500ms for 1-day data |
| Bundle size (initial) | ≤ 180KB gzipped |
| Data refresh frequency | Every 1 second |

**Update Strategy:** REST API polling every second provides optimal balance of real-time feel and system performance.

Perf budgets enforced in CI via Playwright + Web Vitals beacons.  

---

## 8. API Specifications

### 8.1 REST Endpoints

**Account Data:**
- `GET /api/account/summary` - Account balance, total equity, day change ✅ IMPLEMENTED
- `GET /api/account/equity-history` - Historical equity performance ✅ IMPLEMENTED
- `WebSocket: account.info` - Account balance and equity ✅ IMPLEMENTED
- `WebSocket: account.positions` - User positions with current market values ✅ IMPLEMENTED
- `WebSocket: account.trades` - Trade history ✅ IMPLEMENTED

**Market Data:**
- `GET /api/snapshot/current` - Latest order book data from snapshots ✅ IMPLEMENTED
- `GET /api/price-history/{symbolId}?period=1d&interval=5m` - Historical price data ✅ IMPLEMENTED
- `GET /api/symbol/{symbolId}/info` - Player metadata from JSON file ✅ IMPLEMENTED

**Order Management:**
- `WebSocket: order.place` - Place buy/sell orders ✅ IMPLEMENTED
- `WebSocket: order.submit` - Alternative order placement ✅ IMPLEMENTED
- `WebSocket: market_data.subscribe` - Real-time market data ✅ IMPLEMENTED

**Admin/System:**
- `POST /api/admin/create-snapshots` - Manually trigger daily equity snapshots ✅ IMPLEMENTED
- `POST /api/admin/test-scheduler` - Check scheduler status ✅ IMPLEMENTED

### 8.2 WebSocket API (Primary for Trading)

**Authentication:**
- `auth` - API key authentication
- `auth.jwt` - JWT token authentication

**Order Management:**
- `order.place` - Place buy/sell orders (primary)
- `order.submit` - Alternative order placement
- **Request Format:**
  ```json
  {
    "id": "1",
    "method": "order.place",
    "params": {
      "symbol": "Josh Allen",
      "side": "BUY",
      "type": "LIMIT",
      "price": 35000,
      "quantity": 100,
      "client_order_id": "my_order_1"
    }
  }
  ```
- **Response Format:**
  ```json
  {
    "id": "1",
    "result": {
      "order_id": "ord_123456789",
      "status": "ACCEPTED",
      "timestamp": 1640995200000,
      "client_order_id": "my_order_1"
    }
  }
  ```

**Account Data:**
- `account.info` - Account balance and equity
- `account.positions` - User positions with current market values
- `account.trades` - Trade history
- `account.setup_sleeper` - Setup Sleeper integration
- `account.select_league` - Select fantasy league

**Market Data:**
- `market_data.subscribe` - Subscribe to real-time market data
- **Real-time Updates:**
  - `price_update` - Live price changes
  - `order_fill` - Order execution notifications
  - `account_update` - Balance/position changes

### 8.3 Data Refresh Strategy

**Primary:** REST API polling every 1 second
**Secondary:** WebSocket for critical real-time updates
**Fallback:** Stale data indicators with retry logic

---

## 9. Reliability & Fault Tolerance

- **Offline mode:** Banner + stale data markers.  
- **Reconnect:** Exponential backoff; stale badge ≤ 2s after drop.  
- **Widget isolation:** Each wrapped in ErrorBoundary.  
- **Backpressure:** Drop intermediate UI frames; show last-known-good.  

---

## 10. Observability

- **Event trace overlay:** Shows tick arrivals, commit times, dropped frames.  
- **Perf metrics:** FID, FPS, commit counts, hydration time.  
- **User metrics:** click-to-update latency, order-ack timing.  
- **Telemetry:** opt-in, anonymized.  

---

## 11. Security

- WebSocket auth via backend session token.  
- No PII beyond account_id.  
- CSRF protection on HTTP mutations.  
- Content Security Policy enforced.  

---

## 12. Developer Experience

- **Storybook:** Each widget with data simulators (spikes, halts).  
- **CI:** Type check, lint, unit + e2e, perf budgets.  
- **Contracts as code:** Backend schemas → TS types via codegen.  
- **Visual regression:** Chromatic; baseline images per widget.  

---

## 13. Extensibility

- **Widget SDK:** `registerWidget({id, schema, dataDeps})`.  
- **Indicator SDK (future):** Pure functions over OHLC stream, worker-executed.  
- **Layout import/export:** JSON with checksum.  

---

## 14. Accessibility

- Full keyboard nav.  
- Dark-first theme, color-blind safe palettes.  
- Reduced motion mode.  

---

## 15. Roadmap (90-Day)

**Day 30 — Foundation**  
- AppShell, Layout, Link groups.  
- Account Summary + Holdings List v1.  
- Symbol View + Order Book components.  

**Day 60 — Depth**  
- Price history system + candlestick charts.  
- Order management + real-time updates.  
- Player metadata integration.  

**Day 90 — Showcase**  
- Advanced chart features (indicators, timeframes).  
- Player watchlist + price alerts.  
- Performance optimization + polish.  

---

## 16. Invariants

1. All UI state changes attributable to a data refresh cycle.  
2. Exactly one symbol context per link group at a time.  
3. One ErrorBoundary per widget; fault does not cascade.  
4. Data consistency maintained across all widgets.  
5. No widget re-renders more than once per refresh cycle unless user interaction.  

---

## 17. Implementation Status & Next Steps

### 17.1 ✅ Ready for Frontend Development (Phase 1)

**Core APIs Implemented:**
- ✅ **Account Summary**: `GET /api/account/summary` - Live balance, equity, day change
- ✅ **Equity History**: `GET /api/account/equity-history` - Historical performance data
- ✅ **Symbol Info**: `GET /api/symbol/{symbolId}/info` - Player metadata with symbol IDs
- ✅ **Price History**: `GET /api/price-history/{symbolId}?period=1d&interval=5m` - OHLC chart data
- ✅ **Live Market Data**: `GET /api/snapshot/current` - Real-time order book data
- ✅ **System Health**: `POST /api/admin/test-scheduler` - Scheduler status monitoring

**Data Sources Ready:**
- ✅ **467 Players** with assigned symbol IDs in JSON file
- ✅ **Price History** table with OHLC data for charts
- ✅ **Daily Equity Snapshots** for performance tracking
- ✅ **Live Order Book** data from Whistle engine
- ✅ **Account Data** with real balance and equity calculations

**Frontend Components Ready to Build:**
- ✅ **Account Summary Widget** - Balance, equity, day change display
- ✅ **Symbol View Component** - Player info, charts, order buttons
- ✅ **Price History Charts** - Candlestick charts with multiple timeframes
- ✅ **Live Market Data** - Real-time price updates via polling
- ✅ **Equity Performance Charts** - Historical account performance

### 17.2 ✅ Phase 2: Order Management (READY)

**WebSocket APIs Available:**
- ✅ **Order Placement**: `WebSocket: order.place` - Buy/sell order submission
- ✅ **Account Positions**: `WebSocket: account.positions` - Current holdings
- ✅ **Trade History**: `WebSocket: account.trades` - Past transactions
- ✅ **Account Info**: `WebSocket: account.info` - Balance and equity
- ✅ **Real-time Updates**: `WebSocket: market_data.subscribe` - Live market data

**Frontend Components Ready to Build:**
- ✅ **Order Modal** - Buy/sell order form (WebSocket integration)
- ✅ **Holdings List** - User positions display (WebSocket data)
- ✅ **Order History** - Past orders and trades (WebSocket data)
- ✅ **Order Book Widget** - Live bid/ask display (WebSocket streams)

### 17.3 🚀 Development Priority

**Start with Phase 1 (Ready Now):**
1. **Account Summary Component** - Display balance and equity
2. **Symbol View Component** - Player info and charts
3. **Price History Charts** - Candlestick visualization
4. **Real-time Updates** - 1-second polling implementation

**Then Phase 2 (Ready Now):**
1. **Order Management** - Buy/sell functionality via WebSocket
2. **Holdings Display** - User positions via WebSocket
3. **Order Book** - Live market data via WebSocket
4. **Trade History** - Transaction records via WebSocket

### 17.4 🛠️ Quick Start Guide

**1. Start Backend Server:**
```bash
cd engine/order-gateway
$env:DATABASE_URL="postgresql://postgres:password@localhost/waiver_exchange"
cargo run --bin rest_server
```

**2. Test REST APIs:**
```bash
# Account Summary
curl "http://localhost:8081/api/account/summary?account_id=1"

# Symbol Info (Josh Allen)
curl "http://localhost:8081/api/symbol/764/info"

# Price History
curl "http://localhost:8081/api/price-history/764?period=1d&interval=5m"

# Live Market Data
curl "http://localhost:8081/api/snapshot/current"
```

**3. Test WebSocket APIs:**
```bash
# Connect to WebSocket
wscat -c ws://localhost:8081/ws

# Authenticate
{"id": "1", "method": "auth", "params": {"api_key": "test_key", "api_secret": "test_secret"}}

# Place Order
{"id": "2", "method": "order.place", "params": {"symbol": "Josh Allen", "side": "BUY", "type": "LIMIT", "price": 35000, "quantity": 100}}

# Get Account Info
{"id": "3", "method": "account.info", "params": {}}

# Get Positions
{"id": "4", "method": "account.positions", "params": {}}
```

**4. Frontend Setup:**
```bash
# Create React app
npx create-react-app waiver-exchange-frontend --template typescript
cd waiver-exchange-frontend

# Install dependencies
npm install @mantine/core @mantine/hooks @mantine/notifications
npm install @tanstack/react-query zustand
npm install lightweight-charts react-grid-layout
npm install @types/react-grid-layout
npm install ws @types/ws  # For WebSocket support

# TradingView Lightweight Charts (already included above)
# npm install --save lightweight-charts
```

**5. Key Implementation Notes:**
- **All prices in cents** - Convert to dollars for display (divide by 100)
- **Hybrid approach** - REST for data fetching, WebSocket for trading
- **Symbol IDs** - Use the 467 assigned symbol IDs from the JSON file
- **Error handling** - All APIs return standardized error responses
- **CORS enabled** - Backend allows all origins for development
- **WebSocket authentication** - Use `auth` method with API key/secret
- **Order placement** - Use `order.place` WebSocket method for trading
- **TradingView Charts** - Professional charts with real-time updates and interactive tooltips
- **Theme Integration** - Charts automatically adapt to dark/light mode using CSS variables

---

## 18. Account Summary Component Implementation

### 18.1 Component Structure
```typescript
interface AccountSummaryProps {
  accountId: string;
  refreshInterval?: number; // Default: 1000ms
}

interface AccountSummaryData {
  totalEquity: number;        // In cents
  dayChange: number;          // In cents
  dayChangePercent: number;   // Percentage
  buyingPower: number;        // In cents
  equityHistory: EquityHistoryPoint[];
}
```

### 18.2 Data Flow
1. **Initial Load:** Fetch account summary and equity history
2. **Real-time Updates:** Poll every 1 second for new data
3. **Chart Updates:** Update TradingView chart with new equity data
4. **UI Updates:** Update displayed values with new data

### 18.3 TradingView Chart Integration
```typescript
// Account Summary Mini-Chart Setup
const setupAccountChart = (container: HTMLElement) => {
  const chart = createChart(container, {
    width: 300,
    height: 200,
    layout: { 
      background: { color: 'transparent' },
      textColor: 'var(--mantine-color-text)'
    },
    grid: { 
      vertLines: { visible: false }, 
      horzLines: { visible: false } 
    },
    timeScale: { visible: false },
    rightPriceScale: { visible: false }
  });

  const lineSeries = chart.addSeries(LineSeries, {
    color: 'var(--mantine-color-blue-6)',
    lineWidth: 2
  });

  return { chart, lineSeries };
};
```

### 18.4 Interactive Tooltip Implementation
```typescript
// Custom tooltip for equity change calculation
const createEquityTooltip = (equityHistory: EquityHistoryPoint[]) => {
  return (param: any) => {
    if (!param.point) return null;
    
    const currentPoint = equityHistory[param.pointIndex];
    const startPoint = equityHistory[0];
    
    const change = currentPoint.total_equity - startPoint.total_equity;
    const changePercent = (change / startPoint.total_equity) * 100;
    
    return {
      title: currentPoint.date,
      content: [
        `Equity: $${(currentPoint.total_equity / 100).toFixed(2)}`,
        `Change: $${(change / 100).toFixed(2)} (${changePercent.toFixed(2)}%)`
      ]
    };
  };
};
```

### 18.5 Real-time Update Strategy
```typescript
// Update chart with new equity data
const updateAccountChart = (
  lineSeries: ISeriesApi<'Line'>,
  newData: EquityHistoryPoint
) => {
  const chartPoint = {
    time: newData.date,
    value: newData.total_equity / 100
  };
  
  // Update last point or add new point
  lineSeries.update(chartPoint);
};
```

### 18.6 Component Features
- **Large Equity Display:** Prominent total equity value
- **Day Change:** Smaller display of daily P&L with percentage
- **Buying Power:** Available cash for trading
- **Interactive Chart:** Hover to see equity and change at any point
- **Time Range Switching:** 1D, 1W, 1M, 1Y with built-in controls
- **Real-time Updates:** Chart updates every second
- **Theme Integration:** Automatically adapts to dark/light mode

---

## 19. Done-When (MVP Acceptance)

- User can view account summary with real-time balance and P&L.  
- User can see holdings list with current market values.  
- User can view symbol details with candlestick chart (1D/1W/1M timeframes).  
- User can see live order book with bid/ask levels.  
- User can place buy/sell orders with real-time confirmation.  
- All data updates every second with ≤ 200ms latency.  
- Offline banner shows within 2s of connection drop.  

---
