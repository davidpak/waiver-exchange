'use client';

import { marketKeys, usePlayerSearch } from '@/hooks/useMarketData';
import type { SymbolInfoResponse } from '@/types/api';
import { formatCents } from '@/utils/format';
import { getTeamLogoWithFallback } from '@/utils/teamLogos';
import { Badge, Box, Group, Kbd, Modal, ScrollArea, Stack, Text, TextInput, UnstyledButton } from '@mantine/core';
import { useDebouncedValue, useHotkeys } from '@mantine/hooks';
import { useQueryClient } from '@tanstack/react-query';
import { IconSearch } from '@tabler/icons-react';
import Image from 'next/image';
import { memo, useCallback, useEffect, useRef, useState } from 'react';

interface CommandPaletteProps {
  opened: boolean;
  onClose: () => void;
  onSelect: (symbolId: number) => void;
}

export function CommandPalette({ opened, onClose, onSelect }: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [debouncedQuery] = useDebouncedValue(query, 120);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();

  const results = usePlayerSearch(debouncedQuery, 10);

  // Snapshot prices when modal opens — avoids subscribing to live price updates
  // that would re-render the entire result list every second
  const pricesRef = useRef<Record<string, number>>({});
  useEffect(() => {
    if (opened) {
      const cached = queryClient.getQueryData<{ prices: Record<string, number> }>(marketKeys.bulkPrices);
      if (cached?.prices) pricesRef.current = cached.prices;
    }
  }, [opened, queryClient]);

  useEffect(() => {
    if (opened) {
      setQuery('');
      setSelectedIndex(0);
      // Use requestAnimationFrame instead of setTimeout — focuses on next paint frame
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [opened]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [results.length]);

  const handleSelect = useCallback(
    (player: SymbolInfoResponse) => {
      onSelect(player.symbol_id);
      onClose();
    },
    [onSelect, onClose]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && results[selectedIndex]) {
        e.preventDefault();
        handleSelect(results[selectedIndex]);
      }
    },
    [results, selectedIndex, handleSelect]
  );

  useHotkeys([['mod+K', () => !opened && onClose()]]);

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      withCloseButton={false}
      size="md"
      padding={0}
      centered
      keepMounted
      overlayProps={{ backgroundOpacity: 0.4, blur: 0 }}
      transitionProps={{ duration: 0 }}
    >
      <Box p="md" style={{ borderBottom: '1px solid var(--border-subtle)' }}>
        <TextInput
          ref={inputRef}
          placeholder="Search players \u2014 name, position, or team..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          leftSection={<IconSearch size={18} />}
          size="md"
          variant="unstyled"
        />
      </Box>

      <ScrollArea style={{ maxHeight: 400 }}>
        {!query.trim() ? (
          <Stack align="center" justify="center" p="xl">
            <Text c="dimmed" size="sm">Start typing to search players</Text>
            <Text c="dimmed" size="xs">Ctrl+K to open anywhere</Text>
          </Stack>
        ) : results.length === 0 ? (
          <Stack align="center" justify="center" p="xl">
            <Text c="dimmed" size="sm">No players found for &quot;{query}&quot;</Text>
          </Stack>
        ) : (
          <Stack gap={0}>
            {results.map((player, idx) => (
              <SearchResultRow
                key={player.symbol_id}
                player={player}
                price={pricesRef.current[player.symbol_id.toString()]}
                selected={idx === selectedIndex}
                onHover={() => setSelectedIndex(idx)}
                onSelect={() => handleSelect(player)}
              />
            ))}
          </Stack>
        )}
      </ScrollArea>

      <Group
        px="md"
        py="xs"
        gap="lg"
        style={{ borderTop: '1px solid var(--border-subtle)' }}
      >
        <Group gap={4}>
          <Kbd size="xs">&uarr;&darr;</Kbd>
          <Text size="xs" c="dimmed">navigate</Text>
        </Group>
        <Group gap={4}>
          <Kbd size="xs">&crarr;</Kbd>
          <Text size="xs" c="dimmed">select</Text>
        </Group>
        <Group gap={4}>
          <Kbd size="xs">esc</Kbd>
          <Text size="xs" c="dimmed">close</Text>
        </Group>
      </Group>
    </Modal>
  );
}

// Memoized row — only re-renders when its own props change, not when
// parent re-renders due to query/selectedIndex changes on other rows
const SearchResultRow = memo(function SearchResultRow({
  player,
  price,
  selected,
  onHover,
  onSelect,
}: {
  player: SymbolInfoResponse;
  price: number | undefined;
  selected: boolean;
  onHover: () => void;
  onSelect: () => void;
}) {
  return (
    <UnstyledButton
      w="100%"
      px="md"
      py="sm"
      bg={selected ? 'dark.4' : undefined}
      style={{ transition: 'background-color 0.1s ease' }}
      onMouseEnter={onHover}
      onClick={onSelect}
    >
      <Group justify="space-between">
        <Group gap="sm">
          <Image
            src={getTeamLogoWithFallback(player.team)}
            alt={player.team}
            width={24}
            height={24}
            style={{ objectFit: 'contain' }}
            unoptimized
          />
          <Box>
            <Text size="sm" fw={500}>
              {player.name}
            </Text>
            <Group gap={4}>
              <Badge variant="light" color="gold" size="xs">
                {player.position}
              </Badge>
              <Badge variant="light" color="gray" size="xs">
                {player.team}
              </Badge>
            </Group>
          </Box>
        </Group>
        <Text className="mono" size="sm" c="dimmed">
          {formatCents(price)}
        </Text>
      </Group>
    </UnstyledButton>
  );
});
