# Whistle Playground

An interactive CLI tool for testing and exploring the Whistle matching engine with session-based multi-account trading.

## 🚀 **Overview**

The Whistle Playground provides a complete trading environment where you can:
- **Create and join trading sessions** with multiple accounts
- **Switch between accounts** and trade as different users
- **Submit orders** with automatic order ID generation
- **View real-time account status** and trading history
- **Monitor order book** and trade execution

## 🎯 **Key Features**

- **Session Management**: Create, join, and manage trading sessions
- **Multi-Account Trading**: Switch between accounts and trade as different users
- **Real-time Order Submission**: Submit orders with automatic ID generation
- **Account Status Monitoring**: View active orders, recent trades, and positions
- **File-based Communication**: Seamless integration with Whistle Monitor
- **Beautiful Terminal UI**: Colored output and intuitive commands

## 📋 **Quick Start**

### 1. Create a Trading Session
```bash
# Create a session with 5 accounts
cargo run --bin whistle-playground -- create-session my-trading --accounts 5
```

### 2. Start the Monitor (in another terminal)
```bash
# Start the session engine with beautiful dashboard
cargo run --bin whistle-monitor -- start-session my-trading --display dashboard
```

### 3. Submit Orders
```bash
# Submit as account 1
cargo run --bin whistle-playground -- submit my-trading --account-id 1 --side buy --order-type limit --price 150 --qty 10

# Submit as account 2
cargo run --bin whistle-playground -- submit my-trading --account-id 2 --side sell --order-type limit --price 155 --qty 5
```

## 🛠️ **Commands Reference**

### Session Management

#### Create Session
```bash
cargo run --bin whistle-playground -- create-session <NAME> --accounts <N>
```
Creates a new trading session with N accounts.

**Example:**
```bash
cargo run --bin whistle-playground -- create-session test-trading --accounts 5
# Output: Session 'test-trading' created with 5 accounts.
```

#### List Sessions
```bash
cargo run --bin whistle-playground -- list-sessions
```
Shows all available trading sessions.

#### Session Info
```bash
cargo run --bin whistle-playground -- session-info <SESSION_NAME>
```
Displays detailed information about a session.

#### Join Session
```bash
cargo run --bin whistle-playground -- join-session <NAME> --account-id <ID> --account-type <TYPE>
```
Joins an existing session as a specific account.

**Example:**
```bash
cargo run --bin whistle-playground -- join-session test-trading --account-id 1 --account-type trader
```

### Account Management

#### Switch Account
```bash
cargo run --bin whistle-playground -- switch-account <SESSION> <ACCOUNT_ID>
```
Switch to a different account in a session.

**Example:**
```bash
cargo run --bin whistle-playground -- switch-account test-trading 2
# Output: Switched to account 2 in session 'test-trading'
```

#### Account Status
```bash
cargo run --bin whistle-playground -- account-status <SESSION> --account-id <ID>
```
View detailed account status including recent trades and order book.

**Example:**
```bash
cargo run --bin whistle-playground -- account-status test-trading --account-id 2
```

**Output:**
```
📊 Account Status - Session: test-trading
Account ID: 2

Recent Trades:
  🟢 BUY @ 155 (3 units)
  🔴 SELL @ 160 (5 units)

Current Order Book:
  Sells (Asks):
    🔴 @ 170 (15 units)
    🔴 @ 165 (8 units)
  Buys (Bids):
    🟢 @ 160 (12 units)
    🟢 @ 155 (20 units)
```

### Order Submission

#### Submit Order
```bash
cargo run --bin whistle-playground -- submit <SESSION> --account-id <ID> --side <SIDE> --order-type <TYPE> --price <PRICE> --qty <QTY>
```

**Parameters:**
- `SESSION`: Session name
- `--account-id`: Account ID (default: 1)
- `--side`: `buy` or `sell`
- `--order-type`: `limit`, `market`, `ioc`, or `post-only`
- `--price`: Price (required for limit orders)
- `--qty`: Quantity
- `--order-id`: Order ID (optional - auto-generated if not provided)

**Examples:**
```bash
# Limit buy order
cargo run --bin whistle-playground -- submit test-trading --account-id 1 --side buy --order-type limit --price 150 --qty 10

# Market sell order
cargo run --bin whistle-playground -- submit test-trading --account-id 2 --side sell --order-type market --qty 5

# IOC order (Immediate or Cancel)
cargo run --bin whistle-playground -- submit test-trading --account-id 3 --side buy --order-type ioc --price 155 --qty 8

# Post-only order
cargo run --bin whistle-playground -- submit test-trading --account-id 1 --side sell --order-type post-only --price 160 --qty 12
```

## 🔄 **Integration with Whistle Monitor**

The playground works seamlessly with the Whistle Monitor for real-time trading:

### Complete Trading Workflow

1. **Create Session** (Playground):
   ```bash
   cargo run --bin whistle-playground -- create-session my-trading --accounts 5
   ```

2. **Start Monitor** (Monitor):
   ```bash
   cargo run --bin whistle-monitor -- start-session my-trading --display dashboard
   ```

3. **Submit Orders** (Playground):
   ```bash
   # Account 1 buys
   cargo run --bin whistle-playground -- submit my-trading --account-id 1 --side buy --order-type limit --price 150 --qty 10
   
   # Account 2 sells
   cargo run --bin whistle-playground -- submit my-trading --account-id 2 --side sell --order-type limit --price 155 --qty 5
   
   # Account 3 market order (triggers trade)
   cargo run --bin whistle-playground -- submit my-trading --account-id 3 --side buy --order-type market --qty 3
   ```

4. **Watch Real-time Updates** (Monitor Dashboard):
   - See orders appear in the order book
   - Watch trades execute in real-time
   - Monitor last trade prices with color coding

## 📊 **Account Status Features**

The account status command provides comprehensive information:

### Recent Trades
- Shows last 5 trades with color coding (🟢 for buys, 🔴 for sells)
- Displays price, quantity, and side for each trade

### Current Order Book
- Real-time order book data from the session
- Shows sells (asks) and buys (bids) with quantities
- Sorted by price (ascending for sells, descending for buys)

### Account Information
- Account ID and session details
- Active orders and positions (coming soon)

## 🎨 **Order Types Explained**

### Limit Orders
- **Purpose**: Place orders at specific prices
- **Execution**: Only execute when market reaches the specified price
- **Example**: `--order-type limit --price 150 --qty 10`

### Market Orders
- **Purpose**: Execute immediately at best available price
- **Execution**: Execute against existing orders in the book
- **Example**: `--order-type market --qty 5`

### IOC Orders (Immediate or Cancel)
- **Purpose**: Execute immediately or cancel remaining quantity
- **Execution**: Execute what's possible, cancel the rest
- **Example**: `--order-type ioc --price 155 --qty 8`

### Post-Only Orders
- **Purpose**: Add liquidity without taking liquidity
- **Execution**: Only accepted if they don't cross existing orders
- **Example**: `--order-type post-only --price 160 --qty 12`

## 🔧 **Advanced Usage**

### Multi-Account Trading Scenarios

#### Scenario 1: Market Making
```bash
# Account 1: Market maker (provides liquidity)
cargo run --bin whistle-playground -- submit test-trading --account-id 1 --side buy --order-type limit --price 150 --qty 20
cargo run --bin whistle-playground -- submit test-trading --account-id 1 --side sell --order-type limit --price 155 --qty 20

# Account 2: Takes liquidity
cargo run --bin whistle-playground -- submit test-trading --account-id 2 --side buy --order-type market --qty 5
```

#### Scenario 2: Price Discovery
```bash
# Multiple accounts place orders at different prices
cargo run --bin whistle-playground -- submit test-trading --account-id 1 --side buy --order-type limit --price 145 --qty 10
cargo run --bin whistle-playground -- submit test-trading --account-id 2 --side buy --order-type limit --price 150 --qty 15
cargo run --bin whistle-playground -- submit test-trading --account-id 3 --side sell --order-type limit --price 155 --qty 12
cargo run --bin whistle-playground -- submit test-trading --account-id 4 --side sell --order-type limit --price 160 --qty 8
```

### Session Management

#### Cleanup Sessions
```bash
cargo run --bin whistle-playground -- cleanup-sessions
```
Removes expired sessions and cleans up temporary files.

## 🚨 **Error Handling**

The playground provides clear error messages for common issues:

- **Session not found**: Create the session first
- **Invalid order parameters**: Check price, quantity, and order type
- **Account not in session**: Join the session with the specified account
- **Price validation**: Ensure price is within valid range and tick-aligned

## 🔮 **Future Enhancements**

- **Interactive Trading Mode**: Real-time account dashboard
- **Position Tracking**: P&L and position management per account
- **Order History**: Complete order lifecycle tracking
- **Risk Management**: Position limits and exposure controls
- **Web Interface**: Browser-based trading interface

## 🛠️ **Development**

### Building
```bash
# Build the playground
cargo build --bin whistle-playground

# Run directly
./target/debug/whistle-playground --help

# Or with cargo
cargo run --bin whistle-playground -- --help
```

### Testing
```bash
# Test session creation
cargo run --bin whistle-playground -- create-session test --accounts 3

# Test order submission
cargo run --bin whistle-playground -- submit test --side buy --order-type limit --price 150 --qty 10

# Test account switching
cargo run --bin whistle-playground -- switch-account test 2
```

## 📚 **Related Documentation**

- [Whistle Monitor README](../whistle-monitor/README.md) - Real-time dashboard and monitoring
- [Whistle Engine Documentation](../../engine/whistle/README.md) - Core matching engine
- [Session System Documentation](./src/session/README.md) - Session management internals
