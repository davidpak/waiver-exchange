
'use client';

import { apiClient } from '@/lib/api-client';
import type {
  AccountSummaryResponse,
  CurrentPriceResponse,
  PriceHistoryResponse,
  SnapshotResponse,
  SymbolInfoResponse,
  Timeframe
} from '@/types/api';
import {
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Modal,
  ScrollArea,
  Skeleton,
  Stack,
  Text,
  TextInput,
  ThemeIcon
} from '@mantine/core';
import {
  IconAlertCircle,
  IconChartLine,
  IconSearch,
  IconTrendingDown,
  IconTrendingUp
} from '@tabler/icons-react';
import { useQuery } from '@tanstack/react-query';
import React, { useEffect, useMemo, useState } from 'react';

// Design system colors - matching AccountSummary
const COLORS = {
  profit: 'var(--profit-color)',
  loss: 'var(--loss-color)',
} as const;

interface SymbolViewProps {
  symbolId: number;
  onSymbolChange?: (newSymbolId: number) => void;
  className?: string;
  style?: React.CSSProperties;
}

export function SymbolView({ symbolId, onSymbolChange, className, style }: SymbolViewProps) {
  const [selectedTimeframe, setSelectedTimeframe] = useState<Timeframe>('1d');
  const [orderModalOpen, setOrderModalOpen] = useState(false);
  const [orderSide, setOrderSide] = useState<'BUY' | 'SELL'>('BUY');
  const [searchModalOpen, setSearchModalOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(searchQuery), 150);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  // Load all players once for search
  const {
    data: allPlayers,
    isLoading: allPlayersLoading,
    error: allPlayersError,
  } = useQuery<SymbolInfoResponse[]>({
    queryKey: ['all-players'],
    queryFn: () => apiClient.rest.getAllPlayers(),
    staleTime: Infinity, // Never refetch - players don't change often
    gcTime: Infinity, // Keep in cache forever (renamed from cacheTime)
  });

  // Instant client-side search
  const searchResults = useMemo(() => {
    if (!debouncedQuery || !allPlayers) return [];
    
    const query = debouncedQuery.toLowerCase();
    return allPlayers
      .filter((player: SymbolInfoResponse) => 
        player.name.toLowerCase().includes(query) ||
        player.position.toLowerCase().includes(query) ||
        player.team.toLowerCase().includes(query)
      )
      .slice(0, 8)
      .sort((a: SymbolInfoResponse, b: SymbolInfoResponse) => {
        // Prioritize exact name matches
        const aExact = a.name.toLowerCase().startsWith(query);
        const bExact = b.name.toLowerCase().startsWith(query);
        if (aExact && !bExact) return -1;
        if (!aExact && bExact) return 1;
        return a.name.localeCompare(b.name);
      });
  }, [debouncedQuery, allPlayers]);

  // Fetch symbol information
  const {
    data: symbolInfo,
    isLoading: symbolInfoLoading,
    error: symbolInfoError,
  } = useQuery<SymbolInfoResponse>({
    queryKey: ['symbol-info', symbolId],
    queryFn: () => apiClient.rest.getSymbolInfo(symbolId),
    refetchInterval: 30000, // 30 seconds
    staleTime: 15000, // 15 seconds
  });

  // Fetch current market data (snapshot)
  const {
    data: snapshot,
    isLoading: snapshotLoading,
    error: snapshotError,
  } = useQuery<SnapshotResponse>({
    queryKey: ['snapshot-current'],
    queryFn: () => apiClient.rest.getCurrentSnapshot(),
    refetchInterval: 1000, // 1 second for real-time updates
    staleTime: 500, // 500ms
  });

  // Fetch price history for day change calculation
  const {
    data: priceHistory,
    isLoading: priceHistoryLoading,
    error: priceHistoryError,
  } = useQuery<PriceHistoryResponse>({
    queryKey: ['price-history', symbolId, '1d'],
    queryFn: () => apiClient.rest.getPriceHistory(symbolId, '1d', '5m'),
    refetchInterval: 10000, // 10 seconds
    staleTime: 5000, // 5 seconds
  });

  // Fetch account summary for order validation
  const {
    data: accountSummary,
    isLoading: accountLoading,
    error: accountError,
  } = useQuery<AccountSummaryResponse>({
    queryKey: ['account-summary'],
    queryFn: () => apiClient.rest.getAccountSummary(),
    refetchInterval: 5000, // 5 seconds
    staleTime: 2500, // 2.5 seconds
  });

  // Fetch current price as fallback
  const {
    data: currentPriceData,
    isLoading: currentPriceLoading,
    error: currentPriceError,
  } = useQuery<CurrentPriceResponse>({
    queryKey: ['current-price', symbolId],
    queryFn: () => apiClient.rest.getCurrentPrice(symbolId),
    refetchInterval: 10000, // 10 seconds
    staleTime: 5000, // 5 seconds
    enabled: !snapshot?.state.order_books[symbolId.toString()]?.last_trade_price, // Only fetch if no price from snapshot
  });

  // Calculate current price with fallback
  const currentPrice = snapshot?.state.order_books[symbolId.toString()]?.last_trade_price || 
                      currentPriceData?.price || 
                      null;
  const dayChange = React.useMemo(() => {
    // Try to get day change from price history
    if (priceHistory?.candles && priceHistory.candles.length >= 2) {
      const firstCandle = priceHistory.candles[priceHistory.candles.length - 1]; // Oldest
      const lastCandle = priceHistory.candles[0]; // Newest
      
      if (firstCandle && lastCandle) {
        const openPrice = firstCandle.open;
        const closePrice = lastCandle.close;
        const change = closePrice - openPrice;
        const changePercent = (change / openPrice) * 100;
        
        return {
          change,
          changePercent,
          open: openPrice,
          close: closePrice,
        };
      }
    }
    
    // No price history data - return zero change (like AccountSummary)
    return {
      change: 0,
      changePercent: 0,
      open: currentPrice || 0,
      close: currentPrice || 0,
    };
  }, [priceHistory, currentPrice]);

  // Helper functions
  const formatCurrency = (cents: number | null | undefined) => {
    if (cents === null || cents === undefined) return 'N/A';
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
    }).format(cents / 100);
  };

  const formatPercentage = (value: number) => {
    return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`;
  };

  const getChangeColor = (change: number | null) => {
    if (change === null || change === 0) return 'dimmed';
    return change > 0 ? 'green' : 'red';
  };

  const getChangeIcon = (change: number | null) => {
    if (change === null || change === 0) return IconChartLine;
    return change > 0 ? IconTrendingUp : IconTrendingDown;
  };

  // Handle player selection
  const handlePlayerSelect = (player: SymbolInfoResponse) => {
    if (onSymbolChange) {
      onSymbolChange(player.symbol_id);
    }
    setSearchModalOpen(false);
    setSearchQuery('');
    setDebouncedQuery('');
  };

  // Loading state
  if (symbolInfoLoading || snapshotLoading) {
    return (
      <Card className={className} style={style} padding="lg" radius="md" withBorder>
        <Stack gap="md">
          <Skeleton height={32} width={200} />
          <Skeleton height={48} width={150} />
          <Skeleton height={24} width={180} />
          <Group gap="sm">
            <Skeleton height={36} width={80} />
            <Skeleton height={36} width={80} />
          </Group>
        </Stack>
      </Card>
    );
  }

  // Error state
  if (symbolInfoError || snapshotError) {
    return (
      <Card className={className} style={style} padding="lg" radius="md" withBorder>
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Unable to load symbol data"
          color="red"
          variant="light"
        >
          Please check your connection and try again.
        </Alert>
      </Card>
    );
  }

  if (!symbolInfo) return null;

  const ChangeIcon = getChangeIcon(dayChange?.change || null);

  return (
    <Card
      className={className}
      style={{
        ...style,
        backgroundColor: 'var(--card-bg)',
        border: '1px solid var(--border-primary)',
      }}
      padding="lg"
      radius="md"
      withBorder
    >
      <Stack gap="md">
        {/* Symbol Header - Robinhood Legend Style */}
        <Stack gap="xs" align="stretch">
          {/* Header Section - Matching AccountSummary Structure */}
          <Stack gap={1} align="stretch">
            <Group gap="xs" align="center">
            <Group 
                gap="xs" 
                align="center"
                style={{ 
                cursor: 'pointer',
                textDecoration: 'highlight',
                textDecorationColor: 'transparent',
                transition: 'text-decoration-color 0.2s ease'
                }}
                onMouseEnter={(e) => {
                e.currentTarget.style.textDecorationColor = 'var(--text-primary)';
                }}
                onMouseLeave={(e) => {
                e.currentTarget.style.textDecorationColor = 'transparent';
                }}
                onClick={() => setSearchModalOpen(true)}
            >
                <IconSearch size={16} color="var(--text-secondary)" />
                <Text size="lg" style={{ color: 'var(--text-primary)' }}>
                {symbolInfo.name}
                </Text>
            </Group>
              <Group gap="xs">
                <Badge variant="light" color="blue" size="sm">
                  {symbolInfo.position}
                </Badge>
                <Badge variant="light" color="gray" size="sm">
                  {symbolInfo.team}
                </Badge>
              </Group>
            </Group>
            <Text size="xl" style={{ color: 'var(--text-primary)' }}>
              {formatCurrency(currentPrice)}
            </Text>
            <ChangeIcon
              size={3}
              color={getChangeColor(dayChange.change)}
            />
            <Text size="sm" c={getChangeColor(dayChange.change)}>
              {formatCurrency(dayChange.change)} ({formatPercentage(dayChange.changePercent)}) Today
            </Text>
          </Stack>

          {/* Action Buttons Row */}
          <Group gap="sm" mt="xs">
            <Button
              variant="light"
              color="green"
              size="sm"
              onClick={() => {
                setOrderSide('BUY');
                setOrderModalOpen(true);
              }}
              disabled={!currentPrice || !accountSummary}
              style={{ flex: 1 }}
            >
              Buy
            </Button>
            <Button
              variant="light"
              color="red"
              size="sm"
              onClick={() => {
                setOrderSide('SELL');
                setOrderModalOpen(true);
              }}
              disabled={!currentPrice || !accountSummary}
              style={{ flex: 1 }}
            >
              Sell
            </Button>
          </Group>
        </Stack>

        {/* Chart Section Placeholder */}
        <Box
          style={{
            height: '400px',
            backgroundColor: 'var(--surface-bg)',
            border: '1px solid var(--border-secondary)',
            borderRadius: '8px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <Stack gap="xs" align="center">
            <ThemeIcon size="xl" color="dimmed" variant="light">
              <IconChartLine size={32} />
            </ThemeIcon>
            <Text c="dimmed" size="sm">
              Chart will be implemented next
            </Text>
          </Stack>
        </Box>
      </Stack>

      {/* Symbol Search Modal */}
      <Modal
        opened={searchModalOpen}
        onClose={() => setSearchModalOpen(false)}
        title="Search Symbol"
        size="sm"
        centered
        overlayProps={{ backgroundOpacity: 0.55, blur: 3 }}
        styles={{
          content: {
            maxHeight: '80vh',
            maxWidth: '90vw',
            overflow: 'auto',
            margin: 'auto',
          },
          header: {
            position: 'sticky',
            top: 0,
            backgroundColor: 'var(--mantine-color-body)',
            zIndex: 1,
          },
        }}
        transitionProps={{ transition: 'fade', duration: 200 }}
      >
        <Stack gap="md" p="xs">
          <TextInput
            placeholder="Search for a player (e.g., 'Josh Allen', 'QB', 'BUF')"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            leftSection={<IconSearch size={16} />}
            size="md"
            style={{ width: '100%' }}
          />
          
          {/* Search Results */}
          <Box
            style={{
              minHeight: '200px',
              maxHeight: '300px',
              backgroundColor: 'var(--mantine-color-body)',
              border: '1px solid var(--mantine-color-default-border)',
              borderRadius: '8px',
              overflow: 'hidden',
            }}
          >
            {allPlayersLoading ? (
              <Stack gap="xs" align="center" justify="center" style={{ height: '200px' }}>
                <Skeleton height={20} width={150} />
                <Skeleton height={16} width={100} />
              </Stack>
            ) : allPlayersError ? (
              <Stack gap="xs" align="center" justify="center" style={{ height: '200px' }}>
                <IconAlertCircle size={24} color="var(--mantine-color-red-6)" />
                <Text c="red" size="sm" ta="center">
                  Failed to load players
                </Text>
              </Stack>
            ) : !debouncedQuery ? (
              <Stack gap="xs" align="center" justify="center" style={{ height: '200px' }}>
                <IconSearch size={24} color="var(--mantine-color-dimmed)" />
                <Text c="dimmed" size="sm" ta="center">
                  Start typing to search players
                </Text>
                <Text c="dimmed" size="xs" ta="center">
                  Search by name, position, or team
                </Text>
              </Stack>
            ) : searchResults.length === 0 ? (
              <Stack gap="xs" align="center" justify="center" style={{ height: '200px' }}>
                <IconSearch size={24} color="var(--mantine-color-dimmed)" />
                <Text c="dimmed" size="sm" ta="center">
                  No players found for "{debouncedQuery}"
                </Text>
                <Text c="dimmed" size="xs" ta="center">
                  Try a different search term
                </Text>
              </Stack>
            ) : (
              <ScrollArea style={{ height: '300px' }}>
                <Stack gap="xs" p="sm">
                  {searchResults.map((player: SymbolInfoResponse) => (
                    <Card
                      key={player.symbol_id}
                      padding="sm"
                      radius="md"
                      style={{
                        cursor: 'pointer',
                        border: '1px solid var(--mantine-color-default-border)',
                        transition: 'all 0.2s ease',
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--mantine-color-default-hover)';
                        e.currentTarget.style.borderColor = 'var(--mantine-color-blue-4)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--mantine-color-body)';
                        e.currentTarget.style.borderColor = 'var(--mantine-color-default-border)';
                      }}
                      onClick={() => handlePlayerSelect(player)}
                    >
                      <Group justify="space-between" align="center">
                        <Stack gap={2}>
                          <Text size="sm" fw={500} style={{ color: 'var(--text-primary)' }}>
                            {player.name}
                          </Text>
                          <Group gap="xs">
                            <Badge variant="light" color="blue" size="xs">
                              {player.position}
                            </Badge>
                            <Badge variant="light" color="gray" size="xs">
                              {player.team}
                            </Badge>
                          </Group>
                        </Stack>
                        <Stack gap={2} align="end">
                          <Text size="xs" c="dimmed">
                            {player.projected_points.toFixed(1)} pts
                          </Text>
                          <Text size="xs" c="dimmed">
                            #{player.rank}
                          </Text>
                        </Stack>
                      </Group>
                    </Card>
                  ))}
                </Stack>
              </ScrollArea>
            )}
          </Box>

          {/* Quick Symbol Selection */}
          <Stack gap="xs">
            <Text size="sm" fw={500}>Quick Select:</Text>
            <Group gap="xs" grow>
              <Button
                variant="light"
                size="sm"
                onClick={() => {
                  if (onSymbolChange) onSymbolChange(764); // Josh Allen
                  setSearchModalOpen(false);
                }}
                style={{ minWidth: '120px' }}
              >
                Josh Allen (764)
              </Button>
              <Button
                variant="light"
                size="sm"
                onClick={() => {
                  if (onSymbolChange) onSymbolChange(1); // Test with different ID
                  setSearchModalOpen(false);
                }}
                style={{ minWidth: '120px' }}
              >
                Test Symbol (1)
              </Button>
            </Group>
          </Stack>
        </Stack>
      </Modal>
    </Card>
  );
}