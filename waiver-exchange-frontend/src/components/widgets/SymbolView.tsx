'use client';

import { useTrading } from '@/contexts/TradingContext';
import { apiClient } from '@/lib/api-client';
import { useBulkPrices, useCurrentSnapshot } from '@/hooks/useMarketData';
import type {
  PriceHistoryResponse,
  SymbolInfoResponse,
  Timeframe,
} from '@/types/api';
import { formatCents, formatPercentage, getChangeColor } from '@/utils/format';
import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import { PriceFlash } from '@/components/common/PriceFlash';
import { Badge, Box, Button, Group, Skeleton, Stack, Text } from '@mantine/core';
import { IconChevronDown } from '@tabler/icons-react';
import { useQuery } from '@tanstack/react-query';
import Image from 'next/image';
import { useRef, useMemo, useState } from 'react';
import { PriceChart } from '../charts/PriceChart';
import { QuickOrderModal } from './QuickOrderModal';

const TIMEFRAMES: { label: string; value: Timeframe; interval: string }[] = [
  { label: '1D', value: '1d', interval: '5m' },
  { label: '1W', value: '1w', interval: '1h' },
  { label: '1M', value: '1m', interval: '4h' },
  { label: '3M', value: '3m', interval: '1d' },
  { label: '1Y', value: '1y', interval: '1d' },
];

interface OrderDetails {
  orderId: string;
  symbol: string;
  side: 'BUY' | 'SELL';
  type: string;
  price: number;
  quantity: number;
  orderStatus: string;
}

interface SymbolViewProps {
  onOrderPlaced?: (status: 'success' | 'error', message: string, orderDetails?: OrderDetails) => void;
}

export function SymbolView({ onOrderPlaced }: SymbolViewProps) {
  const { selectedSymbolId, openSearch } = useTrading();
  const [selectedTimeframe, setSelectedTimeframe] = useState(TIMEFRAMES[0]);
  const [orderModal, setOrderModal] = useState<'BUY' | 'SELL' | null>(null);

  const initialLoadDone = useRef(false);
  const { data: symbolInfo, isLoading: infoLoading, error: infoError } = useQuery<SymbolInfoResponse>({
    queryKey: ['symbol-info', selectedSymbolId],
    queryFn: () => apiClient.rest.getSymbolInfo(selectedSymbolId),
    refetchInterval: 30000,
    staleTime: 15000,
  });
  if (!infoLoading) initialLoadDone.current = true;

  const { data: snapshot } = useCurrentSnapshot();
  const { data: bulkPrices } = useBulkPrices(1000);

  const { data: priceHistory } = useQuery<PriceHistoryResponse>({
    queryKey: ['price-history', selectedSymbolId, selectedTimeframe.value],
    queryFn: () =>
      apiClient.rest.getPriceHistory(selectedSymbolId, selectedTimeframe.value, selectedTimeframe.interval),
    refetchInterval: 10000,
    staleTime: 5000,
  });

  const currentPrice =
    snapshot?.state?.order_books?.[selectedSymbolId.toString()]?.last_trade_price ||
    bulkPrices?.prices?.[selectedSymbolId.toString()] ||
    null;

  const dayChange = useMemo(() => {
    if (priceHistory?.candles && priceHistory.candles.length >= 2) {
      const oldest = priceHistory.candles[priceHistory.candles.length - 1];
      const newest = priceHistory.candles[0];
      if (oldest && newest) {
        const change = newest.close - oldest.open;
        const pct = oldest.open !== 0 ? (change / oldest.open) * 100 : 0;
        return { change, pct };
      }
    }
    return { change: 0, pct: 0 };
  }, [priceHistory]);

  if (!initialLoadDone.current) {
    return (
      <Stack gap="sm" p="sm">
        <Skeleton height={18} width={160} />
        <Skeleton height={28} width={100} />
        <Skeleton height={300} />
      </Stack>
    );
  }

  if (infoError || !symbolInfo) {
    return (
      <Box p="sm">
        <Text fz={12} c="dark.2">Unable to load symbol</Text>
      </Box>
    );
  }

  return (
    <Stack gap={0} style={{ flex: 1, minHeight: 0 }}>
      {/* Symbol header bar */}
      <Group justify="space-between" px="sm" py={10} wrap="nowrap"
        style={{ borderBottom: '1px solid var(--border-subtle)' }}
      >
        {/* Left: name, badges, price */}
        <Box>
          <Group gap={8} align="center" mb={2}>
            <Image
              src={getTeamLogoWithFallback(symbolInfo.team)}
              alt={symbolInfo.team}
              width={20}
              height={20}
              style={{ objectFit: 'contain' }}
              unoptimized
            />
            <Text
              fw={600}
              fz={15}
              c="dark.0"
              style={{ cursor: 'pointer' }}
              onClick={openSearch}
            >
              {symbolInfo.name}
            </Text>
            <IconChevronDown size={12} color="var(--mantine-color-dark-2)" style={{ cursor: 'pointer' }} onClick={openSearch} />
            <Badge variant="light" color="gold" size="xs" fz={9}>
              {symbolInfo.position}
            </Badge>
            <Badge variant="light" color="dark" size="xs" fz={9}>
              {symbolInfo.team}
            </Badge>
          </Group>

          {/* Price row */}
          <Group gap="sm" align="baseline">
            <PriceFlash value={currentPrice}>
              <Text className="mono" fz={22} fw={600} c="dark.0">
                {formatCents(currentPrice)}
              </Text>
            </PriceFlash>
            <Text
              className="mono"
              fz={12}
              fw={500}
              style={{ color: getChangeColor(dayChange.change) }}
            >
              {dayChange.change >= 0 ? '+' : ''}
              {formatCents(dayChange.change)} ({formatPercentage(dayChange.pct)})
            </Text>
          </Group>
        </Box>

        {/* Right: Buy + Sell buttons */}
        <Group gap={8}>
          <Button
            size="sm"
            px="lg"
            radius="md"
            style={{
              backgroundColor: 'rgba(52, 211, 153, 0.12)',
              color: 'var(--color-profit)',
              border: '1px solid rgba(52, 211, 153, 0.20)',
              transition: 'all 0.15s ease',
            }}
            styles={{
              root: {
                '&:hover': {
                  backgroundColor: 'rgba(52, 211, 153, 0.22)',
                  borderColor: 'rgba(52, 211, 153, 0.35)',
                  boxShadow: '0 0 16px rgba(52, 211, 153, 0.10)',
                },
              },
            }}
            onClick={() => setOrderModal('BUY')}
          >
            Buy
          </Button>
          <Button
            size="sm"
            px="lg"
            radius="md"
            style={{
              backgroundColor: 'rgba(248, 113, 113, 0.12)',
              color: 'var(--color-loss)',
              border: '1px solid rgba(248, 113, 113, 0.20)',
              transition: 'all 0.15s ease',
            }}
            styles={{
              root: {
                '&:hover': {
                  backgroundColor: 'rgba(248, 113, 113, 0.22)',
                  borderColor: 'rgba(248, 113, 113, 0.35)',
                  boxShadow: '0 0 16px rgba(248, 113, 113, 0.10)',
                },
              },
            }}
            onClick={() => setOrderModal('SELL')}
          >
            Sell
          </Button>
        </Group>
      </Group>

      {/* Timeframe pills */}
      <Group gap={2} px="sm" py={6}>
        {TIMEFRAMES.map((tf) => (
          <Button
            key={tf.value}
            variant={selectedTimeframe.value === tf.value ? 'filled' : 'subtle'}
            color={selectedTimeframe.value === tf.value ? 'gold' : 'gray'}
            size="compact-xs"
            fz={10}
            fw={600}
            px={8}
            onClick={() => setSelectedTimeframe(tf)}
          >
            {tf.label}
          </Button>
        ))}
      </Group>

      {/* Chart */}
      <Box style={{ flex: 1, minHeight: 250, padding: '0 4px' }}>
        {priceHistory?.candles && priceHistory.candles.length > 0 ? (
          <PriceChart candles={priceHistory.candles} />
        ) : (
          <Stack align="center" justify="center" style={{ height: '100%' }}>
            <Text fz={12} c="dark.3">No chart data available</Text>
          </Stack>
        )}
      </Box>

      {/* Quick order modal */}
      {orderModal && (
        <QuickOrderModal
          side={orderModal}
          symbolInfo={symbolInfo}
          currentPrice={currentPrice}
          onClose={() => setOrderModal(null)}
          onOrderPlaced={onOrderPlaced}
        />
      )}
    </Stack>
  );
}
