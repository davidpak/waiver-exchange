# OrderGateway & Public API Design

## 1. Overview

The `OrderGateway` is the external entry point that enables multiple users/clients to submit orders and receive real-time market data updates via WebSocket connections. It serves as the bridge between external clients and our core trading system, providing a public API similar to Binance's architecture.

The gateway handles authentication, order validation, rate limiting, and real-time market data broadcasting while maintaining the high-performance, in-memory order book architecture.

## 2. Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web UI Client │    │   Mobile App    │    │   Trading Bot   │
│                 │    │                 │    │                 │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                       │
          └──────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────▼─────────────┐
                    │      OrderGateway         │
                    │                           │
                    │  • WebSocket Server       │
                    │  • Authentication         │
                    │  • Rate Limiting          │
                    │  • Order Validation       │
                    │  • Market Data Broadcast  │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │    Shared System State    │
                    │                           │
                    │  • Order Books (In-Memory)│
                    │  • Market Data            │
                    │  • User Sessions          │
                    │  • Persistence Layer      │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │    Core Trading System    │
                    │                           │
                    │  • OrderRouter            │
                    │  • Whistle Engines        │
                    │  • ExecutionManager       │
                    │  • AnalyticsEngine        │
                    └───────────────────────────┘
```

## 3. Core Components

### 3.1 OrderGateway Service

**Purpose**: Handle external client connections and order submission

**Responsibilities**:
- WebSocket connection management
- API key authentication
- Order format validation
- Rate limiting per user
- Order routing to OrderRouter
- Real-time market data broadcasting

**Technology Stack**:
- Rust with `tokio-tungstenite` for WebSocket server
- JSON message format for simplicity and debugging
- `clap` for configuration management
- `serde` for message serialization

### 3.2 Shared System State

**Purpose**: Centralized state management shared between OrderGateway and Admin CLI

**Responsibilities**:
- In-memory order books for all symbols
- Market data aggregation and storage
- User session management
- Persistence layer integration
- Real-time event broadcasting

**Key Features**:
- Thread-safe access via `Arc<RwLock<T>>`
- Automatic persistence via WAL + snapshots
- Event emission for real-time updates
- Symbol-to-player mapping integration

### 3.3 Market Data Broadcaster

**Purpose**: Distribute real-time market updates to all connected clients

**Responsibilities**:
- Subscribe to ExecutionManager events
- Format market data for client consumption
- Broadcast updates to all connected WebSocket clients
- Handle client subscriptions and filtering

## 4. API Design

### 4.1 WebSocket Endpoints

```
ws://gateway:8080/orders      - Order submission and management
ws://gateway:8080/market-data - Real-time market data stream
ws://gateway:8080/user-data   - User-specific order status updates
```

### 4.2 Message Format

**Order Submission**:
```json
{
  "method": "order.place",
  "params": {
    "symbol": "SYMBOL_1",
    "side": "BUY",
    "type": "LIMIT",
    "price": 150,
    "quantity": 10,
    "account_id": "user123"
  },
  "id": "req_001"
}
```

**Order Response**:
```json
{
  "id": "req_001",
  "result": {
    "order_id": "ord_123456",
    "status": "ACCEPTED",
    "timestamp": 1640995200000
  }
}
```

**Market Data Update**:
```json
{
  "stream": "market_data",
  "data": {
    "symbol": "SYMBOL_1",
    "bids": [[150, 10], [149, 5], [148, 3]],
    "asks": [[151, 8], [152, 12], [153, 7]],
    "last_trade": {
      "price": 150,
      "quantity": 5,
      "timestamp": 1640995200000
    },
    "tick": 12345
  }
}
```

**Order Status Update**:
```json
{
  "stream": "user_data",
  "data": {
    "order_id": "ord_123456",
    "status": "FILLED",
    "filled_quantity": 10,
    "average_price": 150,
    "timestamp": 1640995200000
  }
}
```

### 4.3 Authentication

**API Key Authentication**:
```json
{
  "method": "auth.login",
  "params": {
    "api_key": "ak_1234567890abcdef",
    "api_secret": "sk_abcdef1234567890"
  }
}
```

**Authentication Response**:
```json
{
  "result": {
    "authenticated": true,
    "user_id": "user123",
    "permissions": ["trade", "market_data"],
    "rate_limits": {
      "orders_per_second": 100,
      "market_data_per_second": 1000
    }
  }
}
```

## 5. Integration with Core System

### 5.1 Shared System State

The OrderGateway integrates with the existing core trading system through a shared system state that provides:

- **In-memory order books** for all symbols
- **Market data aggregation** and storage
- **User session management**
- **Persistence layer integration** (see [Persistence Design](persistence.md))
- **Real-time event broadcasting**

### 5.2 Fantasy Football Integration

The system supports trading of NFL player shares through integration with the Sleeper API. See [Fantasy Football Integration Design](fantasy-football-integration.md) for detailed implementation.

## 6. Performance Targets

### 6.1 Latency Requirements

- **Order submission**: < 1ms (gateway to router)
- **Market data updates**: < 5ms (execution to broadcast)
- **End-to-end**: < 10ms (user order to market data update)

### 6.2 Scalability Targets

- **Concurrent connections**: 10,000+ WebSocket connections
- **Orders per second**: 100,000+ order submissions
- **Market data updates**: 1M+ updates per second
- **Symbols**: 500+ NFL players

### 6.3 Rate Limiting

```rust
pub struct RateLimits {
    pub orders_per_second: u32,      // 100 orders/second per user
    pub market_data_per_second: u32, // 1000 updates/second per user
    pub burst_limit: u32,            // 10 orders in burst
}
```

## 7. Error Handling

### 7.1 Network Errors

- **Connection drops**: Automatic reconnection with exponential backoff
- **Message parsing**: Detailed error responses with field-specific issues
- **Authentication failures**: Clear error codes and retry guidance

### 7.2 Order Errors

```json
{
  "error": {
    "code": 40001,
    "message": "Invalid order parameters",
    "details": {
      "field": "price",
      "issue": "Price must be positive integer"
    }
  }
}
```

### 7.3 System Errors

- **Rate limiting**: HTTP 429 with retry-after header
- **System overload**: Graceful degradation with queue backpressure
- **Maintenance**: Scheduled downtime notifications

## 8. Integration with Existing System

### 8.1 Admin CLI Compatibility

**Shared State**: Admin CLI and OrderGateway use the same SharedSystemState
**Real-time Sync**: Dashboard shows live data from public API
**Unified View**: Admin CLI becomes "super user" interface

### 8.2 Core System Integration

```
OrderGateway → OrderRouter → Whistle → ExecutionManager → MarketDataBroadcaster
```

**Event Flow**:
1. Order submitted via WebSocket
2. Validated and routed to OrderRouter
3. Processed by Whistle engine
4. Events emitted to ExecutionManager
5. Market data broadcasted to all clients

## 9. Implementation Plan

### 9.1 Phase 1: Core Infrastructure

1. **Create PersistenceBackend trait** - Abstract persistence interface
2. **Implement LocalPersistence** - Local file-based storage
3. **Create SharedSystemState** - Centralized state management
4. **Update Admin CLI** - Use shared state instead of local state
5. **Test persistence** - Verify orders survive restarts

### 9.2 Phase 2: OrderGateway Service

1. **WebSocket server setup** - Basic connection handling
2. **Authentication system** - API key validation
3. **Order submission flow** - Connect to OrderRouter
4. **Message serialization** - JSON message handling
5. **Error handling** - Comprehensive error responses

### 9.3 Phase 3: Market Data Broadcasting

1. **ExecutionManager integration** - Subscribe to events
2. **Market data formatter** - Convert events to client format
3. **Broadcast system** - Send updates to all clients
4. **Subscription management** - Handle client subscriptions
5. **Performance optimization** - Minimize broadcast latency

### 9.4 Phase 4: Fantasy Football Integration

1. **Sleeper API integration** - Fetch player data
2. **Symbol mapping system** - Assign symbols to players
3. **Order book initialization** - Create books for all players
4. **Player metadata storage** - Store player information
5. **Admin CLI updates** - Display player names in dashboard

### 9.5 Phase 5: Production Readiness

1. **Rate limiting implementation** - Per-user limits
2. **Monitoring and metrics** - System health tracking
3. **Cloud persistence migration** - S3 backup system
4. **Load testing** - Verify scalability targets
5. **Documentation** - API documentation and examples

## 10. Configuration

### 10.1 Gateway Configuration

```toml
[gateway]
host = "0.0.0.0"
port = 8080
max_connections = 10000
heartbeat_interval = 30

[gateway.rate_limits]
orders_per_second = 100
market_data_per_second = 1000
burst_limit = 10

[gateway.auth]
api_key_validation = true
jwt_support = false  # Future enhancement
```

### 10.2 Persistence Configuration

```toml
[persistence]
backend = "local"  # "local" or "cloud"

[persistence.local]
wal_dir = "./data/wal"
snapshot_dir = "./data/snapshots"
max_wal_files = 100
max_snapshots = 10
snapshot_interval = 1000

[persistence.cloud]
s3_bucket = "waiver-exchange-wal"
region = "us-east-1"
upload_interval = 300  # 5 minutes
retention_days = 2555  # 7 years
```

## 11. Testing Strategy

### 11.1 Unit Tests

- **Message serialization/deserialization**
- **Order validation logic**
- **Rate limiting algorithms**
- **Persistence layer operations**

### 11.2 Integration Tests

- **WebSocket connection handling**
- **Order submission flow**
- **Market data broadcasting**
- **Persistence and recovery**

### 11.3 Load Tests

- **Concurrent connection handling**
- **Order throughput testing**
- **Market data broadcast performance**
- **Memory usage under load**

### 11.4 End-to-End Tests

- **Complete trading scenarios**
- **Admin CLI and public API integration**
- **Persistence across restarts**
- **Error handling and recovery**

## 12. Monitoring and Observability

### 12.1 Metrics

- **Connection count** - Active WebSocket connections
- **Order throughput** - Orders per second
- **Latency percentiles** - P50, P95, P99 latencies
- **Error rates** - Failed orders, connection drops
- **Memory usage** - Order book memory consumption

### 12.2 Logging

- **Structured logging** - JSON format for easy parsing
- **Request tracing** - Track orders through the system
- **Error logging** - Detailed error information
- **Performance logging** - Latency and throughput metrics

### 12.3 Health Checks

- **Gateway health** - WebSocket server status
- **System state health** - Order book integrity
- **Persistence health** - WAL and snapshot status
- **External dependencies** - Sleeper API connectivity

## 13. Security Considerations

### 13.1 Authentication

- **API key validation** - Secure key storage and validation
- **Rate limiting** - Prevent abuse and DoS attacks
- **Input validation** - Sanitize all user inputs
- **Connection limits** - Prevent resource exhaustion

### 13.2 Data Protection

- **Encryption in transit** - WSS (WebSocket Secure)
- **Audit logging** - Complete order history
- **Access controls** - User-specific data isolation
- **Compliance** - 7-year data retention for audit

## 14. Future Enhancements

### 14.1 Advanced Features

- **JWT authentication** - More sophisticated auth
- **Order types** - IOC, FOK, stop orders
- **Advanced market data** - Level 2 depth, trade history
- **User management** - Account creation, permissions

### 14.2 Performance Optimizations

- **Message compression** - Reduce bandwidth usage
- **Selective subscriptions** - Symbol-specific updates
- **Caching layers** - Reduce computation overhead
- **Horizontal scaling** - Multiple gateway instances

### 14.3 Integration Features

- **REST API** - HTTP endpoints for non-real-time operations
- **Webhook support** - External system notifications
- **Third-party integrations** - Trading platform connections
- **Mobile SDK** - Native mobile app support

---

This design document provides a comprehensive blueprint for implementing the OrderGateway and public API system. The modular architecture ensures easy implementation and future extensibility while maintaining the high-performance characteristics of the core trading system.
