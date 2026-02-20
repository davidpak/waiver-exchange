'use client';

import { useTrading } from '@/contexts/TradingContext';
import { useAllPlayers, useBulkPrices } from '@/hooks/useMarketData';
import { formatCents } from '@/utils/format';
import { Box, Text, UnstyledButton } from '@mantine/core';
import { useMemo } from 'react';

interface TickerItem {
  symbol_id: number;
  name: string;
  team: string;
  price: number;
  change_pct: number;
}

export function TickerBar() {
  const { setSelectedSymbolId } = useTrading();

  const { data: allPlayers } = useAllPlayers();
  const { data: bulkPrices } = useBulkPrices(2000);

  const items: TickerItem[] = useMemo(() => {
    if (!allPlayers || !bulkPrices?.prices) return [];

    return allPlayers
      .filter((p: any) => bulkPrices.prices[p.symbol_id.toString()])
      .slice(0, 50)
      .map((p: any) => {
        const price = bulkPrices.prices[p.symbol_id.toString()] || 0;
        return {
          symbol_id: p.symbol_id,
          name: p.name,
          team: p.team,
          price,
          change_pct: 0, // Would need historical data for real change
        };
      });
  }, [allPlayers, bulkPrices]);

  if (items.length === 0) return null;

  const tickerContent = items.map((item) => {
    const isPositive = item.change_pct >= 0;
    const changeColor = item.change_pct === 0
      ? 'var(--mantine-color-dark-2)'
      : isPositive
        ? 'var(--color-profit)'
        : 'var(--color-loss)';

    return (
      <UnstyledButton
        key={item.symbol_id}
        onClick={() => setSelectedSymbolId(item.symbol_id)}
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 8,
          paddingInline: 16,
          whiteSpace: 'nowrap',
          flexShrink: 0,
        }}
      >
        {/* Team + Player name */}
        <Text fz={11} c="dark.2" fw={500}>
          {item.team}
        </Text>
        <Text fz={11} fw={500} c="dark.1">
          {item.name}
        </Text>

        {/* Price */}
        <Text className="mono" fz={11} fw={500} c="dark.0">
          {formatCents(item.price)}
        </Text>

        {/* Change percentage */}
        <Text className="mono" fz={10} fw={500} style={{ color: changeColor }}>
          {isPositive ? '+' : ''}{item.change_pct.toFixed(2)}%
        </Text>
      </UnstyledButton>
    );
  });

  return (
    <Box
      bg="dark.9"
      style={{
        position: 'fixed',
        bottom: 0,
        left: 0,
        right: 0,
        height: 36,
        borderTop: '1px solid var(--border-subtle)',
        overflow: 'hidden',
        display: 'flex',
        alignItems: 'center',
        zIndex: 1000,
      }}
    >
      <Box
        style={{
          display: 'flex',
          alignItems: 'center',
          animation: 'ticker-scroll 180s linear infinite',
          width: 'max-content',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.animationPlayState = 'paused';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.animationPlayState = 'running';
        }}
      >
        <Box style={{ display: 'flex', alignItems: 'center' }}>
          {tickerContent}
        </Box>
        <Box style={{ display: 'flex', alignItems: 'center' }}>
          {tickerContent}
        </Box>
      </Box>
    </Box>
  );
}
