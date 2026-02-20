'use client';

import { useAllPlayers, useBulkPrices } from '@/hooks/useMarketData';
import type { SymbolInfoResponse } from '@/types/api';
import { formatCents, formatPercentage, getChangeColor } from '@/utils/format';
import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import {
  Badge,
  Box,
  Button,
  Group,
  Pagination,
  Skeleton,
  Stack,
  Table,
  Text,
  TextInput,
  UnstyledButton,
} from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { IconArrowDown, IconArrowUp, IconSearch } from '@tabler/icons-react';
import Image from 'next/image';
import { useCallback, useMemo, useState } from 'react';
import { MarketStats } from './MarketStats';

interface MarketTableProps {
  onSymbolSelect?: (symbolId: number) => void;
}

type SortField = 'rank' | 'name' | 'position' | 'team' | 'price' | 'change' | 'projected';
type SortDir = 'asc' | 'desc';

const POSITIONS = ['ALL', 'QB', 'RB', 'WR', 'TE'] as const;
const PER_PAGE = 25;

export function MarketTable({ onSymbolSelect }: MarketTableProps) {
  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search, 150);
  const [posFilter, setPosFilter] = useState<string>('ALL');
  const [page, setPage] = useState(1);
  const [sort, setSort] = useState<{ field: SortField; dir: SortDir }>({
    field: 'rank',
    dir: 'asc',
  });

  const { data: allPlayers, isLoading: playersLoading } = useAllPlayers();
  const { data: bulkPrices, isLoading: pricesLoading } = useBulkPrices(1000);

  const getPrice = useCallback(
    (p: SymbolInfoResponse) => bulkPrices?.prices?.[p.symbol_id.toString()] || 0,
    [bulkPrices]
  );

  const filtered = useMemo(() => {
    if (!allPlayers) return [];
    let result = allPlayers;

    if (posFilter !== 'ALL') {
      result = result.filter((p) => p.position === posFilter);
    }

    if (debouncedSearch.trim()) {
      const q = debouncedSearch.toLowerCase();
      result = result.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.team.toLowerCase().includes(q) ||
          p.position.toLowerCase().includes(q)
      );
    }

    return result;
  }, [allPlayers, posFilter, debouncedSearch]);

  const sorted = useMemo(() => {
    if (!filtered.length) return [];
    return [...filtered].sort((a, b) => {
      let cmp = 0;
      switch (sort.field) {
        case 'rank': cmp = a.rank - b.rank; break;
        case 'name': cmp = a.name.localeCompare(b.name); break;
        case 'position': cmp = a.position.localeCompare(b.position); break;
        case 'team': cmp = a.team.localeCompare(b.team); break;
        case 'price': cmp = getPrice(a) - getPrice(b); break;
        case 'projected': cmp = a.projected_points - b.projected_points; break;
        default: cmp = 0;
      }
      return sort.dir === 'asc' ? cmp : -cmp;
    });
  }, [filtered, sort, getPrice]);

  const totalPages = Math.ceil(sorted.length / PER_PAGE);
  const pageData = sorted.slice((page - 1) * PER_PAGE, page * PER_PAGE);

  const statsPlayers = useMemo(
    () =>
      (allPlayers || []).map((p) => ({
        symbol_id: p.symbol_id,
        name: p.name,
        team: p.team,
        currentPrice: getPrice(p),
        priceChange: 0,
      })),
    [allPlayers, getPrice]
  );

  const toggleSort = (field: SortField) => {
    setSort((prev) =>
      prev.field === field
        ? { field, dir: prev.dir === 'asc' ? 'desc' : 'asc' }
        : { field, dir: 'desc' }
    );
    setPage(1);
  };

  const loading = playersLoading || pricesLoading;

  if (loading) {
    return (
      <Stack gap="md" p="md">
        <Skeleton height={100} />
        <Skeleton height={40} />
        <Skeleton height={400} />
      </Stack>
    );
  }

  return (
    <Stack gap={0}>
      {/* Stats cards */}
      <Box px="md" pt="md">
        <MarketStats players={statsPlayers} />
      </Box>

      {/* Filters row */}
      <Group px="md" pb="sm" gap="sm">
        <TextInput
          placeholder="Search players..."
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(1); }}
          leftSection={<IconSearch size={16} />}
          size="sm"
          style={{ flex: 1, maxWidth: 300 }}
        />
        <Group gap={4}>
          {POSITIONS.map((pos) => (
            <Button
              key={pos}
              variant={posFilter === pos ? 'filled' : 'subtle'}
              color={posFilter === pos ? 'gold' : 'gray'}
              size="compact-xs"
              fz={11}
              px={10}
              onClick={() => { setPosFilter(pos); setPage(1); }}
            >
              {pos}
            </Button>
          ))}
        </Group>
        <Text size="xs" c="dimmed" ml="auto">
          {sorted.length} players
        </Text>
      </Group>

      {/* Table */}
      <Box style={{ overflow: 'auto' }}>
        <Table highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th><SortBtn field="rank" current={sort} onClick={toggleSort}>#</SortBtn></Table.Th>
              <Table.Th><SortBtn field="name" current={sort} onClick={toggleSort}>Player</SortBtn></Table.Th>
              <Table.Th><SortBtn field="position" current={sort} onClick={toggleSort}>Pos</SortBtn></Table.Th>
              <Table.Th><SortBtn field="team" current={sort} onClick={toggleSort}>Team</SortBtn></Table.Th>
              <Table.Th><SortBtn field="price" current={sort} onClick={toggleSort}>Price</SortBtn></Table.Th>
              <Table.Th>Change</Table.Th>
              <Table.Th><SortBtn field="projected" current={sort} onClick={toggleSort}>Proj. Pts</SortBtn></Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {pageData.map((player) => {
              const price = getPrice(player);
              return (
                <Table.Tr
                  key={player.symbol_id}
                  style={{ cursor: 'pointer' }}
                  onClick={() => onSymbolSelect?.(player.symbol_id)}
                >
                  <Table.Td>
                    <Text className="mono" size="sm" c="dimmed">{player.rank}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Group gap="sm">
                      <Image
                        src={getTeamLogoWithFallback(player.team)}
                        alt={player.team}
                        width={24}
                        height={24}
                        style={{ objectFit: 'contain' }}
                        unoptimized
                      />
                      <Text size="sm" fw={500}>
                        {player.name}
                      </Text>
                    </Group>
                  </Table.Td>
                  <Table.Td>
                    <Badge variant="light" color="gold" size="sm">
                      {player.position}
                    </Badge>
                  </Table.Td>
                  <Table.Td>
                    <Badge variant="light" color="gray" size="sm">{player.team}</Badge>
                  </Table.Td>
                  <Table.Td>
                    <Text className="mono" size="sm" fw={500}>
                      {formatCents(price)}
                    </Text>
                  </Table.Td>
                  <Table.Td>
                    <Text className="mono" size="sm" style={{ color: getChangeColor(0) }}>
                      {formatPercentage(0)}
                    </Text>
                  </Table.Td>
                  <Table.Td>
                    <Text className="mono" size="sm" c="dimmed">
                      {player.projected_points.toFixed(1)}
                    </Text>
                  </Table.Td>
                </Table.Tr>
              );
            })}
          </Table.Tbody>
        </Table>
      </Box>

      {/* Pagination */}
      {totalPages > 1 && (
        <Group justify="center" py="md">
          <Pagination
            value={page}
            onChange={setPage}
            total={totalPages}
            size="sm"
            radius="md"
          />
        </Group>
      )}
    </Stack>
  );
}

function SortBtn({
  field,
  current,
  onClick,
  children,
}: {
  field: SortField;
  current: { field: SortField; dir: SortDir };
  onClick: (f: SortField) => void;
  children: React.ReactNode;
}) {
  const active = current.field === field;
  const Icon = active ? (current.dir === 'asc' ? IconArrowUp : IconArrowDown) : null;

  return (
    <UnstyledButton
      onClick={() => onClick(field)}
      c={active ? 'gold.3' : 'dimmed'}
      fw={active ? 600 : 500}
      fz={13}
      style={{ display: 'flex', alignItems: 'center', gap: 4 }}
    >
      {children}
      {Icon && <Icon size={12} />}
    </UnstyledButton>
  );
}
