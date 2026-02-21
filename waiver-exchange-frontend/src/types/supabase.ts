// Generated Supabase database types
// Matches the schema from supabase/migrations/001_combined_schema.sql + 002_supabase_auth.sql + 005_pricing_system.sql

export interface Database {
  public: {
    Tables: {
      accounts: {
        Row: {
          id: number;
          google_id: string | null;
          supabase_uid: string | null;
          sleeper_user_id: string | null;
          sleeper_roster_id: string | null;
          sleeper_league_id: string | null;
          display_name: string | null;
          fantasy_points: number | null;
          weekly_wins: number | null;
          currency_balance: number | null;
          realized_pnl: number | null;
          is_admin: boolean;
          created_at: string | null;
          last_updated: string | null;
        };
        Insert: {
          google_id?: string | null;
          supabase_uid?: string | null;
          sleeper_user_id?: string | null;
          sleeper_roster_id?: string | null;
          sleeper_league_id?: string | null;
          display_name?: string | null;
          fantasy_points?: number;
          weekly_wins?: number;
          currency_balance?: number;
          realized_pnl?: number;
          is_admin?: boolean;
        };
        Update: Partial<Database['public']['Tables']['accounts']['Insert']>;
        Relationships: [];
      };
      positions: {
        Row: {
          id: number;
          account_id: number | null;
          symbol_id: number;
          quantity: number;
          avg_cost: number;
          realized_pnl: number | null;
          last_updated: string | null;
        };
        Insert: {
          account_id?: number | null;
          symbol_id: number;
          quantity: number;
          avg_cost: number;
          realized_pnl?: number;
        };
        Update: Partial<Database['public']['Tables']['positions']['Insert']>;
        Relationships: [];
      };
      trades: {
        Row: {
          id: number;
          account_id: number | null;
          symbol_id: number;
          side: string;
          quantity: number;
          price: number;
          timestamp: string | null;
          order_id: number;
        };
        Insert: {
          account_id?: number | null;
          symbol_id: number;
          side: string;
          quantity: number;
          price: number;
          order_id: number;
        };
        Update: Partial<Database['public']['Tables']['trades']['Insert']>;
        Relationships: [];
      };
      reservations: {
        Row: {
          id: number;
          account_id: number | null;
          amount: number;
          order_id: number;
          status: string | null;
          created_at: string | null;
          expires_at: string;
        };
        Insert: {
          account_id?: number | null;
          amount: number;
          order_id: number;
          expires_at: string;
        };
        Update: Partial<Database['public']['Tables']['reservations']['Insert']>;
        Relationships: [];
      };
      player_metadata: {
        Row: {
          player_id: string;
          name: string;
          position: string;
          team: string;
          projected_points: number | null;
          rank: number | null;
          symbol_id: number | null;
          last_updated: string | null;
        };
        Insert: {
          player_id: string;
          name: string;
          position: string;
          team: string;
          projected_points?: number | null;
          rank?: number | null;
          symbol_id?: number | null;
        };
        Update: Partial<Database['public']['Tables']['player_metadata']['Insert']>;
        Relationships: [];
      };
      price_history: {
        Row: {
          symbol_id: number;
          timestamp: string;
          open_price: number;
          high_price: number;
          low_price: number;
          close_price: number;
          volume: number;
        };
        Insert: {
          symbol_id: number;
          timestamp: string;
          open_price: number;
          high_price: number;
          low_price: number;
          close_price: number;
          volume: number;
        };
        Update: Partial<Database['public']['Tables']['price_history']['Insert']>;
        Relationships: [];
      };
      daily_equity_snapshots: {
        Row: {
          id: number;
          account_id: number;
          date: string;
          total_equity: number;
          cash_balance: number;
          position_value: number;
          day_change: number;
          day_change_percent: number;
          created_at: string | null;
        };
        Insert: {
          account_id: number;
          date: string;
          total_equity: number;
          cash_balance: number;
          position_value: number;
          day_change: number;
          day_change_percent: number;
        };
        Update: Partial<Database['public']['Tables']['daily_equity_snapshots']['Insert']>;
        Relationships: [];
      };
      equity_timeseries: {
        Row: {
          id: number;
          account_id: number;
          timestamp: string;
          tick: number;
          total_equity: number;
          cash_balance: number;
          position_value: number;
          unrealized_pnl: number;
          realized_pnl: number;
          day_change: number;
          day_change_percent: number;
          created_at: string | null;
        };
        Insert: {
          account_id: number;
          timestamp: string;
          tick: number;
          total_equity: number;
          cash_balance: number;
          position_value: number;
          unrealized_pnl: number;
          realized_pnl: number;
          day_change: number;
          day_change_percent: number;
        };
        Update: Partial<Database['public']['Tables']['equity_timeseries']['Insert']>;
        Relationships: [];
      };
      rpe_fair_prices: {
        Row: {
          player_id: number;
          ts: string;
          season: number;
          week: number | null;
          fair_cents: number;
          band_bps: number;
          kappa_cents_per_pt: number;
          pacing_mode: string;
          actual_pts: number;
          delta_pts: number;
          reason: Record<string, unknown>;
          source: string | null;
          confidence_score: number | null;
        };
        Insert: {
          player_id: number;
          ts: string;
          season: number;
          fair_cents: number;
          band_bps: number;
          kappa_cents_per_pt: number;
          pacing_mode: string;
          actual_pts: number;
          delta_pts: number;
          reason: Record<string, unknown>;
        };
        Update: Partial<Database['public']['Tables']['rpe_fair_prices']['Insert']>;
        Relationships: [];
      };
      projections_season: {
        Row: {
          player_id: number;
          season: number;
          proj_points: number;
          fantasy_pos: string;
          adp: number | null;
          source: string;
          ingested_at: string;
        };
        Insert: {
          player_id: number;
          season: number;
          proj_points: number;
          fantasy_pos: string;
          adp?: number | null;
          source?: string;
        };
        Update: Partial<Database['public']['Tables']['projections_season']['Insert']>;
        Relationships: [];
      };
      player_week_points: {
        Row: {
          player_id: number;
          season: number;
          week: number;
          ts: string;
          fantasy_pts: number;
          is_game_over: boolean | null;
          raw: Record<string, unknown>;
        };
        Insert: {
          player_id: number;
          season: number;
          week: number;
          ts: string;
          fantasy_pts: number;
          raw: Record<string, unknown>;
        };
        Update: Partial<Database['public']['Tables']['player_week_points']['Insert']>;
        Relationships: [];
      };
      player_id_mapping: {
        Row: {
          sportsdataio_player_id: number;
          our_symbol_id: number;
          player_name: string;
          team: string | null;
          position: string | null;
          created_at: string;
        };
        Insert: {
          sportsdataio_player_id: number;
          our_symbol_id: number;
          player_name: string;
          team?: string | null;
          position?: string | null;
        };
        Update: Partial<Database['public']['Tables']['player_id_mapping']['Insert']>;
        Relationships: [];
      };
      market_state: {
        Row: {
          symbol_id: number;
          state: string;
          reason: string | null;
          last_updated: string;
        };
        Insert: {
          symbol_id: number;
          state: string;
          reason?: string | null;
        };
        Update: Partial<Database['public']['Tables']['market_state']['Insert']>;
        Relationships: [];
      };
      house_fills: {
        Row: {
          id: number;
          order_id: number;
          account_id: number;
          player_id: number;
          ts: string;
          price_cents: number;
          qty_bp: number;
          ft_at_fill: number;
          rule_json: Record<string, unknown>;
        };
        Insert: {
          order_id: number;
          account_id: number;
          player_id: number;
          ts: string;
          price_cents: number;
          qty_bp: number;
          ft_at_fill: number;
          rule_json: Record<string, unknown>;
        };
        Update: Partial<Database['public']['Tables']['house_fills']['Insert']>;
        Relationships: [];
      };
      house_accounts: {
        Row: {
          id: number;
          account_type: string;
          display_name: string;
          currency_balance: number | null;
          is_active: boolean | null;
          created_at: string;
        };
        Insert: {
          account_type: string;
          display_name: string;
          currency_balance?: number;
          is_active?: boolean;
        };
        Update: Partial<Database['public']['Tables']['house_accounts']['Insert']>;
        Relationships: [];
      };
      source_values: {
        Row: {
          id: number;
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
        };
        Insert: {
          source: string;
          season: number;
          week?: number;
          player_name: string;
          position?: string | null;
          team?: string | null;
          raw_value: number;
          source_player_id?: string | null;
          meta?: Record<string, unknown>;
        };
        Update: Partial<Database['public']['Tables']['source_values']['Insert']>;
        Relationships: [];
      };
      fair_prices: {
        Row: {
          symbol_id: number;
          season: number;
          week: number;
          fair_price_cents: number;
          composite_percentile: number;
          crowd_percentile: number | null;
          projection_percentile: number | null;
          performance_percentile: number | null;
          confidence: number;
          config_snapshot: Record<string, unknown>;
          calculated_at: string;
        };
        Insert: {
          symbol_id: number;
          season: number;
          week?: number;
          fair_price_cents: number;
          composite_percentile: number;
          crowd_percentile?: number | null;
          projection_percentile?: number | null;
          performance_percentile?: number | null;
          confidence: number;
          config_snapshot?: Record<string, unknown>;
        };
        Update: Partial<Database['public']['Tables']['fair_prices']['Insert']>;
        Relationships: [];
      };
      pricing_config: {
        Row: {
          id: number;
          label: string;
          mu: number;
          sigma: number;
          gamma: number;
          p_max_cents: number;
          p_min_cents: number;
          crossover_pct: number;
          crowd_floor: number;
          crowd_decay: number;
          proj_decay: number;
          is_active: boolean;
          created_at: string;
          updated_at: string;
        };
        Insert: {
          label: string;
          mu?: number;
          sigma?: number;
          gamma?: number;
          p_max_cents?: number;
          p_min_cents?: number;
          crossover_pct?: number;
          crowd_floor?: number;
          crowd_decay?: number;
          proj_decay?: number;
          is_active?: boolean;
        };
        Update: Partial<Database['public']['Tables']['pricing_config']['Insert']>;
        Relationships: [];
      };
      player_source_mapping: {
        Row: {
          id: number;
          symbol_id: number;
          source: string;
          source_player_id: string | null;
          source_name: string;
          match_score: number | null;
          verified: boolean;
          created_at: string;
        };
        Insert: {
          symbol_id: number;
          source: string;
          source_player_id?: string | null;
          source_name: string;
          match_score?: number | null;
          verified?: boolean;
        };
        Update: Partial<Database['public']['Tables']['player_source_mapping']['Insert']>;
        Relationships: [];
      };
      admin_actions: {
        Row: {
          id: number;
          account_id: number;
          action: string;
          details: Record<string, unknown>;
          created_at: string;
        };
        Insert: {
          account_id: number;
          action: string;
          details?: Record<string, unknown>;
        };
        Update: Partial<Database['public']['Tables']['admin_actions']['Insert']>;
        Relationships: [];
      };
    };
    Views: Record<string, never>;
    Functions: {
      get_my_account_id: {
        Args: Record<string, never>;
        Returns: number;
      };
    };
  };
}
