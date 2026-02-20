'use client';

import { apiClient } from '@/lib/api-client';
import type { SnapshotResponse, SymbolInfoResponse } from '@/types/api';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';

// ---------------------------------------------------------------------------
// Query keys (single source of truth)
// ---------------------------------------------------------------------------
export const marketKeys = {
  allPlayers: ['all-players'] as const,
  bulkPrices: ['bulk-prices'] as const,
  snapshot: ['snapshot-current'] as const,
};

// ---------------------------------------------------------------------------
// localStorage cache for player data
// ---------------------------------------------------------------------------
const PLAYERS_CACHE_KEY = 'waiver-exchange-players';

function readPlayersCache(): SymbolInfoResponse[] | undefined {
  try {
    const raw = localStorage.getItem(PLAYERS_CACHE_KEY);
    if (!raw) return undefined;
    return JSON.parse(raw) as SymbolInfoResponse[];
  } catch {
    return undefined;
  }
}

function writePlayersCache(players: SymbolInfoResponse[]) {
  try {
    localStorage.setItem(PLAYERS_CACHE_KEY, JSON.stringify(players));
  } catch {
    // localStorage full or unavailable — silently ignore
  }
}

// ---------------------------------------------------------------------------
// Shared queryFn — fetches players and persists to localStorage
// ---------------------------------------------------------------------------
async function fetchAndCachePlayers(): Promise<SymbolInfoResponse[]> {
  const players = await apiClient.rest.getAllPlayers();
  writePlayersCache(players);
  return players;
}

// ---------------------------------------------------------------------------
// useAllPlayers — seeded from localStorage, fetched once, cached forever
// ---------------------------------------------------------------------------
export function useAllPlayers() {
  return useQuery<SymbolInfoResponse[]>({
    queryKey: marketKeys.allPlayers,
    queryFn: fetchAndCachePlayers,
    staleTime: Infinity,
    gcTime: Infinity,
  });
}

// ---------------------------------------------------------------------------
// useBulkPrices — shared price query with configurable refetch
// ---------------------------------------------------------------------------
export function useBulkPrices(refetchInterval = 2000) {
  return useQuery<{ prices: Record<string, number> }>({
    queryKey: marketKeys.bulkPrices,
    queryFn: () => apiClient.rest.getAllPrices(),
    refetchInterval,
    staleTime: Math.max(refetchInterval / 2, 500),
  });
}

// ---------------------------------------------------------------------------
// usePlayerSearch — pure local filter over cached players (zero network)
// ---------------------------------------------------------------------------

// Pre-computed search entry to avoid repeated .toLowerCase() calls
interface SearchEntry {
  player: SymbolInfoResponse;
  nameLower: string;
  positionLower: string;
  teamLower: string;
}

export function usePlayerSearch(query: string, limit = 10) {
  const { data: allPlayers } = useAllPlayers();

  // Build search index once when player data loads (not on every keystroke)
  const searchIndex = useMemo<SearchEntry[]>(() => {
    if (!allPlayers) return [];
    return allPlayers.map((p) => ({
      player: p,
      nameLower: p.name.toLowerCase(),
      positionLower: p.position.toLowerCase(),
      teamLower: p.team.toLowerCase(),
    }));
  }, [allPlayers]);

  return useMemo(() => {
    if (!query.trim() || !searchIndex.length) return [];
    const q = query.toLowerCase();
    return searchIndex
      .filter(
        (e) =>
          e.nameLower.includes(q) ||
          e.positionLower.includes(q) ||
          e.teamLower.includes(q)
      )
      .sort((a, b) => {
        const aStart = a.nameLower.startsWith(q);
        const bStart = b.nameLower.startsWith(q);
        if (aStart && !bStart) return -1;
        if (!aStart && bStart) return 1;
        return a.player.rank - b.player.rank;
      })
      .slice(0, limit)
      .map((e) => e.player);
  }, [query, searchIndex, limit]);
}

// ---------------------------------------------------------------------------
// useCurrentSnapshot — consolidates the duplicated snapshot query
// ---------------------------------------------------------------------------
export function useCurrentSnapshot() {
  return useQuery<SnapshotResponse>({
    queryKey: marketKeys.snapshot,
    queryFn: () => apiClient.rest.getCurrentSnapshot(),
    refetchInterval: 1000,
    staleTime: 500,
  });
}

// ---------------------------------------------------------------------------
// Prefetch helper — seeds cache from localStorage, then fetches in background
// ---------------------------------------------------------------------------
export function usePrefetchMarketData() {
  const queryClient = useQueryClient();

  useEffect(() => {
    // 1. Seed React Query from localStorage so widgets render immediately
    const cached = readPlayersCache();
    if (cached) {
      queryClient.setQueryData(marketKeys.allPlayers, cached);
    }

    // 2. Always fetch fresh data in background (updates localStorage on success)
    queryClient.fetchQuery({
      queryKey: marketKeys.allPlayers,
      queryFn: fetchAndCachePlayers,
      staleTime: 0, // force fetch regardless of cache state
    });
  }, [queryClient]);
}
