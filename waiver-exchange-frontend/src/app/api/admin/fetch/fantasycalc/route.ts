import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';

const FANTASYCALC_URL =
  'https://api.fantasycalc.com/values/current?isDynasty=true&numQbs=1&numTeams=12&ppr=1';

interface FantasyCalcPlayer {
  player: {
    name: string;
    position: string;
    team: string;
    maybePlayerId?: string;
  };
  value: number;
  overallRank?: number;
}

export async function POST(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    const body = await request.json().catch(() => ({}));
    const season: number = body.season ?? new Date().getFullYear();
    const week: number = body.week ?? 0;

    const res = await fetch(FANTASYCALC_URL);
    if (!res.ok) {
      return NextResponse.json(
        { error: `FantasyCalc API returned ${res.status}` },
        { status: 502 }
      );
    }

    const data: FantasyCalcPlayer[] = await res.json();
    const now = new Date().toISOString();

    // Delete existing rows for this source/season/week to avoid duplicates
    await supabaseAdmin
      .from('source_values')
      .delete()
      .eq('source', 'fantasycalc')
      .eq('season', season)
      .eq('week', week);

    // Build rows
    const rows = data.map((item) => ({
      source: 'fantasycalc' as const,
      season,
      week,
      player_name: item.player.name,
      position: item.player.position || null,
      team: item.player.team || null,
      raw_value: item.value,
      source_player_id: item.player.maybePlayerId || null,
      fetched_at: now,
      meta: { overallRank: item.overallRank } as Record<string, unknown>,
    }));

    // Insert in batches of 500
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

    // Log admin action
    await supabaseAdmin.from('admin_actions').insert({
      account_id: accountId,
      action: 'fetch_source',
      details: { source: 'fantasycalc', count: insertedCount, fetchedAt: now },
    });

    return NextResponse.json({
      count: insertedCount,
      source: 'fantasycalc',
      fetchedAt: now,
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
