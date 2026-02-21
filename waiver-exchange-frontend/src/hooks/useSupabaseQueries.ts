'use client';

import { supabase } from '@/lib/supabase';
import type { Database } from '@/types/supabase';
import { useQuery } from '@tanstack/react-query';

// Feature flag for gradual migration
const USE_SUPABASE_DIRECT = process.env.NEXT_PUBLIC_USE_SUPABASE_DIRECT === 'true';

// ---------------------------------------------------------------------------
// Query keys
// ---------------------------------------------------------------------------
export const supabaseKeys = {
  playerMetadata: ['supabase', 'player-metadata'] as const,
  fairPrices: ['supabase', 'fair-prices'] as const,
  myTrades: (accountId: number) => ['supabase', 'trades', accountId] as const,
  myPositions: (accountId: number) => ['supabase', 'positions', accountId] as const,
  equityHistory: (accountId: number, range: string) =>
    ['supabase', 'equity-history', accountId, range] as const,
  accountSummary: (accountId: number) => ['supabase', 'account-summary', accountId] as const,
  priceHistory: (symbolId: number, period: string) =>
    ['supabase', 'price-history', symbolId, period] as const,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
type PlayerMetadataRow = Database['public']['Tables']['player_metadata']['Row'];
type FairPriceRow = Database['public']['Tables']['rpe_fair_prices']['Row'];
type TradeRow = Database['public']['Tables']['trades']['Row'];
type PositionRow = Database['public']['Tables']['positions']['Row'];
type EquityRow = Database['public']['Tables']['equity_timeseries']['Row'];
type AccountRow = Database['public']['Tables']['accounts']['Row'];

// ---------------------------------------------------------------------------
// usePlayerMetadata — replaces GET /api/symbols/all
// ---------------------------------------------------------------------------
export function usePlayerMetadata(enabled = USE_SUPABASE_DIRECT) {
  return useQuery<PlayerMetadataRow[]>({
    queryKey: supabaseKeys.playerMetadata,
    queryFn: async () => {
      const { data, error } = await supabase
        .from('player_metadata')
        .select('*')
        .order('rank', { ascending: true });

      if (error) throw error;
      return data;
    },
    staleTime: Infinity,
    gcTime: Infinity,
    enabled,
  });
}

// ---------------------------------------------------------------------------
// useFairPrices — replaces GET /api/symbols/prices
// ---------------------------------------------------------------------------
export function useFairPrices(refetchInterval = 2000, enabled = USE_SUPABASE_DIRECT) {
  return useQuery<Record<string, number>>({
    queryKey: supabaseKeys.fairPrices,
    queryFn: async () => {
      const { data, error } = await supabase
        .from('rpe_fair_prices')
        .select('player_id, fair_cents') as { data: { player_id: number; fair_cents: number }[] | null; error: unknown };

      if (error) throw error;

      // Transform to { symbol_id: price } map to match existing API format
      const prices: Record<string, number> = {};
      for (const row of data ?? []) {
        prices[row.player_id.toString()] = row.fair_cents;
      }
      return prices;
    },
    refetchInterval,
    staleTime: Math.max(refetchInterval / 2, 500),
    enabled,
  });
}

// ---------------------------------------------------------------------------
// useMyTrades — replaces GET /api/account/trades
// ---------------------------------------------------------------------------
export function useMyTrades(accountId: number | null, enabled = USE_SUPABASE_DIRECT) {
  return useQuery<TradeRow[]>({
    queryKey: supabaseKeys.myTrades(accountId ?? 0),
    queryFn: async () => {
      const { data, error } = await supabase
        .from('trades')
        .select('*')
        .eq('account_id', accountId!)
        .order('timestamp', { ascending: false })
        .limit(100);

      if (error) throw error;
      return data;
    },
    enabled: enabled && accountId != null,
    staleTime: 5000,
  });
}

// ---------------------------------------------------------------------------
// useMyPositions — replaces GET /api/account/positions
// ---------------------------------------------------------------------------
export function useMyPositions(accountId: number | null, enabled = USE_SUPABASE_DIRECT) {
  return useQuery<PositionRow[]>({
    queryKey: supabaseKeys.myPositions(accountId ?? 0),
    queryFn: async () => {
      const { data, error } = await supabase
        .from('positions')
        .select('*')
        .eq('account_id', accountId!);

      if (error) throw error;
      return data;
    },
    enabled: enabled && accountId != null,
    staleTime: 5000,
  });
}

// ---------------------------------------------------------------------------
// useEquityHistory — replaces GET /api/account/equity-history
// ---------------------------------------------------------------------------
export function useEquityHistory(
  accountId: number | null,
  range: '1d' | '1w' | '1m' | '3m' | 'all' = '1d',
  enabled = USE_SUPABASE_DIRECT
) {
  return useQuery<EquityRow[]>({
    queryKey: supabaseKeys.equityHistory(accountId ?? 0, range),
    queryFn: async () => {
      const now = new Date();
      let since: Date;

      switch (range) {
        case '1d':
          since = new Date(now.getTime() - 24 * 60 * 60 * 1000);
          break;
        case '1w':
          since = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
          break;
        case '1m':
          since = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);
          break;
        case '3m':
          since = new Date(now.getTime() - 90 * 24 * 60 * 60 * 1000);
          break;
        case 'all':
        default:
          since = new Date(0);
          break;
      }

      const { data, error } = await supabase
        .from('equity_timeseries')
        .select('*')
        .eq('account_id', accountId!)
        .gte('timestamp', since.toISOString())
        .order('timestamp', { ascending: true });

      if (error) throw error;
      return data;
    },
    enabled: enabled && accountId != null,
    staleTime: 10000,
  });
}

// ---------------------------------------------------------------------------
// useAccountSummary — replaces GET /api/account/summary
// ---------------------------------------------------------------------------
export function useAccountSummary(accountId: number | null, enabled = USE_SUPABASE_DIRECT) {
  return useQuery<AccountRow | null>({
    queryKey: supabaseKeys.accountSummary(accountId ?? 0),
    queryFn: async () => {
      const { data, error } = await supabase
        .from('accounts')
        .select('*')
        .eq('id', accountId!)
        .single();

      if (error) throw error;
      return data;
    },
    enabled: enabled && accountId != null,
    staleTime: 5000,
  });
}

// ---------------------------------------------------------------------------
// usePriceHistory — replaces GET /api/price-history/:symbolId
// ---------------------------------------------------------------------------
export function usePriceHistory(
  symbolId: number | null,
  period: '1d' | '1w' | '1m' = '1d',
  enabled = USE_SUPABASE_DIRECT
) {
  return useQuery({
    queryKey: supabaseKeys.priceHistory(symbolId ?? 0, period),
    queryFn: async () => {
      const now = new Date();
      let since: Date;

      switch (period) {
        case '1d':
          since = new Date(now.getTime() - 24 * 60 * 60 * 1000);
          break;
        case '1w':
          since = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
          break;
        case '1m':
          since = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);
          break;
        default:
          since = new Date(now.getTime() - 24 * 60 * 60 * 1000);
      }

      const { data, error } = await supabase
        .from('price_history')
        .select('*')
        .eq('symbol_id', symbolId!)
        .gte('timestamp', since.toISOString())
        .order('timestamp', { ascending: true });

      if (error) throw error;
      return data;
    },
    enabled: enabled && symbolId != null,
    staleTime: 5000,
  });
}
