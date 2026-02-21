import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';
import { normalizeName } from '@/lib/player-matching';

/**
 * POST: Sync player_metadata from player_id_mapping (canonical symbol IDs)
 * and enrich with team/position data from source_values.
 *
 * player_id_mapping is the source of truth for our_symbol_id — the trading
 * engine uses these IDs. This step ensures player_metadata is populated
 * with the correct symbol IDs so the pricing pipeline works.
 */
export async function POST(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    const body = await request.json().catch(() => ({}));
    const season: number = body.season ?? new Date().getFullYear();
    const week: number = body.week ?? 0;

    // 1. Load canonical players from player_id_mapping
    const { data: mappingRows, error: mapErr } = await supabaseAdmin
      .from('player_id_mapping')
      .select('sportsdataio_player_id, our_symbol_id, player_name, team, position');

    if (mapErr) {
      return NextResponse.json(
        { error: `Failed to load player_id_mapping: ${mapErr.message}` },
        { status: 500 }
      );
    }

    if (!mappingRows || mappingRows.length === 0) {
      return NextResponse.json(
        { error: 'player_id_mapping table is empty — run the player mapping script first' },
        { status: 400 }
      );
    }

    // 2. Load source_values to enrich team/position data (sources often
    //    have more up-to-date team info than the mapping table)
    const { data: sourceValues } = await supabaseAdmin
      .from('source_values')
      .select('player_name, position, team')
      .eq('season', season)
      .eq('week', week)
      .limit(10000);

    // Build a normalized name → latest team/position lookup from sources
    const sourceInfo = new Map<string, { position: string; team: string }>();
    for (const sv of sourceValues ?? []) {
      if (!sv.player_name) continue;
      const norm = normalizeName(sv.player_name);
      if (!sourceInfo.has(norm) && sv.position && sv.team) {
        sourceInfo.set(norm, {
          position: sv.position.toUpperCase(),
          team: sv.team.toUpperCase(),
        });
      }
    }

    // 3. Load existing player_metadata to detect changes
    const { data: existing } = await supabaseAdmin
      .from('player_metadata')
      .select('player_id, symbol_id');

    const existingByPlayerId = new Set<string>();
    for (const p of existing ?? []) {
      existingByPlayerId.add(p.player_id);
    }

    // 4. Build upsert rows from player_id_mapping
    const rows = mappingRows.map((m) => {
      const norm = normalizeName(m.player_name);
      const enriched = sourceInfo.get(norm);

      return {
        player_id: String(m.sportsdataio_player_id),
        name: m.player_name,
        position: enriched?.position || (m.position || '').toUpperCase() || 'UNK',
        team: enriched?.team || (m.team || '').toUpperCase() || 'UNK',
        symbol_id: m.our_symbol_id,
      };
    });

    // 5. Upsert into player_metadata in batches
    const BATCH_SIZE = 500;
    let upsertedCount = 0;
    for (let i = 0; i < rows.length; i += BATCH_SIZE) {
      const batch = rows.slice(i, i + BATCH_SIZE);
      const { error } = await supabaseAdmin
        .from('player_metadata')
        .upsert(batch, { onConflict: 'player_id' });
      if (error) {
        return NextResponse.json(
          { error: `Upsert failed at batch ${Math.floor(i / BATCH_SIZE)}: ${error.message}` },
          { status: 500 }
        );
      }
      upsertedCount += batch.length;
    }

    const newCount = rows.filter((r) => !existingByPlayerId.has(r.player_id)).length;

    // 6. Log admin action
    await supabaseAdmin.from('admin_actions').insert({
      account_id: accountId,
      action: 'sync_player_metadata',
      details: {
        season,
        week,
        fromMapping: mappingRows.length,
        upserted: upsertedCount,
        newPlayers: newCount,
        enrichedFromSources: sourceInfo.size,
      },
    });

    return NextResponse.json({
      count: upsertedCount,
      totalInUniverse: upsertedCount,
      newPlayers: newCount,
      fromMapping: mappingRows.length,
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
