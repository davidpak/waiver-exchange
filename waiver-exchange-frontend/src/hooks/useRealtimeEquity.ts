'use client';

import { supabase } from '@/lib/supabase';
import { supabaseKeys } from '@/hooks/useSupabaseQueries';
import type { Database } from '@/types/supabase';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import type { RealtimeChannel } from '@supabase/supabase-js';

type EquityRow = Database['public']['Tables']['equity_timeseries']['Row'];

/**
 * Subscribes to real-time INSERTs on the `equity_timeseries` table,
 * filtered to the current user's account. New rows are appended to
 * the React Query cache so the equity chart updates live.
 */
export function useRealtimeEquity(accountId: number | null, enabled = true) {
  const queryClient = useQueryClient();
  const channelRef = useRef<RealtimeChannel | null>(null);

  useEffect(() => {
    if (!enabled || accountId == null) return;

    const channel = supabase
      .channel(`realtime-equity-${accountId}`)
      .on(
        'postgres_changes',
        {
          event: 'INSERT',
          schema: 'public',
          table: 'equity_timeseries',
          filter: `account_id=eq.${accountId}`,
        },
        (payload) => {
          const newRow = payload.new as EquityRow;

          // Append the new equity snapshot to all matching range caches
          const ranges = ['1d', '1w', '1m', '3m', 'all'] as const;
          for (const range of ranges) {
            queryClient.setQueryData<EquityRow[]>(
              supabaseKeys.equityHistory(accountId, range),
              (old) => {
                if (!old) return old;
                return [...old, newRow];
              }
            );
          }

          // Also update the account summary cache
          queryClient.invalidateQueries({
            queryKey: supabaseKeys.accountSummary(accountId),
          });
        }
      )
      .subscribe();

    channelRef.current = channel;

    return () => {
      if (channelRef.current) {
        supabase.removeChannel(channelRef.current);
        channelRef.current = null;
      }
    };
  }, [enabled, accountId, queryClient]);
}
