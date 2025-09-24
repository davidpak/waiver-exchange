// Main Application Store using Zustand
import type {
    AccountSummaryResponse,
    OrderBookState,
    SymbolInfoResponse,
    Timeframe
} from '@/types/api';
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';

// ============================================================================
// Store State Types
// ============================================================================

interface AppState {
  // UI State
  selectedSymbolId: number | null;
  selectedTimeframe: Timeframe;
  isLoading: boolean;
  error: string | null;

  // Market Data
  symbolInfo: SymbolInfoResponse | null;
  currentPrice: number | null;
  orderBook: OrderBookState | null;
  priceHistory: any[] | null;

  // Account Data
  accountSummary: AccountSummaryResponse | null;

  // WebSocket Connection
  isWebSocketConnected: boolean;
  lastUpdate: string | null;
}

interface AppActions {
  // UI Actions
  setSelectedSymbol: (symbolId: number | null) => void;
  setSelectedTimeframe: (timeframe: Timeframe) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;

  // Market Data Actions
  setSymbolInfo: (info: SymbolInfoResponse | null) => void;
  setCurrentPrice: (price: number | null) => void;
  setOrderBook: (orderBook: OrderBookState | null) => void;
  setPriceHistory: (history: any[] | null) => void;

  // Account Actions
  setAccountSummary: (summary: AccountSummaryResponse | null) => void;

  // WebSocket Actions
  setWebSocketConnected: (connected: boolean) => void;
  setLastUpdate: (timestamp: string) => void;

  // Combined Actions
  updateMarketData: (data: {
    symbolInfo?: SymbolInfoResponse;
    currentPrice?: number;
    orderBook?: OrderBookState;
    priceHistory?: any[];
  }) => void;
}

// ============================================================================
// Store Implementation
// ============================================================================

export const useAppStore = create<AppState & AppActions>()(
  devtools(
    (set, get) => ({
      // Initial State
      selectedSymbolId: null,
      selectedTimeframe: '1d',
      isLoading: false,
      error: null,

      symbolInfo: null,
      currentPrice: null,
      orderBook: null,
      priceHistory: null,

      accountSummary: null,

      isWebSocketConnected: false,
      lastUpdate: null,

      // UI Actions
      setSelectedSymbol: (symbolId) => set({ selectedSymbolId: symbolId }),
      setSelectedTimeframe: (timeframe) => set({ selectedTimeframe: timeframe }),
      setLoading: (loading) => set({ isLoading: loading }),
      setError: (error) => set({ error }),

      // Market Data Actions
      setSymbolInfo: (info) => set({ symbolInfo: info }),
      setCurrentPrice: (price) => set({ currentPrice: price }),
      setOrderBook: (orderBook) => set({ orderBook }),
      setPriceHistory: (history) => set({ priceHistory: history }),

      // Account Actions
      setAccountSummary: (summary) => set({ accountSummary: summary }),

      // WebSocket Actions
      setWebSocketConnected: (connected) => set({ isWebSocketConnected: connected }),
      setLastUpdate: (timestamp) => set({ lastUpdate: timestamp }),

      // Combined Actions
      updateMarketData: (data) => set((state) => ({
        ...state,
        ...data,
        lastUpdate: new Date().toISOString(),
      })),
    }),
    {
      name: 'waiver-exchange-store',
    }
  )
);

// ============================================================================
// Selectors (for performance optimization)
// ============================================================================

export const useSelectedSymbol = () => useAppStore((state) => state.selectedSymbolId);
export const useSelectedTimeframe = () => useAppStore((state) => state.selectedTimeframe);
export const useIsLoading = () => useAppStore((state) => state.isLoading);
export const useError = () => useAppStore((state) => state.error);

export const useSymbolInfo = () => useAppStore((state) => state.symbolInfo);
export const useCurrentPrice = () => useAppStore((state) => state.currentPrice);
export const useOrderBook = () => useAppStore((state) => state.orderBook);
export const usePriceHistory = () => useAppStore((state) => state.priceHistory);

export const useAccountSummary = () => useAppStore((state) => state.accountSummary);

export const useWebSocketStatus = () => useAppStore((state) => ({
  isConnected: state.isWebSocketConnected,
  lastUpdate: state.lastUpdate,
}));

// ============================================================================
// Computed Values
// ============================================================================

export const useFormattedPrice = () => {
  const currentPrice = useCurrentPrice();
  return currentPrice ? (currentPrice / 100).toFixed(2) : '0.00';
};

export const useFormattedBalance = () => {
  const accountSummary = useAccountSummary();
  return accountSummary ? (accountSummary.balance / 100).toFixed(2) : '0.00';
};

export const useFormattedEquity = () => {
  const accountSummary = useAccountSummary();
  return accountSummary ? (accountSummary.total_equity / 100).toFixed(2) : '0.00';
};

export const useFormattedDayChange = () => {
  const accountSummary = useAccountSummary();
  if (!accountSummary) return { amount: '0.00', percent: '0.00%', isPositive: true };
  
  const amount = (accountSummary.day_change / 100).toFixed(2);
  const percent = accountSummary.day_change_percent.toFixed(2) + '%';
  const isPositive = accountSummary.day_change >= 0;
  
  return { amount, percent, isPositive };
};
