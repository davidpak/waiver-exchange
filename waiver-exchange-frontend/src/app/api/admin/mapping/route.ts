import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';
import { autoMatch, type PlayerMetadata, type SourcePlayer } from '@/lib/player-matching';

/**
 * GET: Returns players that exist in player_metadata but have no mapping
 *      in player_source_mapping for one or more sources.
 */
export async function GET(request: Request) {
  try {
    await verifyAdmin(request);

    // Load all players with symbol_ids
    const { data: players, error: pErr } = await supabaseAdmin
      .from('player_metadata')
      .select('symbol_id, name, position, team')
      .not('symbol_id', 'is', null);

    if (pErr || !players) {
      return NextResponse.json(
        { error: `Failed to load players: ${pErr?.message}` },
        { status: 500 }
      );
    }

    // Load existing mappings (paginated — can exceed 1000)
    const mappings: any[] = [];
    {
      const PAGE = 1000;
      let offset = 0;
      while (true) {
        const { data: page, error: mErr } = await supabaseAdmin
          .from('player_source_mapping')
          .select('symbol_id, source')
          .range(offset, offset + PAGE - 1);
        if (mErr) {
          return NextResponse.json(
            { error: `Failed to load mappings: ${mErr.message}` },
            { status: 500 }
          );
        }
        if (!page || page.length === 0) break;
        mappings.push(...page);
        if (page.length < PAGE) break;
        offset += PAGE;
      }
    }

    // Build set of mapped (symbol_id, source) pairs
    const mapped = new Set<string>();
    for (const m of mappings) {
      mapped.add(`${m.symbol_id}::${m.source}`);
    }

    // Check which sources exist in source_values (paginated)
    const activeSources = new Set<string>();
    {
      const PAGE = 1000;
      let offset = 0;
      while (true) {
        const { data: page } = await supabaseAdmin
          .from('source_values')
          .select('source')
          .range(offset, offset + PAGE - 1);
        if (!page || page.length === 0) break;
        for (const s of page) {
          activeSources.add(s.source);
        }
        if (page.length < PAGE) break;
        offset += PAGE;
      }
    }

    // Find unmatched players per source
    const unmatched: Record<string, { symbolId: number; name: string; position: string; team: string }[]> = {};
    for (const source of activeSources) {
      const missing = players
        .filter((p) => p.symbol_id != null && !mapped.has(`${p.symbol_id}::${source}`))
        .map((p) => ({
          symbolId: p.symbol_id!,
          name: p.name,
          position: p.position,
          team: p.team,
        }));
      if (missing.length > 0) {
        unmatched[source] = missing;
      }
    }

    const totalUnmatched = Object.values(unmatched).reduce((sum, arr) => sum + arr.length, 0);

    return NextResponse.json({
      totalPlayers: players.length,
      totalMappings: mappings?.length ?? 0,
      totalUnmatched,
      activeSources: [...activeSources],
      unmatched,
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

/**
 * POST: Runs auto-matching for all sources that have data in source_values.
 *       Uses fuzzy name + position + team matching.
 */
export async function POST(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    // Load our players
    const { data: players, error: pErr } = await supabaseAdmin
      .from('player_metadata')
      .select('symbol_id, name, position, team')
      .not('symbol_id', 'is', null);

    if (pErr || !players) {
      return NextResponse.json(
        { error: `Failed to load players: ${pErr?.message}` },
        { status: 500 }
      );
    }

    const ourPlayers: PlayerMetadata[] = players
      .filter((p) => p.symbol_id != null)
      .map((p) => ({
        symbolId: p.symbol_id!,
        name: p.name,
        position: p.position,
        team: p.team,
      }));

    // Load existing mappings to avoid re-matching (paginated — can exceed 1000)
    const existingMappings: any[] = [];
    {
      const PAGE = 1000;
      let offset = 0;
      while (true) {
        const { data: page } = await supabaseAdmin
          .from('player_source_mapping')
          .select('symbol_id, source')
          .range(offset, offset + PAGE - 1);
        if (!page || page.length === 0) break;
        existingMappings.push(...page);
        if (page.length < PAGE) break;
        offset += PAGE;
      }
    }

    const alreadyMapped = new Set<string>();
    for (const m of existingMappings) {
      alreadyMapped.add(`${m.symbol_id}::${m.source}`);
    }

    // Get distinct sources from source_values (paginated to find all sources)
    const activeSources = new Set<string>();
    {
      const PAGE = 1000;
      let offset = 0;
      while (true) {
        const { data: page } = await supabaseAdmin
          .from('source_values')
          .select('source')
          .range(offset, offset + PAGE - 1);
        if (!page || page.length === 0) break;
        for (const s of page) {
          activeSources.add(s.source);
        }
        if (page.length < PAGE) break;
        offset += PAGE;
      }
    }

    const results: Record<string, { matched: number; unmatched: number }> = {};
    let totalInserted = 0;

    for (const source of activeSources) {
      // Load latest source values for this source (paginated — Sleeper has >1000)
      const sourceValues: any[] = [];
      {
        const PAGE = 1000;
        let offset = 0;
        while (true) {
          const { data: page } = await supabaseAdmin
            .from('source_values')
            .select('player_name, position, team, source_player_id, raw_value')
            .eq('source', source)
            .order('fetched_at', { ascending: false })
            .range(offset, offset + PAGE - 1);
          if (!page || page.length === 0) break;
          sourceValues.push(...page);
          if (page.length < PAGE) break;
          offset += PAGE;
        }
      }

      if (sourceValues.length === 0) continue;

      // Deduplicate by player name (keep latest)
      const seen = new Set<string>();
      const sourcePlayers: SourcePlayer[] = [];
      for (const sv of sourceValues) {
        const key = sv.player_name.toLowerCase();
        if (seen.has(key)) continue;
        seen.add(key);
        sourcePlayers.push({
          name: sv.player_name,
          position: sv.position ?? undefined,
          team: sv.team ?? undefined,
          sourceId: sv.source_player_id ?? undefined,
          value: sv.raw_value,
        });
      }

      // Filter out players that already have a mapping for this source
      const unmappedPlayers = ourPlayers.filter(
        (p) => !alreadyMapped.has(`${p.symbolId}::${source}`)
      );

      if (unmappedPlayers.length === 0) {
        results[source] = { matched: 0, unmatched: 0 };
        continue;
      }

      const { matched, unmatched } = autoMatch(unmappedPlayers, sourcePlayers);

      // Insert new mappings
      if (matched.length > 0) {
        const rows = matched.map((m) => ({
          symbol_id: m.symbolId,
          source,
          source_player_id: m.sourceId ?? null,
          source_name: m.sourceName,
          match_score: m.score,
          verified: false,
        }));

        const BATCH_SIZE = 500;
        for (let i = 0; i < rows.length; i += BATCH_SIZE) {
          const batch = rows.slice(i, i + BATCH_SIZE);
          const { error } = await supabaseAdmin
            .from('player_source_mapping')
            .upsert(batch, { onConflict: 'symbol_id,source' });
          if (error) {
            return NextResponse.json(
              { error: `Mapping insert failed for ${source}: ${error.message}` },
              { status: 500 }
            );
          }
        }
        totalInserted += matched.length;
      }

      results[source] = { matched: matched.length, unmatched: unmatched.length };
    }

    // Log admin action
    await supabaseAdmin.from('admin_actions').insert({
      account_id: accountId,
      action: 'auto_match_players',
      details: { results, totalInserted },
    });

    return NextResponse.json({
      totalInserted,
      bySource: results,
      debug: {
        playersInMetadata: ourPlayers.length,
        existingMappings: existingMappings?.length ?? 0,
        activeSources: [...activeSources],
      },
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
