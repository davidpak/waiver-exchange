// API Types for Waiver Exchange Trading Platform

// ============================================================================
// REST API Types
// ============================================================================

export interface SymbolInfoResponse {
  player_id: string;
  name: string;
  position: string;
  team: string;
  projected_points: number;
  rank: number;
  symbol_id: number;
  last_updated: string;
}

export interface CurrentPriceResponse {
  symbol_id: number;
  price: number;
  source: string;
  last_updated: string;
}

export interface PriceHistoryResponse {
  symbol_id: number;
  period: string;
  interval: string;
  candles: CandleData[];
  total_candles: number;
}

export interface CandleData {
  timestamp: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface AccountSummaryResponse {
  account_id: number;
  balance: number;             // In cents
  total_equity: number;        // In cents
  position_value: number;      // In cents
  day_change: number;          // In cents
  day_change_percent: number;  // Percentage
  buying_power: number;        // In cents
  unrealized_pnl: number;      // In cents
  realized_pnl: number;        // In cents
  last_updated: string;
}

export interface EquityHistoryResponse {
  account_id: number;
  snapshots: EquitySnapshot[];
  total_days: number;
}

export interface EquitySnapshot {
  date: string;                // YYYY-MM-DD format
  total_equity: number;        // In cents
  cash_balance: number;        // In cents
  position_value: number;      // In cents
  unrealized_pnl: number;      // In cents
  realized_pnl: number;        // In cents
  day_change: number;          // In cents
  day_change_percent: number;  // Percentage
}

export interface SnapshotResponse {
  id: string;
  timestamp: string;
  tick: number;
  state: {
    order_books: Record<string, OrderBookState>;
    accounts: Record<string, any>;
    active_symbols: number[];
    config: any;
    stats: any;
  };
  metadata: {
    version: string;
    created_at: string;
  };
}

export interface OrderBookState {
  symbol_id: number;
  buy_orders: [number, number][];  // [price, quantity]
  sell_orders: [number, number][]; // [price, quantity]
  last_trade_price: number | null;
  last_trade_quantity: number | null;
  last_trade_timestamp: string | null;
}

// ============================================================================
// WebSocket API Types
// ============================================================================

export interface WebSocketMessage {
  id?: string;
  method?: string;
  stream?: string;
  params?: any;
  data?: any;
  result?: any;
  error?: WebSocketError;
}

export interface WebSocketError {
  code: number;
  message: string;
  details?: any;
}

export interface OrderPlaceRequest {
  symbol: string;       // Player name (e.g., "Josh Allen")
  side: 'BUY' | 'SELL';
  type: 'LIMIT' | 'MARKET' | 'IOC' | 'FOK';
  price: number;        // In cents
  quantity: number;     // Number of shares
  client_order_id?: string;
}

export interface OrderPlaceResponse {
  order_id: string;
  status: 'ACCEPTED' | 'REJECTED' | 'PENDING';
  timestamp: number;
  client_order_id?: string;
}

export interface AuthRequest {
  api_key: string;
  api_secret: string;
}

export interface AuthResponse {
  authenticated: boolean;
  user_id?: string;
  account_id?: number;
  permissions?: string[];
}

// ============================================================================
// Error Types
// ============================================================================

export interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: any;
  };
  timestamp: string;
}

// ============================================================================
// Utility Types
// ============================================================================

export type Timeframe = '1d' | '1w' | '1m' | '3m' | '1y' | '5y';
export type OrderSide = 'BUY' | 'SELL';
export type OrderType = 'LIMIT' | 'MARKET' | 'IOC' | 'FOK';

// ============================================================================
// API Configuration
// ============================================================================

export const API_CONFIG = {
  REST_BASE_URL: 'http://localhost:8083/api',
  WS_BASE_URL: 'ws://localhost:8081/ws',
  POLLING_INTERVAL: 1000, // 1 second
} as const;
