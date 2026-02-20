'use client';

import { useTrading } from '@/contexts/TradingContext';
import { useCurrentSnapshot } from '@/hooks/useMarketData';
import { formatCents } from '@/utils/format';
import { Box, Group, Stack, Text, UnstyledButton } from '@mantine/core';
import { IconArrowUp } from '@tabler/icons-react';
import { useMemo } from 'react';

const MAX_LEVELS = 15;

export function OrderBook() {
  const { selectedSymbolId, setFillPrice } = useTrading();

  const { data: snapshot } = useCurrentSnapshot();

  const orderBook = snapshot?.state?.order_books?.[selectedSymbolId.toString()];

  const { asks, bids, spread, spreadPct, lastPrice, maxQty } = useMemo(() => {
    if (!orderBook) {
      return { asks: [], bids: [], spread: null, spreadPct: null, lastPrice: null, maxQty: 1 };
    }

    const toEntries = (orders: any): { price: number; qty: number }[] => {
      if (!orders) return [];
      if (Array.isArray(orders)) {
        return orders.map(([price, qty]: [number, number]) => ({ price, qty }));
      }
      return Object.entries(orders).map(([price, qty]) => ({
        price: Number(price),
        qty: Number(qty),
      }));
    };

    // Asks: lowest first, then take the closest N to spread, display reversed (highest on top)
    const rawAsks = toEntries(orderBook.sell_orders)
      .sort((a, b) => a.price - b.price)
      .slice(0, MAX_LEVELS);

    // Bids: highest first (closest to spread on top)
    const rawBids = toEntries(orderBook.buy_orders)
      .sort((a, b) => b.price - a.price)
      .slice(0, MAX_LEVELS);

    // Cumulative quantities for depth bars
    let askCum = 0;
    const asksWithCum = rawAsks.map(a => {
      askCum += a.qty;
      return { ...a, cumQty: askCum };
    });

    let bidCum = 0;
    const bidsWithCum = rawBids.map(b => {
      bidCum += b.qty;
      return { ...b, cumQty: bidCum };
    });

    const maxCum = Math.max(askCum, bidCum, 1);

    const bestAsk = rawAsks.length > 0 ? rawAsks[0].price : null;
    const bestBid = rawBids.length > 0 ? rawBids[0].price : null;
    const sp = bestAsk != null && bestBid != null ? bestAsk - bestBid : null;
    const spPct = bestAsk != null && bestBid != null && bestBid > 0
      ? ((bestAsk - bestBid) / bestBid) * 100
      : null;

    const lp = orderBook.last_trade_price;

    return {
      asks: asksWithCum.reverse(), // Highest ask on top
      bids: bidsWithCum,
      spread: sp,
      spreadPct: spPct,
      lastPrice: lp,
      maxQty: maxCum,
    };
  }, [orderBook]);

  return (
    <Stack gap={0} style={{ height: '100%' }}>
      {/* Header */}
      <Group justify="space-between" px="sm" py={8}
        style={{ borderBottom: '1px solid var(--border-subtle)' }}
      >
        <Text fz={12} fw={600} tt="uppercase" lts="0.04em" c="dark.1">
          Order Book
        </Text>
      </Group>

      {/* Column labels */}
      <Group justify="space-between" px="sm" py={4}>
        <Text fz={10} c="dark.2" fw={500} tt="uppercase" lts="0.06em">
          Price
        </Text>
        <Text fz={10} c="dark.2" fw={500} tt="uppercase" lts="0.06em">
          Qty
        </Text>
        <Text fz={10} c="dark.2" fw={500} tt="uppercase" lts="0.06em">
          Total
        </Text>
      </Group>

      {/* Asks (sells) — highest price on top, lowest at bottom near spread */}
      <Box style={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'flex-end', minHeight: 0 }}>
        {asks.length === 0 && (
          <Text fz={11} c="dark.3" ta="center" py="xs">No asks</Text>
        )}
        {asks.map((ask) => (
          <OrderRow
            key={`ask-${ask.price}`}
            price={ask.price}
            qty={ask.qty}
            cumQty={ask.cumQty}
            maxQty={maxQty}
            side="ask"
            onClick={() => setFillPrice(ask.price)}
          />
        ))}
      </Box>

      {/* Spread + last trade price */}
      <Box
        px="sm"
        py={6}
        style={{
          borderTop: '1px solid var(--border-subtle)',
          borderBottom: '1px solid var(--border-subtle)',
        }}
      >
        {lastPrice != null && (
          <Group gap={4} justify="center">
            <Text className="mono" fz={15} fw={700} c="dark.0">
              {formatCents(lastPrice)}
            </Text>
            <IconArrowUp size={14} color="var(--color-profit)" />
          </Group>
        )}
        <Text className="mono" fz={10} c="dark.2" ta="center">
          Spread: {spread != null ? formatCents(spread) : '\u2014'}
          {spreadPct != null ? ` (${spreadPct.toFixed(2)}%)` : ''}
        </Text>
      </Box>

      {/* Bids (buys) — highest price on top near spread */}
      <Box style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
        {bids.length === 0 && (
          <Text fz={11} c="dark.3" ta="center" py="xs">No bids</Text>
        )}
        {bids.map((bid) => (
          <OrderRow
            key={`bid-${bid.price}`}
            price={bid.price}
            qty={bid.qty}
            cumQty={bid.cumQty}
            maxQty={maxQty}
            side="bid"
            onClick={() => setFillPrice(bid.price)}
          />
        ))}
      </Box>
    </Stack>
  );
}

function OrderRow({
  price,
  qty,
  cumQty,
  maxQty,
  side,
  onClick,
}: {
  price: number;
  qty: number;
  cumQty: number;
  maxQty: number;
  side: 'bid' | 'ask';
  onClick: () => void;
}) {
  const depthPct = (cumQty / maxQty) * 100;
  const priceColor = side === 'bid' ? 'var(--color-profit)' : 'var(--color-loss)';
  const barColor = side === 'bid' ? 'var(--color-profit-bg)' : 'var(--color-loss-bg)';

  return (
    <UnstyledButton
      w="100%"
      onClick={onClick}
      px="sm"
      py={3}
      style={{
        position: 'relative',
        overflow: 'hidden',
        transition: 'background-color 0.08s ease',
      }}
      styles={{
        root: {
          '&:hover': {
            backgroundColor: 'var(--mantine-color-dark-5)',
          },
        },
      }}
    >
      {/* Depth bar */}
      <Box
        style={{
          position: 'absolute',
          top: 0,
          bottom: 0,
          [side === 'bid' ? 'right' : 'left']: 0,
          width: `${depthPct}%`,
          backgroundColor: barColor,
          transition: 'width 0.15s ease',
        }}
      />

      <Group justify="space-between" style={{ position: 'relative', zIndex: 1 }}>
        <Text className="mono" fz={11} fw={500} style={{ color: priceColor }}>
          {formatCents(price)}
        </Text>
        <Text className="mono" fz={11} c="dark.1">
          {qty}
        </Text>
        <Text className="mono" fz={11} c="dark.2">
          {cumQty}
        </Text>
      </Group>
    </UnstyledButton>
  );
}
