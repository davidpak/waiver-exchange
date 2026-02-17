'use client';

import { apiClient } from '@/lib/api-client';
import type {
  SymbolInfoResponse
} from '@/types/api';
import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import {
  Badge,
  Box,
  Card,
  Group,
  Pagination,
  ScrollArea,
  Skeleton,
  Stack,
  Table,
  Text,
  TextInput,
  ThemeIcon,
  UnstyledButton,
} from '@mantine/core';
import {
  IconArrowDown,
  IconArrowUp,
  IconSearch,
  IconTrendingDown,
  IconTrendingUp,
} from '@tabler/icons-react';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { PreviewCards } from './PreviewCards';

// Design system colors - matching existing components
const COLORS = {
  profit: 'var(--profit-color)',
  loss: 'var(--loss-color)',
  neutral: 'var(--neutral-color)',
} as const;

interface MarketPageProps {
  onSymbolSelect?: (symbolId: number) => void;
  className?: string;
  style?: React.CSSProperties;
}

type SortField = 'name' | 'position' | 'team' | 'price' | 'change' | 'rank';
type SortDirection = 'asc' | 'desc';

interface SortState {
  field: SortField;
  direction: SortDirection;
}

export function MarketPage({ onSymbolSelect, className, style }: MarketPageProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [currentPage, setCurrentPage] = useState(1);
  const [sortState, setSortState] = useState<SortState>({
    field: 'price',
    direction: 'desc',
  });
  const itemsPerPage = 25;

         // Helper functions - using bulk prices data
         const getCurrentPrice = (player: SymbolInfoResponse): number | null => {
           if (!bulkPricesData?.prices) return null;
           return bulkPricesData.prices[player.symbol_id.toString()] || null;
         };

  const getPriceChange = (player: SymbolInfoResponse): number | null => {
    // For now, return 0 as we don't have historical data
    // This could be enhanced with price history data
    return 0;
  };

  const formatCurrency = (cents: number | null): string => {
    if (cents === null) return 'N/A';
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
    }).format(cents / 100);
  };

  const formatPercentage = (value: number): string => {
    return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`;
  };

  const getChangeColor = (change: number | null) => {
    if (change === null || change === 0) return 'dimmed';
    return change > 0 ? 'green' : 'red';
  };

  const getChangeIcon = (change: number | null) => {
    if (change === null || change === 0) return null;
    return change > 0 ? IconTrendingUp : IconTrendingDown;
  };

         // Load all players - this already includes all the data we need
         const {
           data: allPlayers,
           isLoading: playersLoading,
           error: playersError,
         } = useQuery<SymbolInfoResponse[]>({
           queryKey: ['all-players'],
           queryFn: () => apiClient.rest.getAllPlayers(),
           staleTime: 30000, // 30 seconds
           gcTime: 300000, // 5 minutes
         });

         // Load bulk prices for all symbols
         const {
           data: bulkPricesData,
           isLoading: pricesLoading,
           error: pricesError,
         } = useQuery({
           queryKey: ['bulk-prices'],
           queryFn: () => apiClient.rest.getAllPrices(),
           refetchInterval: 1000, // 1 second
           staleTime: 500, // 500ms
         });

  // Filter and search players
  const filteredPlayers = useMemo(() => {
    if (!allPlayers) return [];

    let filtered = allPlayers;

    // Apply search filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (player) =>
          player.name.toLowerCase().includes(query) ||
          player.position.toLowerCase().includes(query) ||
          player.team.toLowerCase().includes(query)
      );
    }

    return filtered;
  }, [allPlayers, searchQuery]);

  // Sort players
  const sortedPlayers = useMemo(() => {
    if (!filteredPlayers.length) return [];

    return [...filteredPlayers].sort((a, b) => {
      let aValue: string | number;
      let bValue: string | number;

      switch (sortState.field) {
        case 'name':
          aValue = a.name.toLowerCase();
          bValue = b.name.toLowerCase();
          break;
        case 'position':
          aValue = a.position;
          bValue = b.position;
          break;
        case 'team':
          aValue = a.team;
          bValue = b.team;
          break;
        case 'price':
          aValue = getCurrentPrice(a) || 0;
          bValue = getCurrentPrice(b) || 0;
          break;
        case 'change':
          aValue = getPriceChange(a) || 0;
          bValue = getPriceChange(b) || 0;
          break;
        case 'rank':
          aValue = a.rank;
          bValue = b.rank;
          break;
        default:
          return 0;
      }

      if (typeof aValue === 'string' && typeof bValue === 'string') {
        const result = aValue.localeCompare(bValue);
        return sortState.direction === 'asc' ? result : -result;
      }

      if (typeof aValue === 'number' && typeof bValue === 'number') {
        const result = aValue - bValue;
        return sortState.direction === 'asc' ? result : -result;
      }

      return 0;
    });
  }, [filteredPlayers, sortState]);

  // Paginate players
  const paginatedPlayers = useMemo(() => {
    const startIndex = (currentPage - 1) * itemsPerPage;
    const endIndex = startIndex + itemsPerPage;
    return sortedPlayers.slice(startIndex, endIndex);
  }, [sortedPlayers, currentPage, itemsPerPage]);

  const totalPages = Math.ceil(sortedPlayers.length / itemsPerPage);

  const handleSort = (field: SortField) => {
    setSortState((prev) => ({
      field,
      direction: prev.field === field && prev.direction === 'desc' ? 'asc' : 'desc',
    }));
  };

  const handleSymbolClick = (symbolId: number) => {
    if (onSymbolSelect) {
      onSymbolSelect(symbolId);
    }
  };

  const SortButton = ({ field, children }: { field: SortField; children: React.ReactNode }) => {
    const isActive = sortState.field === field;
    const Icon = isActive
      ? sortState.direction === 'asc'
        ? IconArrowUp
        : IconArrowDown
      : null;

    return (
      <UnstyledButton
        onClick={() => handleSort(field)}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          fontWeight: isActive ? 600 : 400,
          color: isActive ? 'var(--mantine-color-blue-6)' : 'var(--mantine-color-text)',
          cursor: 'pointer',
          transition: 'all 0.2s ease',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.color = 'var(--mantine-color-blue-6)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.color = isActive
            ? 'var(--mantine-color-blue-6)'
            : 'var(--mantine-color-text)';
        }}
      >
        {children}
        {Icon && <Icon size={14} />}
      </UnstyledButton>
    );
  };

         // Loading state
         if (playersLoading || pricesLoading) {
    return (
      <Card className={className} style={style} padding="lg" radius="md" withBorder>
        <Stack gap="md">
          <Skeleton height={40} width={300} />
          <Skeleton height={400} />
          <Group justify="center">
            <Skeleton height={32} width={200} />
          </Group>
        </Stack>
      </Card>
    );
  }

         // Error state
         if (playersError || pricesError) {
    return (
      <Card className={className} style={style} padding="lg" radius="md" withBorder>
        <Stack gap="md" align="center">
          <ThemeIcon size="xl" color="red" variant="light">
            <IconTrendingDown size={32} />
          </ThemeIcon>
          <Text c="red" size="lg" fw={500}>
            Unable to load market data
          </Text>
          <Text c="dimmed" size="sm" ta="center">
            Please check your connection and try again.
          </Text>
        </Stack>
      </Card>
    );
  }

  if (!allPlayers) return null;

  return (
    <div
      className={className}
      style={{
        ...style,
        maxWidth: '900px',
        margin: '0 auto',
        width: '100%',
      }}
    >
      {/* Header - Outside the card */}
      <Stack gap="xs" style={{ marginBottom: '20px' }}>
        <Text size="xl" fw={600} style={{ color: 'var(--text-primary)' }}>
          Market Overview
        </Text>
        <Text size="sm" c="dimmed">
          {sortedPlayers.length} players • Real-time prices
        </Text>
      </Stack>

      {/* Preview Cards */}
      <PreviewCards 
        allPlayers={sortedPlayers || []} 
        allPrices={bulkPricesData || []} 
        isLoading={playersLoading || pricesLoading}
      />

      <Card
        style={{
          backgroundColor: 'var(--site-bg)',
          border: '1px solid var(--border-primary)',
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
        }}
        padding="lg"
        radius="md"
        withBorder
      >
        <Stack gap="md" style={{ height: '100%' }}>
          {/* Search */}
        <TextInput
          placeholder="Search players by name, position, or team..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          leftSection={<IconSearch size={16} />}
          size="md"
          style={{ width: '100%' }}
        />

        {/* Market Table */}
        <Box style={{ flex: 1, minHeight: 0 }}>
          <ScrollArea style={{ height: '100%' }}>
            <Table
              style={{
                backgroundColor: 'var(--mantine-color-body)',
                fontSize: '16px',
                borderCollapse: 'separate',
                borderSpacing: '0',
              }}
            >
              <Table.Thead>
                <Table.Tr>
                  <Table.Th style={{ padding: '8px 12px', fontSize: '14px', fontWeight: 500, color: 'var(--text-muted)' }}>
                    <SortButton field="name">Player</SortButton>
                  </Table.Th>
                  <Table.Th style={{ padding: '8px 12px', fontSize: '14px', fontWeight: 500, color: 'var(--text-muted)' }}>
                    <SortButton field="position">Pos</SortButton>
                  </Table.Th>
                  <Table.Th style={{ padding: '8px 12px', fontSize: '14px', fontWeight: 500, color: 'var(--text-muted)' }}>
                    <SortButton field="team">Team</SortButton>
                  </Table.Th>
                  <Table.Th style={{ padding: '8px 12px', fontSize: '14px', fontWeight: 500, color: 'var(--text-muted)' }}>
                    <SortButton field="price">Price</SortButton>
                  </Table.Th>
                  <Table.Th style={{ padding: '8px 12px', fontSize: '14px', fontWeight: 500, color: 'var(--text-muted)' }}>
                    <SortButton field="change">24h Change</SortButton>
                  </Table.Th>
                  <Table.Th style={{ padding: '8px 12px', fontSize: '14px', fontWeight: 500, color: 'var(--text-muted)' }}>Projected</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {paginatedPlayers.map((player) => {
                  const currentPrice = getCurrentPrice(player);
                  const priceChange = getPriceChange(player);
                  const ChangeIcon = getChangeIcon(priceChange);

                  return (
                    <Table.Tr
                      key={player.symbol_id}
                      style={{
                        cursor: 'pointer',
                        transition: 'background-color 0.2s ease',
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--hover-bg)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'transparent';
                      }}
                      onClick={() => handleSymbolClick(player.symbol_id)}
                    >
                      <Table.Td style={{ padding: '16px 12px' }}>
                        <Group gap="xs" align="center">
                          <img
                            src={getTeamLogoWithFallback(player.team)}
                            alt={`${player.team} logo`}
                            style={{
                              width: '28px',
                              height: '28px',
                              objectFit: 'contain',
                              borderRadius: '2px',
                              flexShrink: 0
                            }}
                            onError={(e) => {
                              // Fallback to placeholder if logo fails to load
                              e.currentTarget.src = '/src/assets/placeholder-logo.jpg';
                            }}
                          />
                          <Text size="md" fw={500} style={{ color: 'var(--text-primary)' }}>
                            {player.name}
                          </Text>
                        </Group>
                      </Table.Td>
                      <Table.Td style={{ padding: '16px 12px' }}>
                        <Badge variant="light" color="blue" size="md">
                          {player.position}
                        </Badge>
                      </Table.Td>
                      <Table.Td style={{ padding: '16px 12px' }}>
                        <Badge variant="light" color="gray" size="md">
                          {player.team}
                        </Badge>
                      </Table.Td>
                      <Table.Td style={{ padding: '16px 12px' }}>
                        <Text size="md" fw={500} style={{ color: 'var(--text-primary)' }}>
                          {formatCurrency(currentPrice)}
                        </Text>
                      </Table.Td>
                      <Table.Td style={{ padding: '16px 12px' }}>
                        <Group gap="xs" align="center">
                          {ChangeIcon && (
                            <ChangeIcon
                              size={16}
                              color={getChangeColor(priceChange)}
                            />
                          )}
                          <Text size="md" c={getChangeColor(priceChange)}>
                            {formatPercentage(priceChange || 0)}
                          </Text>
                        </Group>
                      </Table.Td>
                      <Table.Td style={{ padding: '16px 12px' }}>
                        <Text size="md" c="dimmed">
                          {player.projected_points.toFixed(1)} pts
                        </Text>
                      </Table.Td>
                    </Table.Tr>
                  );
                })}
              </Table.Tbody>
            </Table>
          </ScrollArea>
        </Box>

        {/* Pagination */}
        {totalPages > 1 && (
          <Group justify="end">
            <Pagination
              value={currentPage}
              onChange={setCurrentPage}
              total={totalPages}
              size="sm"
              radius="sm"
              withEdges
            />
          </Group>
        )}

        {/* Footer Info */}
        <Text size="xs" c="dimmed" ta="center">
          Showing {paginatedPlayers.length} of {sortedPlayers.length} players
          {searchQuery && ` matching "${searchQuery}"`}
        </Text>
        </Stack>
      </Card>

      {/* Footer for bottom spacing */}
      <div style={{ 
        marginTop: '40px', 
        padding: '20px 0 40px 0', 
        textAlign: 'center',
        borderTop: '1px solid var(--border-primary)',
        color: 'var(--text-muted)'
      }}>
        <Text size="sm">
          Waiver Exchange • Real-time Fantasy Football Trading
        </Text>
        <Text size="xs" style={{ marginTop: '8px' }}>
          © 2025 Waiver Exchange. All rights reserved.
        </Text>
      </div>
    </div>
  );
}
