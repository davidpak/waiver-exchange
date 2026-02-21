import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';

/**
 * DELETE: Clear all price data to start fresh.
 * Deletes from rpe_fair_prices, and optionally source_values + mappings.
 */
export async function DELETE(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    const body = await request.json().catch(() => ({}));
    const full: boolean = body.full === true;

    const { error: rpeErr } = await supabaseAdmin
      .from('rpe_fair_prices')
      .delete()
      .gte('player_id', 0);

    if (rpeErr) {
      return NextResponse.json(
        { error: `Failed to clear rpe_fair_prices: ${rpeErr.message}` },
        { status: 500 }
      );
    }

    let clearedSources = false;
    let clearedMappings = false;

    if (full) {
      await supabaseAdmin
        .from('source_values')
        .delete()
        .gte('id', 0);
      clearedSources = true;

      await supabaseAdmin
        .from('player_source_mapping')
        .delete()
        .gte('symbol_id', 0);
      clearedMappings = true;
    }

    await supabaseAdmin.from('admin_actions').insert({
      account_id: accountId,
      action: 'reset_prices',
      details: { full, clearedSources, clearedMappings },
    });

    return NextResponse.json({
      cleared: ['rpe_fair_prices'],
      ...(full ? { alsoClearedSources: true, alsoClearedMappings: true } : {}),
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

/**
 * GET: Return summary of what data exists.
 */
export async function GET(request: Request) {
  try {
    await verifyAdmin(request);

    // Get season/week info from rpe_fair_prices
    const { data: priceRows } = await supabaseAdmin
      .from('rpe_fair_prices')
      .select('season, week, ts')
      .order('ts', { ascending: false })
      .limit(1000);

    // Group by season/week
    const weekMap = new Map<string, { season: number; week: number; count: number; lastCalculated: string }>();
    for (const row of priceRows ?? []) {
      const key = `${row.season}-${row.week}`;
      const existing = weekMap.get(key);
      if (existing) {
        existing.count++;
        if (row.ts > existing.lastCalculated) {
          existing.lastCalculated = row.ts;
        }
      } else {
        weekMap.set(key, {
          season: row.season,
          week: row.week ?? 0,
          count: 1,
          lastCalculated: row.ts ?? '',
        });
      }
    }

    // Count rows in rpe_fair_prices
    const { count: rpeCount } = await supabaseAdmin
      .from('rpe_fair_prices')
      .select('*', { count: 'exact', head: true });

    // Count source_values
    const { count: sourceCount } = await supabaseAdmin
      .from('source_values')
      .select('*', { count: 'exact', head: true });

    return NextResponse.json({
      rpeFairPrices: rpeCount ?? 0,
      sourceValues: sourceCount ?? 0,
      calculatedWeeks: Array.from(weekMap.values()),
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
