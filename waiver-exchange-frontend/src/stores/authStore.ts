import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface User {
  id: string;
  name: string;
  email: string;
  picture?: string;
  fantasy_points?: number;
  currency_balance_dollars?: number;
}

export interface AuthState {
  // Authentication state
  isAuthenticated: boolean;
  user: User | null;
  token: string | null;
  accountId: string | null;

  // Auth provider tracking
  authProvider: 'legacy' | 'supabase' | null;

  // Sleeper integration state
  sleeperSetupComplete: boolean;
  sleeperUsername: string | null;
  sleeperLeagueId: string | null;
  sleeperRosterId: string | null;
  availableLeagues: any[] | null;

  // WebSocket connection state
  wsConnected: boolean;
  wsAuthenticated: boolean;

  // Actions
  setAuth: (user: User, token: string, accountId: string) => void;
  setAuthProvider: (provider: 'legacy' | 'supabase') => void;
  setSleeperSetup: (username: string, leagueId: string, rosterId: string) => void;
  setAvailableLeagues: (leagues: any[] | null) => void;
  setWebSocketState: (connected: boolean, authenticated: boolean) => void;
  logout: () => void;
  clearAuth: () => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, _get) => ({
      // Initial state
      isAuthenticated: false,
      user: null,
      token: null,
      accountId: null,
      authProvider: null,
      sleeperSetupComplete: false,
      sleeperUsername: null,
      sleeperLeagueId: null,
      sleeperRosterId: null,
      availableLeagues: null,
      wsConnected: false,
      wsAuthenticated: false,

      // Actions
      setAuth: (user: User, token: string, accountId: string) => {
        set({
          isAuthenticated: true,
          user,
          token,
          accountId,
        });
      },

      setAuthProvider: (provider: 'legacy' | 'supabase') => {
        set({ authProvider: provider });
      },

      setSleeperSetup: (username: string, leagueId: string, rosterId: string) => {
        set({
          sleeperSetupComplete: true,
          sleeperUsername: username,
          sleeperLeagueId: leagueId,
          sleeperRosterId: rosterId,
        });
      },

      setAvailableLeagues: (leagues: any[] | null) => {
        set({
          availableLeagues: leagues,
        });
      },

      setWebSocketState: (connected: boolean, authenticated: boolean) => {
        set({
          wsConnected: connected,
          wsAuthenticated: authenticated,
        });
      },

      logout: () => {
        // Clear localStorage
        localStorage.removeItem('waiver_exchange_token');
        localStorage.removeItem('waiver_exchange_user');

        // Reset state
        set({
          isAuthenticated: false,
          user: null,
          token: null,
          accountId: null,
          authProvider: null,
          sleeperSetupComplete: false,
          sleeperUsername: null,
          sleeperLeagueId: null,
          sleeperRosterId: null,
          availableLeagues: null,
          wsConnected: false,
          wsAuthenticated: false,
        });
      },

      clearAuth: () => {
        set({
          isAuthenticated: false,
          user: null,
          token: null,
          accountId: null,
          wsConnected: false,
          wsAuthenticated: false,
        });
      },
    }),
    {
      name: 'waiver-exchange-auth',
      partialize: (state) => ({
        isAuthenticated: state.isAuthenticated,
        user: state.user,
        token: state.token,
        accountId: state.accountId,
        authProvider: state.authProvider,
        sleeperSetupComplete: state.sleeperSetupComplete,
        sleeperUsername: state.sleeperUsername,
        sleeperLeagueId: state.sleeperLeagueId,
        sleeperRosterId: state.sleeperRosterId,
        availableLeagues: state.availableLeagues,
      }),
    }
  )
);
