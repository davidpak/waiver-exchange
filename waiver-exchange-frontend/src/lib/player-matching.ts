/**
 * Player matching utility for cross-source identification.
 *
 * Handles name normalization (suffixes, apostrophes, hyphens) and
 * fuzzy matching using name + position + team signals.
 */

export interface PlayerRef {
  name: string;
  position?: string;
  team?: string;
}

export interface PlayerMetadata {
  symbolId: number;
  name: string;
  position: string;
  team: string;
}

export interface SourcePlayer {
  name: string;
  position?: string;
  team?: string;
  sourceId?: string;
  value: number;
}

export interface MatchResult {
  symbolId: number;
  ourName: string;
  sourceName: string;
  sourceId?: string;
  score: number;
}

// ─── Name Normalization ──────────────────────────────────────────────────────

const SUFFIXES = /\s+(jr\.?|sr\.?|ii|iii|iv|v)$/i;
const APOSTROPHE_VARIANTS = /[\u2018\u2019\u0060\u00B4]/g; // smart quotes, backtick, acute accent
const NON_ALPHA = /[^a-z\s'-]/g;
const MULTI_SPACE = /\s+/g;

/**
 * Normalize a player name for comparison:
 * - Lowercase
 * - Normalize apostrophe variants to ASCII '
 * - Remove Jr/Sr/II/III/IV/V suffixes
 * - Strip non-alphabetic characters (except spaces, hyphens, apostrophes)
 * - Collapse whitespace
 */
export function normalizeName(name: string): string {
  return name
    .toLowerCase()
    .replace(APOSTROPHE_VARIANTS, "'")
    .replace(SUFFIXES, '')
    .replace(NON_ALPHA, '')
    .replace(MULTI_SPACE, ' ')
    .trim();
}

// ─── Team Name Normalization ─────────────────────────────────────────────────

const TEAM_ALIASES: Record<string, string> = {
  'ari': 'ARI', 'arizona': 'ARI', 'cardinals': 'ARI',
  'atl': 'ATL', 'atlanta': 'ATL', 'falcons': 'ATL',
  'bal': 'BAL', 'baltimore': 'BAL', 'ravens': 'BAL',
  'buf': 'BUF', 'buffalo': 'BUF', 'bills': 'BUF',
  'car': 'CAR', 'carolina': 'CAR', 'panthers': 'CAR',
  'chi': 'CHI', 'chicago': 'CHI', 'bears': 'CHI',
  'cin': 'CIN', 'cincinnati': 'CIN', 'bengals': 'CIN',
  'cle': 'CLE', 'cleveland': 'CLE', 'browns': 'CLE',
  'dal': 'DAL', 'dallas': 'DAL', 'cowboys': 'DAL',
  'den': 'DEN', 'denver': 'DEN', 'broncos': 'DEN',
  'det': 'DET', 'detroit': 'DET', 'lions': 'DET',
  'gb': 'GB', 'green bay': 'GB', 'packers': 'GB', 'gnb': 'GB',
  'hou': 'HOU', 'houston': 'HOU', 'texans': 'HOU',
  'ind': 'IND', 'indianapolis': 'IND', 'colts': 'IND',
  'jax': 'JAX', 'jacksonville': 'JAX', 'jaguars': 'JAX', 'jac': 'JAX',
  'kc': 'KC', 'kansas city': 'KC', 'chiefs': 'KC',
  'lv': 'LV', 'las vegas': 'LV', 'raiders': 'LV', 'lvr': 'LV',
  'lac': 'LAC', 'los angeles chargers': 'LAC', 'chargers': 'LAC',
  'lar': 'LAR', 'los angeles rams': 'LAR', 'rams': 'LAR', 'la': 'LAR',
  'mia': 'MIA', 'miami': 'MIA', 'dolphins': 'MIA',
  'min': 'MIN', 'minnesota': 'MIN', 'vikings': 'MIN',
  'ne': 'NE', 'new england': 'NE', 'patriots': 'NE', 'nep': 'NE',
  'no': 'NO', 'new orleans': 'NO', 'saints': 'NO', 'nor': 'NO',
  'nyg': 'NYG', 'new york giants': 'NYG', 'giants': 'NYG',
  'nyj': 'NYJ', 'new york jets': 'NYJ', 'jets': 'NYJ',
  'phi': 'PHI', 'philadelphia': 'PHI', 'eagles': 'PHI',
  'pit': 'PIT', 'pittsburgh': 'PIT', 'steelers': 'PIT',
  'sf': 'SF', 'san francisco': 'SF', '49ers': 'SF', 'sfo': 'SF',
  'sea': 'SEA', 'seattle': 'SEA', 'seahawks': 'SEA',
  'tb': 'TB', 'tampa bay': 'TB', 'buccaneers': 'TB', 'bucs': 'TB', 'tam': 'TB',
  'ten': 'TEN', 'tennessee': 'TEN', 'titans': 'TEN',
  'was': 'WAS', 'washington': 'WAS', 'commanders': 'WAS', 'wsh': 'WAS',
};

function normalizeTeam(team: string | undefined): string | undefined {
  if (!team) return undefined;
  return TEAM_ALIASES[team.toLowerCase()] ?? team.toUpperCase();
}

// ─── Position Normalization ──────────────────────────────────────────────────

function normalizePosition(pos: string | undefined): string | undefined {
  if (!pos) return undefined;
  const upper = pos.toUpperCase();
  // Normalize common variants
  if (upper === 'HB' || upper === 'FB') return 'RB';
  if (upper === 'WR/TE' || upper === 'TE/WR') return 'WR'; // flex
  return upper;
}

// ─── Levenshtein Distance ────────────────────────────────────────────────────

function levenshtein(a: string, b: string): number {
  const m = a.length;
  const n = b.length;

  // Use a single flat array for the DP table
  const prev = new Array(n + 1);
  const curr = new Array(n + 1);

  for (let j = 0; j <= n; j++) prev[j] = j;

  for (let i = 1; i <= m; i++) {
    curr[0] = i;
    for (let j = 1; j <= n; j++) {
      if (a[i - 1] === b[j - 1]) {
        curr[j] = prev[j - 1];
      } else {
        curr[j] = 1 + Math.min(prev[j - 1], prev[j], curr[j - 1]);
      }
    }
    // Swap
    for (let j = 0; j <= n; j++) prev[j] = curr[j];
  }

  return prev[n];
}

// ─── Match Scoring ───────────────────────────────────────────────────────────

/**
 * Score how well two player references match (0-1).
 *
 * Scoring breakdown:
 * - Name similarity: 70% weight (based on normalized Levenshtein distance)
 * - Position match: 15% weight (exact match on normalized position)
 * - Team match: 15% weight (exact match on normalized team)
 */
export function matchScore(a: PlayerRef, b: PlayerRef): number {
  const nameA = normalizeName(a.name);
  const nameB = normalizeName(b.name);

  // Name similarity: 1 - (levenshtein / max_length)
  const maxLen = Math.max(nameA.length, nameB.length);
  const nameSim = maxLen === 0 ? 1 : 1 - levenshtein(nameA, nameB) / maxLen;

  // Position match
  const posA = normalizePosition(a.position);
  const posB = normalizePosition(b.position);
  const posMatch = posA && posB ? (posA === posB ? 1 : 0) : 0.5; // unknown = neutral

  // Team match
  const teamA = normalizeTeam(a.team);
  const teamB = normalizeTeam(b.team);
  const teamMatch = teamA && teamB ? (teamA === teamB ? 1 : 0) : 0.5; // unknown = neutral

  return 0.70 * nameSim + 0.15 * posMatch + 0.15 * teamMatch;
}

// ─── Auto-Matching ───────────────────────────────────────────────────────────

/**
 * Auto-match our players against a source's player list.
 *
 * For each of our players, finds the best match in the source list.
 * Only includes matches above the threshold (default 0.85).
 */
export function autoMatch(
  ourPlayers: PlayerMetadata[],
  sourcePlayers: SourcePlayer[],
  threshold: number = 0.85
): { matched: MatchResult[]; unmatched: PlayerMetadata[] } {
  const matched: MatchResult[] = [];
  const unmatched: PlayerMetadata[] = [];

  // Track which source players have been claimed to avoid double-matching
  const claimed = new Set<number>();

  for (const ours of ourPlayers) {
    let bestScore = 0;
    let bestIdx = -1;

    for (let i = 0; i < sourcePlayers.length; i++) {
      if (claimed.has(i)) continue;

      const score = matchScore(
        { name: ours.name, position: ours.position, team: ours.team },
        { name: sourcePlayers[i].name, position: sourcePlayers[i].position, team: sourcePlayers[i].team }
      );

      if (score > bestScore) {
        bestScore = score;
        bestIdx = i;
      }
    }

    if (bestScore >= threshold && bestIdx >= 0) {
      claimed.add(bestIdx);
      matched.push({
        symbolId: ours.symbolId,
        ourName: ours.name,
        sourceName: sourcePlayers[bestIdx].name,
        sourceId: sourcePlayers[bestIdx].sourceId,
        score: Math.round(bestScore * 1000) / 1000,
      });
    } else {
      unmatched.push(ours);
    }
  }

  return { matched, unmatched };
}
