'use client';

import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { AccountSummaryResponse } from '@/types/api';
import {
  Alert,
  Card,
  Group,
  Skeleton,
  Stack,
  Text,
  ThemeIcon,
  Tooltip,
} from '@mantine/core';
import {
  IconAlertCircle,
  IconChartLine,
  IconCurrencyDollar,
  IconInfoCircle,
  IconMoneybag,
  IconTrendingDown,
  IconTrendingUp
} from '@tabler/icons-react';
import { useQuery } from '@tanstack/react-query';
import React from 'react';

// Design system colors - now using CSS variables
const COLORS = {
  profit: 'var(--profit-color)',
  loss: 'var(--loss-color)',
} as const;

interface AccountSummaryProps {
  accountId?: number;
  className?: string;
  style?: React.CSSProperties;
}

export function AccountSummary({ accountId, className, style }: AccountSummaryProps) {
  const { accountId: authAccountId } = useAuthStore();

  // Use provided accountId or fall back to auth store accountId, then default to 1
  const currentAccountId = accountId || (authAccountId ? parseInt(authAccountId) : 1);

  // Fetch account summary data
  const {
    data: summaryData,
    isLoading: summaryLoading,
    error: summaryError,
  } = useQuery<AccountSummaryResponse>({
    queryKey: ['account-summary', currentAccountId],
    queryFn: () => apiClient.rest.getAccountSummary(currentAccountId),
    refetchInterval: 1000, // 1 second polling
    staleTime: 500, // Consider data stale after 500ms
    enabled: !!currentAccountId, // Only run if we have an account ID
  });

  // Helper functions
  const formatCurrency = (cents: number | undefined) => {
    if (cents === undefined) return 'N/A';
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
    }).format(cents / 100);
  };

  const formatPercentage = (value: number | undefined) => {
    if (value === undefined) return 'N/A';
    return `${value.toFixed(2)}%`;
  };

  const getDayChangeColor = (change: number | undefined) => {
    if (change === undefined || change === 0) return 'dimmed';
    return change > 0 ? 'green' : 'red';
  };

  const getDayChangeIcon = (change: number | undefined) => {
    if (change === undefined || change === 0) return IconChartLine;
    if (change > 0) return IconTrendingUp;
    if (change < 0) return IconTrendingDown;
    return IconChartLine;
  };

  const formatPnL = (value: number | undefined) => {
    if (value === undefined) return 'N/A';
    const formatted = formatCurrency(Math.abs(value));
    const prefix = value >= 0 ? '+' : '-';
    return `${prefix}${formatted}`;
  };

  const getPnLColor = (value: number | undefined) => {
    if (value === undefined || value === 0) return 'dimmed';
    return value > 0 ? COLORS.profit : COLORS.loss;
  };

  // Loading state
  if (summaryLoading) {
    return (
      <Card
        className={className}
        style={style}
        padding="lg"
        radius="md"
        withBorder
      >
        <Stack gap="xs">
          <Skeleton height={24} width={150} />
          <Skeleton height={48} width={200} />
          <Skeleton height={24} width={180} />
        </Stack>
      </Card>
    );
  }

  // Error state
  if (summaryError) {
    return (
      <Card
        className={className}
        style={style}
        padding="lg"
        radius="md"
        withBorder
      >
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Unable to load account data"
          color="red"
          variant="light"
        >
          Please check your connection and try again.
        </Alert>
      </Card>
    );
  }

  if (!summaryData) return null;

  const DayChangeIcon = getDayChangeIcon(summaryData.day_change);

  return (
    <Card
      className={className}
      style={{
        ...style,
        minHeight: '100%', // Allow expansion beyond container height
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: 'var(--card-bg)',
        border: '1px solid var(--border-primary)',
      }}
      padding="lg"
      radius="md"
      withBorder
    >
      <Stack gap="xs" style={{ minHeight: '100%' }}>
        {/* Header Section */}
        <Stack gap={1} align="stretch">
          <Text size="lg" style={{ color: 'var(--text-primary)' }}>
            Account Summary
          </Text>
          <Text size="xl" style={{ color: 'var(--text-primary)' }}>
            {formatCurrency(summaryData.total_equity)}
          </Text>
          <DayChangeIcon
            size={3}
            color={getDayChangeColor(summaryData.day_change)}
          />
          <Text size="sm" c={getDayChangeColor(summaryData.day_change)}>
            {formatCurrency(summaryData.day_change)} (
            {formatPercentage(summaryData.day_change_percent)}) Today
          </Text>
        </Stack>
        

        {/* Content Area - No internal scrolling */}
        <Stack gap="md" style={{ flex: 1 }}>
            {/* Equity Chart */}
            {/* Chart removed - will be replaced with custom implementation */}

            {/* Profit & Loss Section */}
            <Stack gap="xs">
              <Group gap="xs" align="center">
                <Text size="sm" fw={500} style={{ letterSpacing: '0.5px', color: 'var(--text-primary)' }}>
                  Profit & Loss
                </Text>
                <Tooltip
                  label={
                    <div>
                      <div><strong>Unrealized P&L:</strong> Profit/loss on open positions</div>
                      <div><strong>Realized P&L:</strong> Profit/loss from completed trades</div>
                    </div>
                  }
                  multiline
                  withArrow
                >
                  <ThemeIcon size="xs" color="dimmed" variant="subtle">
                    <IconInfoCircle size={10} />
                  </ThemeIcon>
                </Tooltip>
              </Group>
              <Stack gap="xs">
                <Group justify="space-between">
                  <Group gap="xs">
                    <ThemeIcon size="sm" color="green" variant="light">
                      <IconTrendingUp size={12} />
                    </ThemeIcon>
                    <Text size="sm" c="dimmed">
                      Unrealized P&L
                    </Text>
                  </Group>
                  <Text 
                    size="sm" 
                    c={getPnLColor(summaryData.unrealized_pnl)}
                    fw={500}
                  >
                    {formatPnL(summaryData.unrealized_pnl)}
                  </Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <ThemeIcon size="sm" color="blue" variant="light">
                      <IconChartLine size={12} />
                    </ThemeIcon>
                    <Text size="sm" c="dimmed">
                      Realized P&L
                    </Text>
                  </Group>
                  <Text 
                    size="sm" 
                    c={getPnLColor(summaryData.realized_pnl)}
                    fw={500}
                  >
                    {formatPnL(summaryData.realized_pnl)}
                  </Text>
                </Group>
              </Stack>
            </Stack>

            {/* Account Overview Section */}
            <Stack gap="xs">
              <Text size="sm" fw={500}>
                Account Overview
              </Text>
              <Stack gap="xs">
                <Group justify="space-between">
                  <Group gap="xs">
                    <ThemeIcon size="sm" color="green" variant="light">
                      <IconCurrencyDollar size={12} />
                    </ThemeIcon>
                    <Text size="sm" c="dimmed">
                      Cash Balance
                    </Text>
                  </Group>
                  <Text size="sm" fw={500}>
                    {formatCurrency(summaryData.balance)}
                  </Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <ThemeIcon size="sm" color="blue" variant="light">
                      <IconChartLine size={12} />
                    </ThemeIcon>
                    <Text size="sm" c="dimmed">
                      Position Value
                    </Text>
                  </Group>
                  <Text size="sm" fw={500}>
                    {formatCurrency(summaryData.position_value)}
                  </Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <ThemeIcon size="sm" color="green" variant="light">
                      <IconMoneybag size={12} />
                    </ThemeIcon>
                    <Text size="sm" c="dimmed">
                      Buying Power
                    </Text>
                  </Group>
                  <Text size="sm" fw={500}>
                    {formatCurrency(summaryData.buying_power)}
                  </Text>
                </Group>
              </Stack>
            </Stack>
            <Text size="xs" c="dimmed" ta="center" style={{ marginTop: 'auto' }}>
              Last updated: {new Date(summaryData.last_updated).toLocaleTimeString()}
            </Text>
        </Stack>
      </Stack>
    </Card>
  );
}