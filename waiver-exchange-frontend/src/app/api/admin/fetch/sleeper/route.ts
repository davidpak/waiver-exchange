import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';

export async function POST(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    const body = await request.json().catch(() => ({}));
    const season: number = body.season ?? new Date().getFullYear();
    const week: number = body.week ?? 0;

    // Fetch both projections and stats in parallel
    const [projRes, statsRes] = await Promise.all([
      fetch(`https://api.sleeper.app/v1/projections/nfl/regular/${season}`),
      fetch(`https://api.sleeper.app/v1/stats/nfl/regular/${season}`),
    ]);

    if (!projRes.ok) {
      return NextResponse.json(
        { error: `Sleeper projections API returned ${projRes.status}` },
        { status: 502 }
      );
    }
    if (!statsRes.ok) {
      return NextResponse.json(
        { error: `Sleeper stats API returned ${statsRes.status}` },
        { status: 502 }
      );
    }

    // Sleeper returns { [player_id]: { pts_ppr, ... } }
    const projections: Record<string, SleeperPlayerData> = await projRes.json();
    const stats: Record<string, SleeperPlayerData> = await statsRes.json();

    // We also need player names — fetch the players list
    const playersRes = await fetch('https://api.sleeper.app/v1/players/nfl');
    if (!playersRes.ok) {
      return NextResponse.json(
        { error: `Sleeper players API returned ${playersRes.status}` },
        { status: 502 }
      );
    }
    const players: Record<string, SleeperPlayer> = await playersRes.json();

    const now = new Date().toISOString();

    // Delete existing rows for this source/season/week to avoid duplicates
    await supabaseAdmin
      .from('source_values')
      .delete()
      .eq('source', 'sleeper_proj')
      .eq('season', season)
      .eq('week', week);
    await supabaseAdmin
      .from('source_values')
      .delete()
      .eq('source', 'sleeper_stats')
      .eq('season', season)
      .eq('week', week);

    const rows: InsertRow[] = [];

    // Process projections
    for (const [playerId, data] of Object.entries(projections)) {
      const player = players[playerId];
      if (!player || !player.full_name) continue;
      const pts = data.pts_ppr ?? data.pts_half_ppr ?? data.pts_std ?? 0;
      if (pts <= 0) continue;

      rows.push({
        source: 'sleeper_proj',
        season,
        week,
        player_name: player.full_name,
        position: player.position || null,
        team: player.team || null,
        raw_value: pts,
        source_player_id: playerId,
        fetched_at: now,
        meta: {
          pts_ppr: data.pts_ppr,
          pts_half_ppr: data.pts_half_ppr,
          pts_std: data.pts_std,
        } as Record<string, unknown>,
      });
    }

    // Process stats
    for (const [playerId, data] of Object.entries(stats)) {
      const player = players[playerId];
      if (!player || !player.full_name) continue;
      const pts = data.pts_ppr ?? data.pts_half_ppr ?? data.pts_std ?? 0;
      if (pts <= 0) continue;

      rows.push({
        source: 'sleeper_stats',
        season,
        week,
        player_name: player.full_name,
        position: player.position || null,
        team: player.team || null,
        raw_value: pts,
        source_player_id: playerId,
        fetched_at: now,
        meta: {
          gp: data.gp,
          pts_ppr: data.pts_ppr,
        } as Record<string, unknown>,
      });
    }

    // Insert in batches
    const BATCH_SIZE = 500;
    let insertedCount = 0;
    for (let i = 0; i < rows.length; i += BATCH_SIZE) {
      const batch = rows.slice(i, i + BATCH_SIZE);
      const { error } = await supabaseAdmin.from('source_values').insert(batch);
      if (error) {
        return NextResponse.json(
          { error: `Insert failed at batch ${Math.floor(i / BATCH_SIZE)}: ${error.message}` },
          { status: 500 }
        );
      }
      insertedCount += batch.length;
    }

    await supabaseAdmin.from('admin_actions').insert({
      account_id: accountId,
      action: 'fetch_source',
      details: {
        source: 'sleeper',
        projections: Object.keys(projections).length,
        stats: Object.keys(stats).length,
        inserted: insertedCount,
        fetchedAt: now,
      },
    });

    return NextResponse.json({
      count: insertedCount,
      source: 'sleeper',
      fetchedAt: now,
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

// ─── Sleeper API types ───────────────────────────────────────────────────────

interface SleeperPlayerData {
  pts_ppr?: number;
  pts_half_ppr?: number;
  pts_std?: number;
  gp?: number;
  [key: string]: unknown;
}

interface SleeperPlayer {
  full_name?: string;
  position?: string;
  team?: string;
  [key: string]: unknown;
}

interface InsertRow {
  source: string;
  season: number;
  week: number;
  player_name: string;
  position: string | null;
  team: string | null;
  raw_value: number;
  source_player_id: string | null;
  fetched_at: string;
  meta: Record<string, unknown>;
}
