'use client';

import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';

interface TradingContextValue {
  /** Currently selected symbol ID */
  selectedSymbolId: number;
  setSelectedSymbolId: (id: number) => void;

  /** Price to pre-fill in order entry (set when clicking order book level) */
  fillPrice: number | null;
  setFillPrice: (price: number | null) => void;

  /** Whether command palette is open */
  searchOpen: boolean;
  openSearch: () => void;
  closeSearch: () => void;
}

const TradingContext = createContext<TradingContextValue | null>(null);

export function TradingProvider({ children }: { children: ReactNode }) {
  const [selectedSymbolId, setSelectedSymbolId] = useState(764);
  const [fillPrice, setFillPrice] = useState<number | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);

  const openSearch = useCallback(() => setSearchOpen(true), []);
  const closeSearch = useCallback(() => setSearchOpen(false), []);

  return (
    <TradingContext.Provider
      value={{
        selectedSymbolId,
        setSelectedSymbolId,
        fillPrice,
        setFillPrice,
        searchOpen,
        openSearch,
        closeSearch,
      }}
    >
      {children}
    </TradingContext.Provider>
  );
}

export function useTrading() {
  const ctx = useContext(TradingContext);
  if (!ctx) throw new Error('useTrading must be used within TradingProvider');
  return ctx;
}
