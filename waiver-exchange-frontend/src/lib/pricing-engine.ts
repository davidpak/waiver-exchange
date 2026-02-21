/**
 * Multi-source pricing engine.
 *
 * Pure TypeScript — no server-only imports, works in browser AND Node.
 * Combines crowd-sourced trade values, analytical projections, and performance
 * data using inverse-variance weighting and a hybrid log-normal + concave elite curve.
 *
 * Price range: $0.01 – $200.00 (1 – 20,000 cents)
 */

// ─── Types ───────────────────────────────────────────────────────────────────

export interface PricingConfig {
  mu: number;           // 7.6 – log-normal center
  sigma: number;        // 1.0 – log-normal spread
  gamma: number;        // 0.65 – elite clustering exponent
  pMaxCents: number;    // 20000 – price ceiling ($200)
  pMinCents: number;    // 1 – price floor ($0.01)
  crossoverPct: number; // 0.90 – where elite curve begins
  crowdFloor: number;   // 0.50 – minimum crowd weight
  crowdDecay: number;   // 0.20 – crowd weight decay rate
  projDecay: number;    // 0.25 – projection weight decay rate
}

export interface WeightSet {
  crowd: number;
  proj: number;
  perf: number;
}

export interface SourcePercentiles {
  crowd: Map<string, number>;       // averaged FantasyCalc + KTC
  projection: Map<string, number>;  // averaged Sleeper + NFL
  performance: Map<string, number>; // actual stats pace
}

export interface FairPriceResult {
  symbolId: number;
  playerName: string;
  fairPriceCents: number;
  compositePercentile: number;
  crowdPercentile: number;
  projectionPercentile: number;
  performancePercentile: number;
  confidence: number;  // 0-1, based on source agreement
}

export interface PricingInput {
  /** symbol_id → player name */
  players: Map<number, string>;
  sources: SourcePercentiles;
  season: number;
  week: number;
}

// ─── Default Config ──────────────────────────────────────────────────────────

export const DEFAULT_CONFIG: PricingConfig = {
  mu: 7.6,
  sigma: 1.0,
  gamma: 0.65,
  pMaxCents: 20000,
  pMinCents: 1,
  crossoverPct: 0.95,
  crowdFloor: 0.50,
  crowdDecay: 0.20,
  projDecay: 0.25,
};

// ─── Percentile Normalization ────────────────────────────────────────────────

/**
 * Convert raw values to rank-based percentiles (0, 1).
 * Players with identical values share the average of their ranks.
 */
export function normalizeToPercentiles(
  values: { id: string; value: number }[]
): Map<string, number> {
  if (values.length === 0) return new Map();
  if (values.length === 1) return new Map([[values[0].id, 0.5]]);

  // Sort ascending by value
  const sorted = [...values].sort((a, b) => a.value - b.value);
  const n = sorted.length;

  // Assign ranks (1-based), averaging ties
  const ranks = new Map<string, number>();
  let i = 0;
  while (i < n) {
    let j = i;
    // Find all items with the same value
    while (j < n && sorted[j].value === sorted[i].value) {
      j++;
    }
    // Average rank for the tie group
    const avgRank = (i + j + 1) / 2; // +1 because 1-based
    for (let k = i; k < j; k++) {
      ranks.set(sorted[k].id, avgRank);
    }
    i = j;
  }

  // Convert ranks to percentiles: (rank - 0.5) / n
  // This maps to (0, 1) — never exactly 0 or 1
  const percentiles = new Map<string, number>();
  for (const [id, rank] of ranks) {
    percentiles.set(id, (rank - 0.5) / n);
  }

  return percentiles;
}

// ─── Weight Calculation ──────────────────────────────────────────────────────

/**
 * Calculate dynamic weights based on the week within the season.
 *
 * Early season: crowd-heavy (trade values are most reliable pre-season).
 * Late season: performance data gets more weight as actuals accumulate.
 *
 * Weights always sum to 1.
 */
export function calculateWeights(week: number, config: PricingConfig): WeightSet {
  // crowd weight decays from 1.0 toward crowdFloor as week increases
  const crowdRaw = Math.max(config.crowdFloor, 1.0 - config.crowdDecay * week);

  // projection weight decays toward 0 as season progresses
  const projRaw = Math.max(0.05, 1.0 - config.projDecay * week);

  // performance weight grows as data accumulates (0 in week 0)
  const perfRaw = week > 0 ? Math.min(1.0, 0.1 * week) : 0;

  // Normalize to sum to 1
  const total = crowdRaw + projRaw + perfRaw;
  return {
    crowd: crowdRaw / total,
    proj: projRaw / total,
    perf: perfRaw / total,
  };
}

// ─── Percentile Fusion ───────────────────────────────────────────────────────

/**
 * Fuse percentiles from multiple source groups into a single composite percentile
 * per player using weighted averaging.
 *
 * Only includes sources where the player has data.
 * Re-normalizes weights per player based on available sources.
 */
export function fusePercentiles(
  sources: SourcePercentiles,
  weights: WeightSet
): Map<string, { composite: number; crowd: number; proj: number; perf: number; sourceCount: number }> {
  // Collect all player IDs across all sources
  const allIds = new Set<string>();
  for (const id of sources.crowd.keys()) allIds.add(id);
  for (const id of sources.projection.keys()) allIds.add(id);
  for (const id of sources.performance.keys()) allIds.add(id);

  const results = new Map<string, { composite: number; crowd: number; proj: number; perf: number; sourceCount: number }>();

  for (const id of allIds) {
    const crowdPct = sources.crowd.get(id);
    const projPct = sources.projection.get(id);
    const perfPct = sources.performance.get(id);

    let weightSum = 0;
    let valueSum = 0;
    let sourceCount = 0;

    if (crowdPct !== undefined) {
      weightSum += weights.crowd;
      valueSum += weights.crowd * crowdPct;
      sourceCount++;
    }
    if (projPct !== undefined) {
      weightSum += weights.proj;
      valueSum += weights.proj * projPct;
      sourceCount++;
    }
    if (perfPct !== undefined) {
      weightSum += weights.perf;
      valueSum += weights.perf * perfPct;
      sourceCount++;
    }

    if (weightSum > 0) {
      results.set(id, {
        composite: valueSum / weightSum,
        crowd: crowdPct ?? -1,
        proj: projPct ?? -1,
        perf: perfPct ?? -1,
        sourceCount,
      });
    }
  }

  return results;
}

// ─── Probit (Inverse Normal CDF) ────────────────────────────────────────────

/**
 * Abramowitz & Stegun rational approximation for the inverse normal CDF.
 * Accurate to ~4.5e-4. Avoids external math library dependency.
 *
 * Input p must be in (0, 1).
 */
export function probit(p: number): number {
  if (p <= 0 || p >= 1) {
    throw new Error(`probit: p must be in (0, 1), got ${p}`);
  }

  // Coefficients for the rational approximation
  const a1 = -3.969683028665376e1;
  const a2 = 2.209460984245205e2;
  const a3 = -2.759285104469687e2;
  const a4 = 1.383577518672690e2;
  const a5 = -3.066479806614716e1;
  const a6 = 2.506628277459239e0;

  const b1 = -5.447609879822406e1;
  const b2 = 1.615858368580409e2;
  const b3 = -1.556989798598866e2;
  const b4 = 6.680131188771972e1;
  const b5 = -1.328068155288572e1;

  const c1 = -7.784894002430293e-3;
  const c2 = -3.223964580411365e-1;
  const c3 = -2.400758277161838e0;
  const c4 = -2.549732539343734e0;
  const c5 = 4.374664141464968e0;
  const c6 = 2.938163982698783e0;

  const d1 = 7.784695709041462e-3;
  const d2 = 3.224671290700398e-1;
  const d3 = 2.445134137142996e0;
  const d4 = 3.754408661907416e0;

  const pLow = 0.02425;
  const pHigh = 1 - pLow;

  let q: number, r: number, x: number;

  if (p < pLow) {
    // Rational approximation for lower region
    q = Math.sqrt(-2 * Math.log(p));
    x =
      (((((c1 * q + c2) * q + c3) * q + c4) * q + c5) * q + c6) /
      ((((d1 * q + d2) * q + d3) * q + d4) * q + 1);
  } else if (p <= pHigh) {
    // Rational approximation for central region
    q = p - 0.5;
    r = q * q;
    x =
      ((((((a1 * r + a2) * r + a3) * r + a4) * r + a5) * r + a6) * q) /
      (((((b1 * r + b2) * r + b3) * r + b4) * r + b5) * r + 1);
  } else {
    // Rational approximation for upper region
    q = Math.sqrt(-2 * Math.log(1 - p));
    x =
      -(((((c1 * q + c2) * q + c3) * q + c4) * q + c5) * q + c6) /
      ((((d1 * q + d2) * q + d3) * q + d4) * q + 1);
  }

  return x;
}

// ─── Percentile → Price ──────────────────────────────────────────────────────

/**
 * Map a composite percentile to a price in cents using a hybrid curve:
 * - Below crossoverPct: log-normal CDF (bulk of players)
 * - Above crossoverPct: concave power curve (elite clustering)
 *
 * This prevents the elite tier from being too spread out (log-normal tails)
 * while keeping natural separation in the middle tiers.
 */
export function percentileToPrice(pct: number, config: PricingConfig): number {
  const { mu, sigma, gamma, pMaxCents, pMinCents, crossoverPct } = config;

  // Clamp percentile to a safe range for probit
  const safePct = Math.max(0.001, Math.min(0.999, pct));

  if (safePct <= crossoverPct) {
    // Log-normal region: price = exp(mu + sigma * probit(pct))
    const z = probit(safePct);
    const rawPrice = Math.exp(mu + sigma * z);
    // Scale to fit within [pMinCents, crossover price]
    // crossover price = what the log-normal would give at crossoverPct
    const crossoverZ = probit(crossoverPct);
    const crossoverRaw = Math.exp(mu + sigma * crossoverZ);
    const minRaw = Math.exp(mu + sigma * probit(0.001));

    // Linear map from [minRaw, crossoverRaw] → [pMinCents, crossoverPrice]
    const crossoverPrice = pMaxCents * Math.pow(crossoverPct, gamma);
    const t = (rawPrice - minRaw) / (crossoverRaw - minRaw);
    return Math.max(pMinCents, Math.round(pMinCents + t * (crossoverPrice - pMinCents)));
  } else {
    // Elite concave region: price = pMax * pct^gamma
    // This gives diminishing returns — top players cluster together
    const price = pMaxCents * Math.pow(safePct, gamma);
    return Math.min(pMaxCents, Math.round(price));
  }
}

// ─── Confidence Score ────────────────────────────────────────────────────────

/**
 * Calculate confidence based on:
 * 1. Number of sources available (more sources = higher base confidence)
 * 2. Agreement between sources (lower variance = higher confidence)
 */
function calculateConfidence(
  percentiles: { crowd: number; proj: number; perf: number },
  sourceCount: number
): number {
  // Base confidence from source count: 1 source = 0.3, 2 = 0.6, 3 = 0.9
  const baseCoverage = Math.min(0.9, sourceCount * 0.3);

  // Collect available percentiles (skip -1 = missing)
  const available: number[] = [];
  if (percentiles.crowd >= 0) available.push(percentiles.crowd);
  if (percentiles.proj >= 0) available.push(percentiles.proj);
  if (percentiles.perf >= 0) available.push(percentiles.perf);

  if (available.length < 2) return baseCoverage;

  // Calculate variance of available percentiles
  const mean = available.reduce((s, v) => s + v, 0) / available.length;
  const variance = available.reduce((s, v) => s + (v - mean) ** 2, 0) / available.length;

  // Agreement bonus: low variance → up to +0.1
  // Max possible variance for percentiles in [0,1] is 0.25 (one at 0, one at 1)
  const agreementBonus = 0.1 * (1 - Math.min(1, variance / 0.25));

  return Math.min(1.0, baseCoverage + agreementBonus);
}

// ─── Full Pipeline ───────────────────────────────────────────────────────────

/**
 * Calculate fair prices for all players.
 *
 * Steps:
 * 1. Calculate dynamic weights for the given week
 * 2. Fuse percentiles across sources
 * 3. Map composite percentiles to prices via hybrid curve
 * 4. Calculate confidence scores
 */
export function calculateAllPrices(
  data: PricingInput,
  config: PricingConfig
): FairPriceResult[] {
  const weights = calculateWeights(data.week, config);
  const fused = fusePercentiles(data.sources, weights);

  const results: FairPriceResult[] = [];

  for (const [symbolId, playerName] of data.players) {
    const key = String(symbolId);
    const fusedData = fused.get(key);

    if (!fusedData) continue;

    const fairPriceCents = percentileToPrice(fusedData.composite, config);

    results.push({
      symbolId,
      playerName,
      fairPriceCents,
      compositePercentile: fusedData.composite,
      crowdPercentile: fusedData.crowd >= 0 ? fusedData.crowd : 0,
      projectionPercentile: fusedData.proj >= 0 ? fusedData.proj : 0,
      performancePercentile: fusedData.perf >= 0 ? fusedData.perf : 0,
      confidence: calculateConfidence(
        { crowd: fusedData.crowd, proj: fusedData.proj, perf: fusedData.perf },
        fusedData.sourceCount
      ),
    });
  }

  // Sort by price descending
  results.sort((a, b) => b.fairPriceCents - a.fairPriceCents);

  return results;
}
