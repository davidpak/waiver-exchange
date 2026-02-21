import Dexie, { type EntityTable } from 'dexie';

// ---------------------------------------------------------------------------
// Table interfaces
// ---------------------------------------------------------------------------

export interface CachedPlayer {
  player_id: string;
  name: string;
  position: string;
  team: string;
  projected_points: number | null;
  rank: number | null;
  symbol_id: number | null;
  last_updated: string | null;
}

export interface CachedTrade {
  id: number;
  account_id: number;
  symbol_id: number;
  side: string;
  quantity: number;
  price: number;
  timestamp: string;
  order_id: number;
}

export interface CachedSetting {
  key: string;
  value: string;
}

// ---------------------------------------------------------------------------
// Database definition
// ---------------------------------------------------------------------------

class WaiverExchangeDB extends Dexie {
  players!: EntityTable<CachedPlayer, 'player_id'>;
  trades!: EntityTable<CachedTrade, 'id'>;
  settings!: EntityTable<CachedSetting, 'key'>;

  constructor() {
    super('waiver-exchange');

    this.version(1).stores({
      players: 'player_id, symbol_id, name, team, position, rank',
      trades: 'id, account_id, symbol_id, timestamp',
      settings: 'key',
    });
  }
}

export const db = new WaiverExchangeDB();

// ---------------------------------------------------------------------------
// Player cache helpers
// ---------------------------------------------------------------------------

/** Bulk-upsert players into IndexedDB */
export async function cachePlayersToIDB(players: CachedPlayer[]): Promise<void> {
  await db.players.bulkPut(players);
}

/** Read all players from IndexedDB, ordered by rank */
export async function readPlayersFromIDB(): Promise<CachedPlayer[]> {
  return db.players.orderBy('rank').toArray();
}

/** Check if player cache is populated */
export async function hasPlayerCache(): Promise<boolean> {
  return (await db.players.count()) > 0;
}

// ---------------------------------------------------------------------------
// Trade cache helpers
// ---------------------------------------------------------------------------

/** Bulk-upsert trades into IndexedDB */
export async function cacheTradesToIDB(trades: CachedTrade[]): Promise<void> {
  await db.trades.bulkPut(trades);
}

/** Read trades for an account, newest first */
export async function readTradesFromIDB(accountId: number, limit = 100): Promise<CachedTrade[]> {
  return db.trades
    .where('account_id')
    .equals(accountId)
    .reverse()
    .sortBy('timestamp')
    .then((trades) => trades.slice(0, limit));
}

/** Get the newest trade timestamp for incremental fetch */
export async function getLatestTradeTimestamp(accountId: number): Promise<string | null> {
  const latest = await db.trades
    .where('account_id')
    .equals(accountId)
    .reverse()
    .sortBy('timestamp')
    .then((trades) => trades[0]);
  return latest?.timestamp ?? null;
}

/** Clean up trade history older than N days */
export async function cleanupOldTrades(maxAgeDays = 90): Promise<number> {
  const cutoff = new Date(Date.now() - maxAgeDays * 24 * 60 * 60 * 1000).toISOString();
  return db.trades.where('timestamp').below(cutoff).delete();
}

// ---------------------------------------------------------------------------
// Settings helpers (widget preferences, timeframes, favorites)
// ---------------------------------------------------------------------------

export async function getSetting(key: string): Promise<string | undefined> {
  const row = await db.settings.get(key);
  return row?.value;
}

export async function setSetting(key: string, value: string): Promise<void> {
  await db.settings.put({ key, value });
}

export async function deleteSetting(key: string): Promise<void> {
  await db.settings.delete(key);
}
