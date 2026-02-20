'use client';

import { formatCents, formatPercentage } from '@/utils/format';
import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import { Group, Paper, SimpleGrid, Stack, Text } from '@mantine/core';
import { IconChartBar, IconTrendingDown, IconTrendingUp } from '@tabler/icons-react';
import Image from 'next/image';

interface PlayerWithPrice {
  symbol_id: number;
  name: string;
  team: string;
  currentPrice: number;
  priceChange: number;
}

interface MarketStatsProps {
  players: PlayerWithPrice[];
}

export function MarketStats({ players }: MarketStatsProps) {
  const topGainers = [...players]
    .filter((p) => p.priceChange > 0)
    .sort((a, b) => b.priceChange - a.priceChange)
    .slice(0, 4);

  const topLosers = [...players]
    .filter((p) => p.priceChange < 0)
    .sort((a, b) => a.priceChange - b.priceChange)
    .slice(0, 4);

  const topVolume = [...players]
    .sort((a, b) => b.currentPrice - a.currentPrice)
    .slice(0, 4);

  return (
    <SimpleGrid cols={3} spacing="sm" mb="md">
      <StatsPanel
        title="Top Gainers"
        icon={<IconTrendingUp size={14} color="var(--color-profit)" />}
        items={topGainers}
        valueRenderer={(p) => (
          <Text className="mono" size="xs" style={{ color: 'var(--color-profit)' }}>
            {formatPercentage(p.priceChange)}
          </Text>
        )}
      />
      <StatsPanel
        title="Top Losers"
        icon={<IconTrendingDown size={14} color="var(--color-loss)" />}
        items={topLosers}
        valueRenderer={(p) => (
          <Text className="mono" size="xs" style={{ color: 'var(--color-loss)' }}>
            {formatPercentage(p.priceChange)}
          </Text>
        )}
      />
      <StatsPanel
        title="Highest Price"
        icon={<IconChartBar size={14} color="var(--mantine-color-gold-3)" />}
        items={topVolume}
        valueRenderer={(p) => (
          <Text className="mono" size="xs" c="dimmed">
            {formatCents(p.currentPrice)}
          </Text>
        )}
      />
    </SimpleGrid>
  );
}

function StatsPanel({
  title,
  icon,
  items,
  valueRenderer,
}: {
  title: string;
  icon: React.ReactNode;
  items: PlayerWithPrice[];
  valueRenderer: (p: PlayerWithPrice) => React.ReactNode;
}) {
  return (
    <Paper withBorder p="md">
      <Group gap={6} mb="sm">
        {icon}
        <Text size="xs" fw={600} tt="uppercase" c="dimmed" lts="0.05em">
          {title}
        </Text>
      </Group>
      <Stack gap="xs">
        {items.length === 0 ? (
          <Text size="xs" c="dimmed">No data</Text>
        ) : (
          items.map((player) => (
            <Group key={player.symbol_id} justify="space-between" gap="xs">
              <Group gap={6} style={{ flex: 1, minWidth: 0 }}>
                <Image
                  src={getTeamLogoWithFallback(player.team)}
                  alt={player.team}
                  width={16}
                  height={16}
                  style={{ objectFit: 'contain', flexShrink: 0 }}
                  unoptimized
                />
                <Text
                  size="xs"
                  style={{
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {player.name}
                </Text>
              </Group>
              {valueRenderer(player)}
            </Group>
          ))
        )}
      </Stack>
    </Paper>
  );
}
