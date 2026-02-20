'use client';

import { ErrorBoundary } from '@/components/common/ErrorBoundary';
import { Header } from '@/components/layout/Header';
import { TickerBar } from '@/components/layout/TickerBar';
import { MarketTable } from '@/components/widgets/MarketTable';
import { useTrading } from '@/contexts/TradingContext';
import { useNavigation } from '@/contexts/NavigationContext';
import { Box } from '@mantine/core';

export default function MarketOverviewPage() {
  const { navigate } = useNavigation();
  const { setSelectedSymbolId } = useTrading();

  const handleSymbolSelect = (symbolId: number) => {
    setSelectedSymbolId(symbolId);
    navigate('dashboard');
  };

  return (
    <ErrorBoundary>
      <Box style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
        <Header />
        <Box
          className="hide-scrollbar"
          style={{ flex: 1, overflow: 'auto', paddingBottom: 36 }}
        >
          <MarketTable onSymbolSelect={handleSymbolSelect} />
        </Box>
        <TickerBar />
      </Box>
    </ErrorBoundary>
  );
}
