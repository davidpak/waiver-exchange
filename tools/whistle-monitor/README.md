# Whistle Monitor

A real-time monitoring dashboard for the Whistle exchange with session-based trading and beautiful terminal UI.

## 🚀 **Overview**

The Whistle Monitor provides a comprehensive real-time trading environment where you can:
- **Run session engines** that process orders from multiple accounts
- **View beautiful real-time dashboards** with live order book updates
- **Monitor trades** with proper color coding (green for buys, red for sells)
- **Track order book changes** as orders are placed and executed
- **Manage multiple trading sessions** simultaneously

## 🎯 **Key Features**

- **Session Engine**: Drive Whistle engines with automatic tick processing
- **Real-time Dashboard**: Beautiful terminal UI with live updates
- **Multi-Account Support**: Process orders from multiple accounts in sessions
- **File-based Communication**: Seamless integration with Whistle Playground
- **Smart Display Updates**: Only updates when there are actual changes
- **Last Trade Tracking**: Real-time trade monitoring with color coding

## 📋 **Quick Start**

### 1. Create a Session (using Playground)
```bash
# In one terminal, create a session
cargo run --bin whistle-playground -- create-session my-trading --accounts 5
```

### 2. Start the Monitor
```bash
# In another terminal, start the session engine
cargo run --bin whistle-monitor -- start-session my-trading --display dashboard
```

### 3. Submit Orders (using Playground)
```bash
# In a third terminal, submit orders
cargo run --bin whistle-playground -- submit my-trading --account-id 1 --side buy --order-type limit --price 150 --qty 10
```

### 4. Watch Real-time Updates
The dashboard will automatically update showing:
- Live order book changes
- Trade executions with color coding
- Session statistics

## 🛠️ **Commands Reference**

### Session Management

#### Start Session Engine
```bash
cargo run --bin whistle-monitor -- start-session <SESSION_NAME> --tick-interval-ms <MS> --display <MODE>
```

**Parameters:**
- `SESSION_NAME`: Name of the session to start
- `--tick-interval-ms`: Tick interval in milliseconds (default: 2000)
- `--display`: Display mode - `dashboard`, `minimal`, or `basic`

**Examples:**
```bash
# Start with beautiful dashboard
cargo run --bin whistle-monitor -- start-session my-trading --display dashboard

# Start with minimal output
cargo run --bin whistle-monitor -- start-session my-trading --display minimal

# Custom tick interval
cargo run --bin whistle-monitor -- start-session my-trading --tick-interval-ms 1000 --display dashboard
```

#### List Sessions
```bash
cargo run --bin whistle-monitor -- list-sessions
```
Shows all available trading sessions.

**Output:**
```
📋 Available Sessions
  📁 test-trading
  📁 my-trading
  📁 demo-session
```

#### Session Info
```bash
cargo run --bin whistle-monitor -- session-info <SESSION_NAME>
```
Displays detailed information about a session.

**Output:**
```
📊 Session Info: test-trading
  📁 Directory: C:\Users\...\whistle-exchange\test-trading
  👥 Accounts: 5
  🕐 Created: 1756574426
  🔄 Last Activity: 1756574426

📄 Session Files:
  📄 orders.jsonl (0 bytes)
  📄 responses.jsonl (0 bytes)
  📄 trades.jsonl (0 bytes)
  📄 book_updates.jsonl (0 bytes)
```

#### Cleanup Sessions
```bash
cargo run --bin whistle-monitor -- cleanup-sessions
```
Removes expired sessions and cleans up temporary files.

## 📊 **Dashboard Display Modes**

### Dashboard Mode (Recommended)
The most beautiful and informative display mode:

```
🎯 WHISTLE SESSION ENGINE - test-trading
🕐 Tick: 130 | Symbol: 1
📁 Session: C:\Users\...\whistle-exchange\test-trading

📈 Symbol 1
  📚 Order Book:
    Price | Amount | Total (Top 10 Sells)
    170 | 18 | 3060
    Last Trade: @ 160 (1 units)
    Price | Amount | Total (Top 10 Buys)
    175 | 5 | 875
    160 | 24 | 3840
    150 | 30 | 4500

📊 Session Status:
  🔄 Orders Processed: 21
  ⏱️  Current Tick: 130
  📁 Session Directory: C:\Users\...\whistle-exchange\test-trading
```

**Features:**
- **Real-time Order Book**: Shows current bids and asks with quantities
- **Last Trade Tracking**: Displays the most recent trade with color coding
- **Session Statistics**: Orders processed, current tick, and session info
- **Smart Updates**: Only refreshes when there are actual changes

### Minimal Mode
Compact output for high-frequency monitoring:

```
🔄 Tick 100: 4 events
  📚 Book: BUY @ 150 (qty: 20)
  🔄 Order 1: ACCEPTED
  🔄 Order 123: ACCEPTED
🔄 Tick 101: 1 events
🔄 Tick 102: 1 events
```

### Basic Mode
Simple order book display:

```
📈 Symbol 1
  📚 Order Book:
    Price | Amount | Total (Top 10 Sells)
    155 | 7 | 1085
    Last Trade: @ None
    Price | Amount | Total (Top 10 Buys)
    150 | 30 | 4500
```

## 🔄 **Session Engine Architecture**

### How It Works

1. **Session Creation**: Playground creates session directory with files
2. **Engine Startup**: Monitor starts Whistle engine for the session
3. **Order Processing**: 
   - Reads orders from `orders.jsonl`
   - Processes through Whistle engine
   - Writes responses to `responses.jsonl`
   - Writes trades to `trades.jsonl`
   - Writes book updates to `book_updates.jsonl`
4. **Real-time Display**: Updates dashboard with current state

### File Communication

```
┌─────────────────┐    File-based    ┌─────────────────┐
│   Playground    │ ◄──────────────► │  Session Engine │
│   (Client)      │   Communication  │   (Monitor)     │
└─────────────────┘                  └─────────────────┘
        │                                      │
        │ Writes orders to                     │ Reads orders from
        │ orders.jsonl                         │ orders.jsonl
        │                                      │
        │ Reads responses from                 │ Writes responses to
        │ responses.jsonl                      │ responses.jsonl
```

### Session Files

- **`orders.jsonl`**: Orders submitted by playground clients
- **`responses.jsonl`**: Order acceptance/rejection responses
- **`trades.jsonl`**: Executed trades with details
- **`book_updates.jsonl`**: Order book level changes

## 🎨 **Last Trade Color Coding**

The dashboard uses real exchange color coding:

- **🟢 Green**: Last trade was a **BUY** (taker bought)
- **🔴 Red**: Last trade was a **SELL** (taker sold)

This helps traders quickly understand market sentiment:
- **Green trades** indicate buying pressure (bullish)
- **Red trades** indicate selling pressure (bearish)

## 🔧 **Advanced Configuration**

### Engine Configuration

The session engine uses these default settings:
```rust
EngineCfg {
    symbol: 1,
    price_domain: PriceDomain { floor: 100, ceil: 200, tick: 1 },
    bands: Bands { mode: BandMode::Percent(1000) },
    batch_max: 1024,
    arena_capacity: 4096,
    elastic_arena: false,
    exec_shift_bits: 12,
    exec_id_mode: ExecIdMode::Sharded,
    self_match_policy: SelfMatchPolicy::Skip,
    allow_market_cold_start: false,
    reference_price_source: ReferencePriceSource::SnapshotLastTrade,
}
```

### Tick Processing

- **Automatic Ticks**: Engine processes ticks at configurable intervals
- **Order Processing**: New orders are processed each tick
- **Event Emission**: Trades, book changes, and lifecycle events are emitted
- **File Updates**: All events are written to session files

## 📈 **Real-time Features**

### Smart Display Updates

The dashboard only updates when there are actual changes:
- **New orders** submitted
- **Trades executed**
- **Tick advancement**
- **Periodic refresh** (every 5 seconds)

This prevents screen flickering and provides smooth real-time updates.

### Order Book Aggregation

The order book shows aggregated price levels:
```
Price | Amount | Total
160   | 15     | 2400
```

This means there are **15 total units** at price 160, which could be:
- 1 order of 15 units, OR
- 2 orders: 10 + 5 units, OR
- 3 orders: 5 + 5 + 5 units

This is how real exchanges display order books.

### Market Order Handling

When market orders are submitted with no opposing liquidity:
- **Order is accepted** into the system
- **No matching occurs** (no opposing orders)
- **Order disappears** (doesn't rest in book)
- **No trade is generated**

This is correct behavior for market orders.

## 🔮 **Integration Examples**

### Complete Trading Session

1. **Create Session**:
   ```bash
   cargo run --bin whistle-playground -- create-session demo --accounts 3
   ```

2. **Start Monitor**:
   ```bash
   cargo run --bin whistle-monitor -- start-session demo --display dashboard
   ```

3. **Submit Orders**:
   ```bash
   # Account 1: Market maker
   cargo run --bin whistle-playground -- submit demo --account-id 1 --side buy --order-type limit --price 150 --qty 20
   cargo run --bin whistle-playground -- submit demo --account-id 1 --side sell --order-type limit --price 155 --qty 20
   
   # Account 2: Takes liquidity
   cargo run --bin whistle-playground -- submit demo --account-id 2 --side buy --order-type market --qty 5
   
   # Account 3: Places limit order
   cargo run --bin whistle-playground -- submit demo --account-id 3 --side sell --order-type limit --price 160 --qty 10
   ```

4. **Watch Real-time Updates**:
   - See orders appear in order book
   - Watch trades execute with color coding
   - Monitor session statistics

### Multi-Session Trading

You can run multiple sessions simultaneously:

```bash
# Terminal 1: Session A
cargo run --bin whistle-monitor -- start-session session-a --display dashboard

# Terminal 2: Session B  
cargo run --bin whistle-monitor -- start-session session-b --display dashboard

# Terminal 3: Submit orders to both sessions
cargo run --bin whistle-playground -- submit session-a --account-id 1 --side buy --order-type limit --price 150 --qty 10
cargo run --bin whistle-playground -- submit session-b --account-id 1 --side sell --order-type limit --price 155 --qty 10
```

## 🚨 **Troubleshooting**

### Common Issues

**Session not found**:
```
❌ Failed to start session engine: Session 'my-session' does not exist
💡 Create the session first with: cargo run --bin whistle-playground -- create-session my-session
```

**File lock errors**:
```
error: failed to remove file ... Access is denied. (os error 5)
```
Solution: Kill the running monitor process and restart.

**No updates in dashboard**:
- Check that orders are being submitted to the correct session
- Verify the session engine is running
- Check session files for data

### Performance Tips

- **Use minimal mode** for high-frequency monitoring
- **Increase tick interval** for less frequent updates
- **Monitor session files** for debugging
- **Use multiple terminals** for different sessions

## 🔮 **Future Enhancements**

- **WebSocket Integration**: Real-time data streaming to web clients
- **Multi-Symbol Support**: Monitor multiple symbols in one dashboard
- **Historical Data**: Persistent storage and replay capabilities
- **Alert System**: Price movement and volume alerts
- **Order Book Depth**: Full order book visualization
- **Trade Analytics**: Volume analysis and market microstructure

## 🛠️ **Development**

### Building
```bash
# Build the monitor
cargo build --bin whistle-monitor

# Run directly
./target/debug/whistle-monitor --help

# Or with cargo
cargo run --bin whistle-monitor -- --help
```

### Testing
```bash
# Test session listing
cargo run --bin whistle-monitor -- list-sessions

# Test session info
cargo run --bin whistle-monitor -- session-info test-trading

# Test session engine
cargo run --bin whistle-monitor -- start-session test-trading --display dashboard
```

## 📚 **Related Documentation**

- [Whistle Playground README](../whistle-playground/README.md) - Session and account management
- [Whistle Engine Documentation](../../engine/whistle/README.md) - Core matching engine
- [Session System Documentation](../whistle-playground/src/session/README.md) - Session management internals
