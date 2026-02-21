'use client';

import { useTrading } from '@/contexts/TradingContext';
import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { PositionsListResponse } from '@/types/api';
import { formatCents, formatPnL, getChangeColor } from '@/utils/format';
import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import { Box, Group, Skeleton, Stack, Text, UnstyledButton } from '@mantine/core';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import Image from 'next/image';
import { useRef } from 'react';

export function Holdings() {
  const { selectedSymbolId, setSelectedSymbolId } = useTrading();
  const { accountId: authAccountId } = useAuthStore();
  const currentAccountId = authAccountId ? parseInt(authAccountId) : 1;

  const initialLoadDone = useRef(false);
  const { data, isLoading } = useQuery<PositionsListResponse>({
    queryKey: ['account-positions', currentAccountId],
    queryFn: () => apiClient.rest.getPositions(currentAccountId),
    refetchInterval: 5000,
    staleTime: 3000,
    placeholderData: keepPreviousData,
    retry: 1,
  });
  if (!isLoading) initialLoadDone.current = true;

  const positions = data?.positions ?? [];

  return (
    <Stack gap={0} style={{ flex: 1, minHeight: 0 }}>
      {/* Section header */}
      <Box px="sm" py={8} style={{ borderBottom: '1px solid var(--border-subtle)' }}>
        <Group justify="space-between">
          <Text fz={11} fw={600} tt="uppercase" lts="0.04em" c="dark.2">
            Holdings
          </Text>
          {positions.length > 0 && (
            <Text className="mono" fz={10} c="dark.3">
              {formatCents(data?.total_value ?? 0)}
            </Text>
          )}
        </Group>
      </Box>

      {!initialLoadDone.current ? (
        <Stack gap={8} p="sm">
          <Skeleton height={32} />
          <Skeleton height={32} />
          <Skeleton height={32} />
        </Stack>
      ) : positions.length === 0 ? (
        <Stack align="center" justify="center" py="lg" gap={4} style={{ flex: 1 }}>
          <Text fz={11} c="dark.3">No positions</Text>
          <Text fz={10} c="dark.3">Place trades to build your portfolio</Text>
        </Stack>
      ) : (
        <Box className="hide-scrollbar" style={{ overflow: 'auto', flex: 1 }}>
          {positions.map((pos) => (
            <UnstyledButton
              key={pos.symbol_id}
              w="100%"
              px="sm"
              py={8}
              style={{
                borderBottom: '1px solid var(--border-subtle)',
                transition: 'background-color 0.08s ease',
                backgroundColor: pos.symbol_id === selectedSymbolId
                  ? 'var(--mantine-color-dark-5)'
                  : undefined,
              }}
              onClick={() => setSelectedSymbolId(pos.symbol_id)}
              styles={{
                root: {
                  '&:hover': {
                    backgroundColor: 'var(--mantine-color-dark-5)',
                  },
                },
              }}
            >
              <Group justify="space-between" wrap="nowrap">
                <Group gap={8} wrap="nowrap" style={{ minWidth: 0 }}>
                  <Image
                    src={getTeamLogoWithFallback(pos.team)}
                    alt={pos.team}
                    width={16}
                    height={16}
                    style={{ objectFit: 'contain', flexShrink: 0 }}
                    loading="lazy"
                  />
                  <Box style={{ minWidth: 0 }}>
                    <Text fz={12} fw={500} truncate>
                      {pos.player_name}
                    </Text>
                    <Text fz={10} c="dark.2">
                      {pos.quantity} @ {formatCents(pos.avg_cost)}
                    </Text>
                  </Box>
                </Group>
                <Box ta="right" style={{ flexShrink: 0 }}>
                  <Text className="mono" fz={11} fw={500}>
                    {formatCents(pos.market_value)}
                  </Text>
                  <Text className="mono" fz={10} style={{ color: getChangeColor(pos.unrealized_pnl) }}>
                    {formatPnL(pos.unrealized_pnl)}
                  </Text>
                </Box>
              </Group>
            </UnstyledButton>
          ))}
        </Box>
      )}
    </Stack>
  );
}
