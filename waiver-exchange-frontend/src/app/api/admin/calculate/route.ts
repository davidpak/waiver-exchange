import { NextResponse } from 'next/server';
import { verifyAdmin } from '@/lib/admin-auth';
import { supabaseAdmin } from '@/lib/supabase-server';
import {
  normalizeToPercentiles,
  calculateAllPrices,
  DEFAULT_CONFIG,
  type PricingConfig,
  type PricingInput,
  type SourcePercentiles,
} from '@/lib/pricing-engine';
import { normalizeName } from '@/lib/player-matching';

export async function POST(request: Request) {
  try {
    const { accountId } = await verifyAdmin(request);

    const body = await request.json();
    const season: number = body.season ?? new Date().getFullYear();
    const week: number = body.week ?? 0;

    // ── 1. Load active pricing config ──────────────────────────────────────
    const { data: configRow } = await supabaseAdmin
      .from('pricing_config')
      .select('*')
      .eq('is_active', true)
      .single();

    const config: PricingConfig = configRow
      ? {
          mu: Number(configRow.mu),
          sigma: Number(configRow.sigma),
          gamma: Number(configRow.gamma),
          pMaxCents: configRow.p_max_cents,
          pMinCents: configRow.p_min_cents,
          crossoverPct: Number(configRow.crossover_pct),
          crowdFloor: Number(configRow.crowd_floor),
          crowdDecay: Number(configRow.crowd_decay),
          projDecay: Number(configRow.proj_decay),
        }
      : DEFAULT_CONFIG;

    // ── 2. Load player metadata (our universe) ────────────────────────────
    const { data: players, error: playersErr } = await supabaseAdmin
      .from('player_metadata')
      .select('*')
      .not('symbol_id', 'is', null);

    if (playersErr || !players) {
      return NextResponse.json(
        { error: `Failed to load players: ${playersErr?.message}` },
        { status: 500 }
      );
    }

    const playerMap = new Map<number, string>();
    const symbolByPlayerId = new Map<string, number>();
    for (const p of players) {
      if (p.symbol_id != null) {
        playerMap.set(p.symbol_id, p.name);
        symbolByPlayerId.set(p.player_id, p.symbol_id);
      }
    }

    // ── 3. Load player_source_mapping (paginated — can exceed 1000 rows) ──
    const mappings: any[] = [];
    {
      const PAGE = 1000;
      let offset = 0;
      while (true) {
        const { data: page, error: mErr } = await supabaseAdmin
          .from('player_source_mapping')
          .select('*')
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

    // Build lookup: source → source_name → symbol_id
    const sourceNameToSymbol = new Map<string, Map<string, number>>();
    for (const m of mappings) {
      if (!sourceNameToSymbol.has(m.source)) {
        sourceNameToSymbol.set(m.source, new Map());
      }
      sourceNameToSymbol.get(m.source)!.set(m.source_name.toLowerCase(), m.symbol_id);
    }

    // ── 4. Load latest source_values for this season/week ─────────────────
    // Supabase caps queries at 1000 rows — paginate to get all
    const sourceValues: any[] = [];
    {
      const PAGE = 1000;
      let offset = 0;
      while (true) {
        const { data: page, error: svErr } = await supabaseAdmin
          .from('source_values')
          .select('*')
          .eq('season', season)
          .eq('week', week)
          .order('fetched_at', { ascending: false })
          .range(offset, offset + PAGE - 1);

        if (svErr) {
          return NextResponse.json(
            { error: `Failed to load source values: ${svErr.message}` },
            { status: 500 }
          );
        }
        if (!page || page.length === 0) break;
        sourceValues.push(...page);
        if (page.length < PAGE) break;
        offset += PAGE;
      }
    }

    // Deduplicate: keep only the latest row per (source, player_name)
    type SourceValueRow = NonNullable<typeof sourceValues>[number];
    const latestBySourcePlayer = new Map<string, SourceValueRow>();
    for (const sv of sourceValues ?? []) {
      const key = `${sv.source}::${sv.player_name.toLowerCase()}`;
      if (!latestBySourcePlayer.has(key)) {
        latestBySourcePlayer.set(key, sv);
      }
    }

    // ── 5. Resolve source_values to symbol_ids ────────────────────────────

    // Build a normalized name → symbol_id lookup for fast fuzzy fallback
    const normNameToSymbol = new Map<string, number>();
    for (const [symbolId, name] of playerMap) {
      normNameToSymbol.set(normalizeName(name), symbolId);
    }

    function resolveSymbolId(source: string, playerName: string): number | undefined {
      // 1. Try source mapping first (exact match on source_name)
      const mapping = sourceNameToSymbol.get(source);
      if (mapping) {
        const symbolId = mapping.get(playerName.toLowerCase());
        if (symbolId !== undefined) return symbolId;
      }

      // 2. Try exact name match against player_metadata
      for (const [symbolId, name] of playerMap) {
        if (name.toLowerCase() === playerName.toLowerCase()) {
          return symbolId;
        }
      }

      // 3. Fallback: normalized name match (strips suffixes, punctuation)
      const norm = normalizeName(playerName);
      const symbolId = normNameToSymbol.get(norm);
      if (symbolId !== undefined) return symbolId;

      return undefined;
    }

    // Group values by category, averaging duplicates per symbol_id
    // (e.g., a player in both FantasyCalc and KTC gets their crowd values averaged)
    const crowdAccum = new Map<number, number[]>();
    const projAccum = new Map<number, number[]>();
    const perfAccum = new Map<number, number[]>();

    let resolvedCount = 0;
    let unresolvedCount = 0;

    for (const sv of latestBySourcePlayer.values()) {
      const symbolId = resolveSymbolId(sv.source, sv.player_name);
      if (!symbolId) {
        unresolvedCount++;
        continue;
      }
      resolvedCount++;

      if (sv.source === 'fantasycalc' || sv.source === 'ktc') {
        if (!crowdAccum.has(symbolId)) crowdAccum.set(symbolId, []);
        crowdAccum.get(symbolId)!.push(sv.raw_value);
      } else if (sv.source === 'sleeper_proj') {
        if (!projAccum.has(symbolId)) projAccum.set(symbolId, []);
        projAccum.get(symbolId)!.push(sv.raw_value);
      } else if (sv.source === 'sleeper_stats') {
        if (!perfAccum.has(symbolId)) perfAccum.set(symbolId, []);
        perfAccum.get(symbolId)!.push(sv.raw_value);
      }
    }

    // Average multiple values per player per category
    function averageAccum(accum: Map<number, number[]>): { id: string; value: number }[] {
      return Array.from(accum.entries()).map(([id, values]) => ({
        id: String(id),
        value: values.reduce((a, b) => a + b, 0) / values.length,
      }));
    }

    const crowdValues = averageAccum(crowdAccum);
    const projValues = averageAccum(projAccum);
    const perfValues = averageAccum(perfAccum);

    // ── 6. Normalize to percentiles ───────────────────────────────────────
    const sources: SourcePercentiles = {
      crowd: normalizeToPercentiles(crowdValues),
      projection: normalizeToPercentiles(projValues),
      performance: normalizeToPercentiles(perfValues),
    };

    // ── 7. Calculate prices ───────────────────────────────────────────────
    const input: PricingInput = {
      players: playerMap,
      sources,
      season,
      week,
    };

    const results = calculateAllPrices(input, config);

    if (results.length === 0) {
      return NextResponse.json(
        {
          error: 'No prices calculated — check source_values and player mappings',
          debug: {
            playersInMetadata: playerMap.size,
            mappingsLoaded: mappings?.length ?? 0,
            sourceValuesLoaded: sourceValues?.length ?? 0,
            sourceValuesDeduped: latestBySourcePlayer.size,
            resolved: resolvedCount,
            unresolved: unresolvedCount,
            resolvedCrowd: crowdValues.length,
            resolvedProj: projValues.length,
            resolvedPerf: perfValues.length,
            queriedSeason: season,
            queriedWeek: week,
          },
        },
        { status: 400 }
      );
    }

    // ── 8. Write to rpe_fair_prices (the table the whole system reads) ────
    const configSnapshot = { ...config, calculatedAt: new Date().toISOString() };
    const now = new Date().toISOString();
    const BATCH_SIZE = 500;

    const rpeRows = results.map((r) => ({
      player_id: r.symbolId,
      ts: now,
      season,
      week,
      fair_cents: r.fairPriceCents,
      band_bps: 3000,
      kappa_cents_per_pt: 0,
      pacing_mode: 'admin',
      actual_pts: 0,
      delta_pts: 0,
      reason: configSnapshot as unknown as Record<string, unknown>,
      source: 'admin_pipeline',
      confidence_score: Math.round(r.confidence * 100) / 100,
    }));

    let upsertedCount = 0;
    for (let i = 0; i < rpeRows.length; i += BATCH_SIZE) {
      const batch = rpeRows.slice(i, i + BATCH_SIZE);
      const { error } = await supabaseAdmin
        .from('rpe_fair_prices')
        .upsert(batch, { onConflict: 'player_id' });
      if (error) {
        return NextResponse.json(
          { error: `rpe_fair_prices upsert failed: ${error.message}` },
          { status: 500 }
        );
      }
      upsertedCount += batch.length;
    }

    // ── 9. Log admin action ───────────────────────────────────────────────
    await supabaseAdmin.from('admin_actions').insert({
      account_id: accountId,
      action: 'calculate_prices',
      details: {
        season,
        week,
        pricesCalculated: upsertedCount,
        crowdSources: crowdValues.length,
        projSources: projValues.length,
        perfSources: perfValues.length,
        config: configSnapshot,
      },
    });

    // Count source_values per source for debug
    const svBySource: Record<string, number> = {};
    for (const sv of sourceValues) {
      svBySource[sv.source] = (svBySource[sv.source] ?? 0) + 1;
    }

    return NextResponse.json({
      count: upsertedCount,
      season,
      week,
      resolution: {
        resolved: resolvedCount,
        unresolved: unresolvedCount,
        rate: `${Math.round((resolvedCount / (resolvedCount + unresolvedCount)) * 100)}%`,
      },
      sources: {
        crowd: crowdValues.length,
        projection: projValues.length,
        performance: perfValues.length,
      },
      debug: {
        playersInMetadata: playerMap.size,
        mappingsLoaded: mappings.length,
        sourceValuesLoaded: sourceValues.length,
        sourceValuesDeduped: latestBySourcePlayer.size,
        sourceValuesBySource: svBySource,
      },
      topPrices: results.slice(0, 20).map((r) => ({
        player: r.playerName,
        priceCents: r.fairPriceCents,
        percentile: Math.round(r.compositePercentile * 10000) / 10000,
        confidence: Math.round(r.confidence * 1000) / 1000,
      })),
    });
  } catch (err) {
    if (err instanceof Response) return err;
    const message = err instanceof Error ? err.message : 'Unknown error';
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
