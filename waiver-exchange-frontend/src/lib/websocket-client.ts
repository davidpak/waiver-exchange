import { useAuthStore } from '@/stores/authStore';

export interface WebSocketMessage {
  id?: string;
  method: string;
  params: Record<string, any>;
  result?: any;
  error?: any;
}

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private messageHandlers: Map<string, (message: WebSocketMessage) => void> = new Map();

  constructor(
    private url: string = 'ws://localhost:8081',
    private onConnect?: () => void,
    private onDisconnect?: () => void,
    private onError?: (error: Event) => void
  ) {}

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
          console.log('WebSocket connected');
          this.reconnectAttempts = 0;
          this.onConnect?.();
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const message: WebSocketMessage = JSON.parse(event.data);
            console.log('WebSocket message received:', message);
            
            // Handle authentication responses
            if (message.id === 'auth_jwt_001' && message.result) {
              if (message.result.authenticated) {
                const authStore = useAuthStore.getState();
                authStore.setWebSocketState(true, true);
                console.log('WebSocket authentication successful');
              } else {
                console.error('WebSocket authentication failed:', message.result.error);
                const authStore = useAuthStore.getState();
                authStore.setWebSocketState(true, false);
              }
            }

            // Call registered message handlers
            if (message.id && this.messageHandlers.has(message.id)) {
              const handler = this.messageHandlers.get(message.id);
              handler?.(message);
              this.messageHandlers.delete(message.id);
            }
          } catch (error) {
            console.error('Error parsing WebSocket message:', error);
          }
        };

        this.ws.onclose = () => {
          console.log('WebSocket disconnected');
          this.onDisconnect?.();
          this.attemptReconnect();
        };

        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error);
          this.onError?.(error);
          reject(error);
        };
      } catch (error) {
        reject(error);
      }
    });
  }

  private attemptReconnect(): void {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      console.log(`Attempting to reconnect (${this.reconnectAttempts}/${this.maxReconnectAttempts})...`);
      
      setTimeout(() => {
        this.connect().catch(console.error);
      }, this.reconnectDelay * this.reconnectAttempts);
    } else {
      console.error('Max reconnection attempts reached');
    }
  }

  authenticateWithJWT(token: string): Promise<boolean> {
    return new Promise((resolve) => {
      const authMessage: WebSocketMessage = {
        id: 'auth_jwt_001',
        method: 'auth.jwt',
        params: { token }
      };

      // Register handler for authentication response
      this.messageHandlers.set('auth_jwt_001', (message) => {
        if (message.result?.authenticated) {
          const authStore = useAuthStore.getState();
          authStore.setWebSocketState(true, true);
          resolve(true);
        } else {
          resolve(false);
        }
      });

      this.send(authMessage);
    });
  }

  send(message: WebSocketMessage): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    } else {
      console.error('WebSocket not connected');
    }
  }

  sendWithResponse(message: WebSocketMessage, timeout: number = 5000): Promise<WebSocketMessage> {
    return new Promise((resolve, reject) => {
      if (!message.id) {
        message.id = `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      }

      // Set up timeout
      const timeoutId = setTimeout(() => {
        this.messageHandlers.delete(message.id!);
        reject(new Error('WebSocket message timeout'));
      }, timeout);

      // Register response handler
      this.messageHandlers.set(message.id, (response) => {
        clearTimeout(timeoutId);
        resolve(response);
      });

      this.send(message);
    });
  }

  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  // Sleeper Integration Methods
  async checkSleeperIntegration(): Promise<WebSocketMessage> {
    return this.sendWithResponse({
      method: 'account.info',
      params: {}
    });
  }

  async setupSleeperIntegration(sleeperUsername: string): Promise<WebSocketMessage> {
    return this.sendWithResponse({
      method: 'account.setup_sleeper',
      params: {
        sleeper_username: sleeperUsername
      }
    });
  }

  async selectLeague(leagueId: string, rosterId: string): Promise<WebSocketMessage> {
    return this.sendWithResponse({
      method: 'account.select_league',
      params: {
        league_id: leagueId,
        roster_id: rosterId
      }
    });
  }
}

// Singleton instance
let wsClient: WebSocketClient | null = null;

export const getWebSocketClient = (): WebSocketClient => {
  if (!wsClient) {
    wsClient = new WebSocketClient();
  }
  return wsClient;
};
