'use client';

import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { AccountSummaryResponse } from '@/types/api';
import { formatCents, formatPnL, formatPercentage, getChangeColor } from '@/utils/format';
import { Box, Divider, Group, Skeleton, Stack, Text } from '@mantine/core';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useRef } from 'react';

export function AccountSummary() {
  const { accountId: authAccountId } = useAuthStore();
  const currentAccountId = authAccountId ? parseInt(authAccountId) : 1;
  const initialLoadDone = useRef(false);

  const { data, isLoading, error } = useQuery<AccountSummaryResponse>({
    queryKey: ['account-summary', currentAccountId],
    queryFn: () => apiClient.rest.getAccountSummary(currentAccountId),
    refetchInterval: 2000,
    staleTime: 1000,
    placeholderData: keepPreviousData,
    enabled: !!currentAccountId,
  });
  if (!isLoading) initialLoadDone.current = true;

  if (!initialLoadDone.current) {
    return (
      <Stack gap="xs" p="sm">
        <Skeleton height={14} width={80} />
        <Skeleton height={28} width={120} />
        <Skeleton height={12} width={100} />
      </Stack>
    );
  }

  if (error || !data) {
    return (
      <Box p="sm">
        <Text fz={11} c="dark.2">Unable to load account</Text>
      </Box>
    );
  }

  return (
    <Stack gap={0}>
      {/* Section header */}
      <Box px="sm" py={8} style={{ borderBottom: '1px solid var(--border-subtle)' }}>
        <Text fz={11} fw={600} tt="uppercase" lts="0.04em" c="dark.2">
          Account
        </Text>
      </Box>

      <Stack gap="xs" px="sm" py="sm">
        {/* Total equity — the big number */}
        <Box>
          <Text fz={10} c="dark.2" fw={500} tt="uppercase" lts="0.04em">
            Total Equity
          </Text>
          <Text className="mono" fz={22} fw={500} lh={1.2} c="dark.0">
            {formatCents(data.total_equity)}
          </Text>
          <Text
            className="mono"
            fz={11}
            fw={500}
            style={{ color: getChangeColor(data.day_change) }}
          >
            {formatPnL(data.day_change)} ({formatPercentage(data.day_change_percent)})
          </Text>
        </Box>

        <Divider color="dark.5" />

        {/* Compact data rows */}
        <Row label="Cash" value={formatCents(data.balance)} />
        <Row label="Positions" value={formatCents(data.position_value)} />
        <Row label="Buying Power" value={formatCents(data.buying_power)} />

        <Divider color="dark.5" />

        <Row label="Unrealized" value={formatPnL(data.unrealized_pnl)} color={getChangeColor(data.unrealized_pnl)} />
        <Row label="Realized" value={formatPnL(data.realized_pnl)} color={getChangeColor(data.realized_pnl)} />
      </Stack>
    </Stack>
  );
}

function Row({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <Group justify="space-between" gap="xs">
      <Text fz={11} c="dark.2">{label}</Text>
      <Text className="mono" fz={11} fw={500} style={color ? { color } : undefined}>
        {value}
      </Text>
    </Group>
  );
}
