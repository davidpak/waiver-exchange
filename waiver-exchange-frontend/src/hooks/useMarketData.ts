'use client';

import { apiClient } from '@/lib/api-client';
import { cachePlayersToIDB, readPlayersFromIDB, type CachedPlayer } from '@/lib/dexie-db';
import { supabase } from '@/lib/supabase';
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
// IndexedDB cache helpers (replaces localStorage)
// ---------------------------------------------------------------------------

async function readPlayersCache(): Promise<SymbolInfoResponse[] | undefined> {
  try {
    const cached = await readPlayersFromIDB();
    if (!cached.length) return undefined;

    // Map CachedPlayer back to SymbolInfoResponse shape
    return cached.map((p) => ({
      player_id: p.player_id,
      name: p.name,
      position: p.position,
      team: p.team,
      projected_points: p.projected_points ?? 0,
      rank: p.rank ?? 0,
      symbol_id: p.symbol_id ?? 0,
      last_updated: p.last_updated ?? '',
    }));
  } catch {
    return undefined;
  }
}

function writePlayersCache(players: SymbolInfoResponse[]): void {
  const mapped: CachedPlayer[] = players.map((p) => ({
    player_id: p.player_id,
    name: p.name,
    position: p.position,
    team: p.team,
    projected_points: p.projected_points,
    rank: p.rank,
    symbol_id: p.symbol_id,
    last_updated: p.last_updated,
  }));

  // Fire-and-forget — async write to IndexedDB
  cachePlayersToIDB(mapped).catch(() => {
    // IndexedDB unavailable — silently ignore
  });
}

// ---------------------------------------------------------------------------
// Shared queryFn — fetches players and persists to IndexedDB
// ---------------------------------------------------------------------------
async function fetchAndCachePlayers(): Promise<SymbolInfoResponse[]> {
  const players = await apiClient.rest.getAllPlayers();
  writePlayersCache(players);
  return players;
}

// ---------------------------------------------------------------------------
// useAllPlayers — seeded from IndexedDB, fetched once, cached forever
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
// useBulkPrices — reads fair prices directly from Supabase rpe_fair_prices
// ---------------------------------------------------------------------------
export function useBulkPrices(refetchInterval = 2000) {
  return useQuery<{ prices: Record<string, number> }>({
    queryKey: marketKeys.bulkPrices,
    queryFn: async () => {
      const { data, error } = await supabase
        .from('rpe_fair_prices')
        .select('player_id, fair_cents');

      if (error) throw error;

      const prices: Record<string, number> = {};
      for (const row of data ?? []) {
        prices[(row as any).player_id.toString()] = (row as any).fair_cents;
      }
      return { prices };
    },
    refetchInterval,
    staleTime: Math.max(refetchInterval / 2, 500),
  });
}

// ---------------------------------------------------------------------------
// usePlayerSearch — pure local filter over cached players (zero network)
// ---------------------------------------------------------------------------

interface SearchEntry {
  player: SymbolInfoResponse;
  nameLower: string;
  positionLower: string;
  teamLower: string;
}

export function usePlayerSearch(query: string, limit = 10) {
  const { data: allPlayers } = useAllPlayers();

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
// Prefetch helper — seeds cache from IndexedDB, then fetches in background
// ---------------------------------------------------------------------------
export function usePrefetchMarketData() {
  const queryClient = useQueryClient();

  useEffect(() => {
    // 1. Seed React Query from IndexedDB so widgets render immediately
    readPlayersCache().then((cached) => {
      if (cached) {
        queryClient.setQueryData(marketKeys.allPlayers, cached);
      }
    });

    // 2. Always fetch fresh data in background (updates IndexedDB on success)
    queryClient.fetchQuery({
      queryKey: marketKeys.allPlayers,
      queryFn: fetchAndCachePlayers,
      staleTime: 0,
    });
  }, [queryClient]);
}
