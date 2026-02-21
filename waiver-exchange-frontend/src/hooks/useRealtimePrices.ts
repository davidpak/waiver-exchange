'use client';

import { supabase } from '@/lib/supabase';
import { supabaseKeys } from '@/hooks/useSupabaseQueries';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import type { RealtimeChannel } from '@supabase/supabase-js';

/**
 * Subscribes to real-time changes on the `rpe_fair_prices` table.
 * On each UPDATE, patches the React Query cache so the UI updates instantly
 * without polling. A 60s fallback refetch is kept as a safety net.
 */
export function useRealtimePrices(enabled = true) {
  const queryClient = useQueryClient();
  const channelRef = useRef<RealtimeChannel | null>(null);

  useEffect(() => {
    if (!enabled) return;

    const channel = supabase
      .channel('realtime-fair-prices')
      .on(
        'postgres_changes',
        {
          event: '*',
          schema: 'public',
          table: 'rpe_fair_prices',
        },
        (payload) => {
          const row = payload.new as { player_id: number; fair_cents: number } | undefined;
          if (!row) return;

          // Patch the prices cache in-place
          queryClient.setQueryData<Record<string, number>>(
            supabaseKeys.fairPrices,
            (oldPrices) => {
              if (!oldPrices) return oldPrices;
              return {
                ...oldPrices,
                [row.player_id.toString()]: row.fair_cents,
              };
            }
          );
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
  }, [enabled, queryClient]);
}
