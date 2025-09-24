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
} from '@mantine/core';
import {
    IconAlertCircle,
    IconChartLine,
    IconTrendingDown,
    IconTrendingUp,
    IconWallet,
} from '@tabler/icons-react';
import { useQuery } from '@tanstack/react-query';
import React from 'react';

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
      style={style}
      padding="lg"
      radius="md"
      withBorder
    >
      <Stack gap="xs">
        {/* Header */}
        <Text size="lg" c="white">
          Account Summary
        </Text>

        {/* Total Equity - BIGGEST TEXT */}
        <Stack gap={2}>
          <Text size="2xl" fw={800} c="white">
            {formatCurrency(summaryData.total_equity)}
          </Text>
          <Group gap="xs" align="center">
            <DayChangeIcon
              size={16}
              color={getDayChangeColor(summaryData.day_change)}
            />
            <Text
              size="sm"
              c={getDayChangeColor(summaryData.day_change)}
              fw={600}
            >
              {formatCurrency(summaryData.day_change)} ({formatPercentage(summaryData.day_change_percent)}) Today
            </Text>
          </Group>
        </Stack>

        {/* Account Details */}
        <Stack gap="xs">
          <Group justify="space-between">
            <Group gap="xs">
              <ThemeIcon size="sm" color="blue" variant="light">
                <IconWallet size={12} />
              </ThemeIcon>
              <Text size="sm" c="dimmed">
                Cash Balance
              </Text>
            </Group>
            <Text size="sm">
              {formatCurrency(summaryData.balance)}
            </Text>
          </Group>

          <Group justify="space-between">
            <Group gap="xs">
              <ThemeIcon size="sm" color="green" variant="light">
                <IconChartLine size={12} />
              </ThemeIcon>
              <Text size="sm" c="dimmed">
                Buying Power
              </Text>
            </Group>
            <Text size="sm">
              {formatCurrency(summaryData.buying_power)}
            </Text>
          </Group>
        </Stack>

        {/* Last Updated */}
        <Text size="xs" c="dimmed" ta="center">
          Last updated: {new Date(summaryData.last_updated).toLocaleTimeString()}
        </Text>
      </Stack>
    </Card>
  );
}