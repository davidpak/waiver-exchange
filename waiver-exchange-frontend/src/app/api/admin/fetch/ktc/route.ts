import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';

// KTC embeds player data as a JS variable in the dynasty rankings page
const KTC_URL = 'https://keeptradecut.com/dynasty-rankings';

const POSITION_MAP: Record<number, string> = {
  1: 'QB',
  2: 'RB',
  3: 'WR',
  4: 'TE',
};

export async function POST(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    const body = await request.json().catch(() => ({}));
    const season: number = body.season ?? new Date().getFullYear();
    const week: number = body.week ?? 0;

    const res = await fetch(KTC_URL, {
      headers: {
        'Accept': 'text/html',
        'User-Agent': 'Mozilla/5.0 (compatible; WaiverExchange/1.0)',
      },
    });
    if (!res.ok) {
      return NextResponse.json(
        { error: `KTC page returned ${res.status}` },
        { status: 502 }
      );
    }

    const html = await res.text();

    // Extract the playersArray JSON from the embedded script
    const match = html.match(/var\s+playersArray\s*=\s*(\[[\s\S]*?\]);\s*(?:var|let|const|function|<\/script>)/);
    if (!match) {
      return NextResponse.json(
        { error: 'Could not find playersArray in KTC page' },
        { status: 502 }
      );
    }

    let data: any[];
    try {
      data = JSON.parse(match[1]);
    } catch {
      return NextResponse.json(
        { error: 'Failed to parse playersArray JSON from KTC page' },
        { status: 502 }
      );
    }

    const now = new Date().toISOString();

    // Delete existing rows for this source/season/week to avoid duplicates
    await supabaseAdmin
      .from('source_values')
      .delete()
      .eq('source', 'ktc')
      .eq('season', season)
      .eq('week', week);

    const rows = data
      .filter((item: any) => item.playerName && item.oneQBValues)
      .map((item: any) => ({
        source: 'ktc' as const,
        season,
        week,
        player_name: item.playerName,
        position: POSITION_MAP[item.positionID] || item.position || null,
        team: item.team || null,
        raw_value: item.oneQBValues?.value ?? 0,
        source_player_id: item.playerID ? String(item.playerID) : null,
        fetched_at: now,
        meta: {
          slug: item.slug,
          rank: item.oneQBValues?.rank,
          positionalRank: item.oneQBValues?.positionalRank,
          superflexValue: item.superflexValues?.value,
        } as Record<string, unknown>,
      }));

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
      details: { source: 'ktc', count: insertedCount, fetchedAt: now },
    });

    return NextResponse.json({
      count: insertedCount,
      source: 'ktc',
      fetchedAt: now,
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
