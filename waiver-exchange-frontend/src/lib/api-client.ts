// API Client for Waiver Exchange Trading Platform
import type {
  AccountSummaryResponse,
  AuthResponse,
  CurrentPriceResponse,
  EquityHistoryResponse,
  OrderPlaceRequest,
  OrderPlaceResponse,
  PriceHistoryResponse,
  SnapshotResponse,
  SymbolInfoResponse,
  Timeframe,
  WebSocketMessage
} from '@/types/api';
import { API_CONFIG } from '@/types/api';

// ============================================================================
// REST API Client
// ============================================================================

class RestApiClient {
  private baseUrl: string;

  constructor(baseUrl: string = API_CONFIG.REST_BASE_URL) {
    this.baseUrl = baseUrl;
  }

  private async request<T>(endpoint: string, options?: RequestInit): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`;
    
    const response = await fetch(url, {
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
      ...options,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({
        error: {
          code: 'NETWORK_ERROR',
          message: `HTTP ${response.status}: ${response.statusText}`,
        },
        timestamp: new Date().toISOString(),
      }));
      throw new Error(error.error?.message || 'Request failed');
    }

    return response.json();
  }

  // Symbol Information
  async getSymbolInfo(symbolId: number): Promise<SymbolInfoResponse> {
    return this.request<SymbolInfoResponse>(`/symbol/${symbolId}/info`);
  }

  // Get all players for search functionality
  async getAllPlayers(): Promise<SymbolInfoResponse[]> {
    return this.request<SymbolInfoResponse[]>('/symbols/all');
  }

  // Get current price for a symbol
  async getCurrentPrice(symbolId: number): Promise<CurrentPriceResponse> {
    return this.request<CurrentPriceResponse>(`/symbol/${symbolId}/price`);
  }

  // Price History
  async getPriceHistory(
    symbolId: number,
    period: Timeframe = '1d',
    interval: string = '5m'
  ): Promise<PriceHistoryResponse> {
    return this.request<PriceHistoryResponse>(
      `/price-history/${symbolId}?period=${period}&interval=${interval}`
    );
  }

  // Account Data
  async getAccountSummary(accountId: number = 1): Promise<AccountSummaryResponse> {
    return this.request<AccountSummaryResponse>(`/account/summary?account_id=${accountId}`);
  }

  async getEquityHistory(
    accountId: number = 1,
    startDate?: string,
    endDate?: string
  ): Promise<EquityHistoryResponse> {
    const params = new URLSearchParams({ account_id: accountId.toString() });
    if (startDate) params.append('start_date', startDate);
    if (endDate) params.append('end_date', endDate);
    
    return this.request<EquityHistoryResponse>(`/account/equity-history?${params}`);
  }

  // Market Data
  async getCurrentSnapshot(): Promise<SnapshotResponse> {
    return this.request<SnapshotResponse>('/snapshot/current');
  }
}

// ============================================================================
// WebSocket API Client
// ============================================================================

class WebSocketClient {
  private ws: WebSocket | null = null;
  private url: string;
  private messageId = 0;
  private pendingMessages = new Map<string, {
    resolve: (value: any) => void;
    reject: (error: Error) => void;
  }>();

  constructor(url: string = API_CONFIG.WS_BASE_URL) {
    this.url = url;
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.url);
        
        this.ws.onopen = () => {
          console.log('WebSocket connected');
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const message: WebSocketMessage = JSON.parse(event.data);
            this.handleMessage(message);
          } catch (error) {
            console.error('Failed to parse WebSocket message:', error);
          }
        };

        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error);
          reject(new Error('WebSocket connection failed'));
        };

        this.ws.onclose = () => {
          console.log('WebSocket disconnected');
          // Reject all pending messages
          this.pendingMessages.forEach(({ reject }) => {
            reject(new Error('WebSocket connection closed'));
          });
          this.pendingMessages.clear();
        };
      } catch (error) {
        reject(error);
      }
    });
  }

  private handleMessage(message: WebSocketMessage) {
    if (message.id && this.pendingMessages.has(message.id)) {
      const { resolve, reject } = this.pendingMessages.get(message.id)!;
      this.pendingMessages.delete(message.id);

      if (message.error) {
        reject(new Error(message.error.message));
      } else {
        resolve(message.result || message.data);
      }
    }
  }

  private sendMessage<T>(method: string, params?: any): Promise<T> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error('WebSocket not connected'));
    }

    const id = (++this.messageId).toString();
    const message: WebSocketMessage = {
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      this.pendingMessages.set(id, { resolve, reject });
      
      try {
        this.ws!.send(JSON.stringify(message));
      } catch (error) {
        this.pendingMessages.delete(id);
        reject(error);
      }
    });
  }

  // Authentication
  async authenticate(apiKey: string, apiSecret: string): Promise<AuthResponse> {
    return this.sendMessage<AuthResponse>('auth', {
      api_key: apiKey,
      api_secret: apiSecret,
    });
  }

  // Order Management
  async placeOrder(order: OrderPlaceRequest): Promise<OrderPlaceResponse> {
    return this.sendMessage<OrderPlaceResponse>('order.place', order);
  }

  // Account Data
  async getAccountInfo(): Promise<any> {
    return this.sendMessage('account.info', {});
  }

  async getAccountPositions(): Promise<any> {
    return this.sendMessage('account.positions', {});
  }

  async getAccountTrades(): Promise<any> {
    return this.sendMessage('account.trades', {});
  }

  // Market Data Subscription
  async subscribeToMarketData(): Promise<any> {
    return this.sendMessage('market_data.subscribe', {});
  }

  disconnect() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}

// ============================================================================
// Combined API Client
// ============================================================================

export class ApiClient {
  public rest: RestApiClient;
  public ws: WebSocketClient;

  constructor() {
    this.rest = new RestApiClient();
    this.ws = new WebSocketClient();
  }

  async connectWebSocket(): Promise<void> {
    await this.ws.connect();
  }

  disconnectWebSocket() {
    this.ws.disconnect();
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

export const apiClient = new ApiClient();
