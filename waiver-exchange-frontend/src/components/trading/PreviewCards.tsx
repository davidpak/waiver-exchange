'use client';

import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import { Card, Group, Stack, Text } from '@mantine/core';
import { IconChartBar, IconTrendingDown, IconTrendingUp } from '@tabler/icons-react';
import { useMemo } from 'react';

interface PreviewCardsProps {
  allPlayers: any[];
  allPrices: any;
  isLoading?: boolean;
  className?: string;
  style?: React.CSSProperties;
}

export function PreviewCards({ allPlayers, allPrices, isLoading, className, style }: PreviewCardsProps) {

  // Helper functions
  const getCurrentPrice = (player: any) => {
    if (!allPrices?.prices) return 0;
    return allPrices.prices[player.symbol_id.toString()] || 0;
  };

  const getPriceChange = (player: any) => {
    // For now, return 0 as we don't have historical data
    return 0;
  };

  // Calculate preview data
  const previewData = useMemo(() => {
    if (!allPlayers.length || !allPrices?.prices) return null;

    const playersWithPrices = allPlayers.map(player => ({
      ...player,
      currentPrice: getCurrentPrice(player),
      priceChange: getPriceChange(player),
    }));

    // Top Gainers (biggest price increases)
    const topGainers = [...playersWithPrices]
      .filter(p => p.priceChange > 0)
      .sort((a, b) => b.priceChange - a.priceChange)
      .slice(0, 3);

    // Top Losers (biggest price decreases)
    const topLosers = [...playersWithPrices]
      .filter(p => p.priceChange < 0)
      .sort((a, b) => a.priceChange - b.priceChange)
      .slice(0, 3);

    // Top Volume (mock data for now)
    const topVolume = [...playersWithPrices]
      .sort((a, b) => b.currentPrice - a.currentPrice)
      .slice(0, 3);

    return { topGainers, topLosers, topVolume };
  }, [allPlayers, allPrices]);

  if (isLoading || !previewData) {
    return (
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '16px', marginBottom: '20px' }}>
        {[1, 2, 3].map(i => (
          <Card key={i} padding="md" radius="md" withBorder>
            <Stack gap="sm">
              <div style={{ height: '20px', backgroundColor: 'var(--mantine-color-gray-3)', borderRadius: '4px' }} />
              <div style={{ height: '40px', backgroundColor: 'var(--mantine-color-gray-2)', borderRadius: '4px' }} />
            </Stack>
          </Card>
        ))}
      </div>
    );
  }

  const formatCurrency = (cents: number) => {
    if (cents === 0) return 'N/A';
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
    }).format(cents / 100);
  };
  const formatPercentage = (percentage: number) => `${percentage >= 0 ? '+' : ''}${percentage.toFixed(1)}%`;

  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '16px', marginBottom: '20px' }}>
      {/* Top Gainers */}
      <Card padding="lg" radius="md" withBorder>
        <Stack gap="md">
          <Group gap="sm" align="center">
            <IconTrendingUp size={20} color="var(--mantine-color-green-6)" />
            <Text size="md" fw={600} c="green">Top Gainers</Text>
          </Group>
          <Stack gap="sm">
            {previewData.topGainers.map((player, index) => (
              <Group key={player.symbol_id} gap="sm" align="center">
                <img
                  src={getTeamLogoWithFallback(player.team)}
                  alt={`${player.team} logo`}
                  style={{ width: '20px', height: '20px', objectFit: 'contain' }}
                />
                <Text size="sm" style={{ flex: 1 }}>{player.name}</Text>
                <Text size="sm" c="green">{formatPercentage(player.priceChange)}</Text>
              </Group>
            ))}
          </Stack>
        </Stack>
      </Card>

      {/* Top Losers */}
      <Card padding="lg" radius="md" withBorder>
        <Stack gap="md">
          <Group gap="sm" align="center">
            <IconTrendingDown size={20} color="var(--mantine-color-red-6)" />
            <Text size="md" fw={600} c="red">Top Losers</Text>
          </Group>
          <Stack gap="sm">
            {previewData.topLosers.map((player, index) => (
              <Group key={player.symbol_id} gap="sm" align="center">
                <img
                  src={getTeamLogoWithFallback(player.team)}
                  alt={`${player.team} logo`}
                  style={{ width: '20px', height: '20px', objectFit: 'contain' }}
                />
                <Text size="sm" style={{ flex: 1 }}>{player.name}</Text>
                <Text size="sm" c="red">{formatPercentage(player.priceChange)}</Text>
              </Group>
            ))}
          </Stack>
        </Stack>
      </Card>

      {/* Top Volume */}
      <Card padding="lg" radius="md" withBorder>
        <Stack gap="md">
          <Group gap="sm" align="center">
            <IconChartBar size={20} color="var(--mantine-color-blue-6)" />
            <Text size="md" fw={600} c="blue">Top Volume</Text>
          </Group>
          <Stack gap="sm">
            {previewData.topVolume.map((player, index) => (
              <Group key={player.symbol_id} gap="sm" align="center">
                <img
                  src={getTeamLogoWithFallback(player.team)}
                  alt={`${player.team} logo`}
                  style={{ width: '20px', height: '20px', objectFit: 'contain' }}
                />
                <Text size="sm" style={{ flex: 1 }}>{player.name}</Text>
                <Text size="sm" c="dimmed">{formatCurrency(player.currentPrice)}</Text>
              </Group>
            ))}
          </Stack>
        </Stack>
      </Card>
    </div>
  );
}
