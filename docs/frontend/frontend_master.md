
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
- **Charts:** TradingView Lightweight Charts (canvas)  
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
- **Order Book Data:** Latest snapshots (fast, real-time)
- **Account Data:** PostgreSQL database via AccountService (reliable, complete)
- **Historical Data:** Price history table (proper time-series data)
- **Player Metadata:** Sleeper API + Redis cache (cached for performance)
- **Real-time Updates:** WebSocket streams (price updates, order fills)  

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
- **Total Equity:** Currency balance + positions value
- **Day Change:** Today's P&L with percentage and dollar amount
- **Buying Power:** Available cash for trading
- **Mini Chart:** Simple line chart of account value over time
- **Data Source:** Database via AccountService
- **SLA:** Update visible in ≤ 150ms after data refresh

### 6.2 Holdings List  
- **User Positions:** Symbol name, quantity, current price, market value
- **Day Change:** Per-position P&L with percentage
- **Total P&L:** Unrealized gains/losses
- **Data Source:** Database positions + current prices from snapshots
- **SLA:** 1000+ rows at ≥ 60fps scroll

### 6.3 Symbol View (Center Component)
- **Header:** Symbol name, position, team, current price, day change, 24h high/low
- **Chart:** Candlestick chart with 1D/1W/1M/3M/1Y timeframes
- **Order Form:** Buy/sell buttons with quantity/price inputs
- **Data Sources:** Sleeper API (metadata), snapshots (current price), price history (chart)
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

## 7. Performance & SLAs

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
- `GET /api/account/summary` - Account balance, total equity, day change
- `GET /api/account/positions` - User positions with current market values  
- `GET /api/account/trades` - Trade history

**Market Data:**
- `GET /api/snapshot/current` - Latest order book data from snapshots
- `GET /api/price-history/{symbol}?period=1d&interval=1m` - Historical price data
- `GET /api/symbol/{symbol}/info` - Player metadata from Sleeper API

**Order Management:**
- `POST /api/orders/place` - Place buy/sell orders
- `GET /api/orders/active` - Active orders
- `DELETE /api/orders/{order_id}` - Cancel orders

### 8.2 WebSocket Streams (Optional)

**Real-time Updates:**
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

## 17. Done-When (MVP Acceptance)

- User can view account summary with real-time balance and P&L.  
- User can see holdings list with current market values.  
- User can view symbol details with candlestick chart (1D/1W/1M timeframes).  
- User can see live order book with bid/ask levels.  
- User can place buy/sell orders with real-time confirmation.  
- All data updates every second with ≤ 200ms latency.  
- Offline banner shows within 2s of connection drop.  

---
