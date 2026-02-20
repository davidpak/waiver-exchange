'use client';

import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { TradesListResponse } from '@/types/api';
import { formatCents } from '@/utils/format';
import { Badge, Box, Group, Skeleton, Stack, Text } from '@mantine/core';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useRef } from 'react';

export function TradeHistory() {
  const { accountId: authAccountId } = useAuthStore();
  const currentAccountId = authAccountId ? parseInt(authAccountId) : 1;
  const initialLoadDone = useRef(false);

  const { data, isLoading } = useQuery<TradesListResponse>({
    queryKey: ['account-trades', currentAccountId],
    queryFn: () => apiClient.rest.getTrades(currentAccountId),
    refetchInterval: 5000,
    staleTime: 3000,
    placeholderData: keepPreviousData,
    retry: 1,
  });
  if (!isLoading) initialLoadDone.current = true;

  const trades = data?.trades ?? [];

  if (!initialLoadDone.current) {
    return (
      <Stack gap={8} p="sm">
        <Skeleton height={28} />
        <Skeleton height={28} />
        <Skeleton height={28} />
      </Stack>
    );
  }

  if (trades.length === 0) {
    return (
      <Stack align="center" justify="center" py="lg" gap={4}>
        <Text fz={11} c="dark.3">No trades yet</Text>
        <Text fz={10} c="dark.3">Trade history will appear here</Text>
      </Stack>
    );
  }

  return (
    <Stack gap={0}>
      {trades.map((trade) => {
        const isBuy = trade.side === 'Buy';
        return (
          <Box
            key={trade.id}
            px="sm"
            py={6}
            style={{ borderBottom: '1px solid var(--border-subtle)' }}
          >
            <Group justify="space-between" wrap="nowrap">
              <Group gap={8} wrap="nowrap" style={{ minWidth: 0 }}>
                <Badge
                  size="xs"
                  variant="light"
                  color={isBuy ? 'green' : 'red'}
                  w={32}
                  fz={9}
                  style={{ textAlign: 'center', flexShrink: 0 }}
                >
                  {isBuy ? 'BUY' : 'SELL'}
                </Badge>
                <Box style={{ minWidth: 0 }}>
                  <Text fz={11} fw={500} truncate>{trade.player_name}</Text>
                  <Text fz={9} c="dark.3">
                    {new Date(trade.timestamp).toLocaleTimeString()}
                  </Text>
                </Box>
              </Group>
              <Box ta="right" style={{ flexShrink: 0 }}>
                <Text className="mono" fz={11}>
                  {trade.quantity} @ {formatCents(trade.price)}
                </Text>
                <Text className="mono" fz={9} c="dark.3">
                  {formatCents(trade.total_value)}
                </Text>
              </Box>
            </Group>
          </Box>
        );
      })}
    </Stack>
  );
}
