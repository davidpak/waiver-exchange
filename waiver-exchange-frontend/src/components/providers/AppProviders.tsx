'use client';

import { QueryProvider } from '@/components/providers/QueryProvider';
import { NavigationProvider } from '@/contexts/NavigationContext';
import { TradingProvider } from '@/contexts/TradingContext';
import { NavigationLoader } from '@/components/common/NavigationLoader';
import { usePrefetchMarketData } from '@/hooks/useMarketData';
import { tradingTheme } from '@/styles/theme';
import { MantineProvider } from '@mantine/core';
import type { ReactNode } from 'react';

function MarketDataPrefetch({ children }: { children: ReactNode }) {
  usePrefetchMarketData();
  return <>{children}</>;
}

export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <MantineProvider theme={tradingTheme} forceColorScheme="dark">
      <QueryProvider>
        <MarketDataPrefetch>
          <NavigationProvider>
            <TradingProvider>
              {children}
              <NavigationLoader />
            </TradingProvider>
          </NavigationProvider>
        </MarketDataPrefetch>
      </QueryProvider>
    </MantineProvider>
  );
}
