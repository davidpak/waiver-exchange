# Whistle Playground

An **interactive CLI application** for testing and exploring the Whistle matching engine with session-based multi-account trading and real-time order processing.

## 🚀 **Overview**

The Whistle Playground provides a **complete interactive trading environment** where you can:
- **🚀 Interactive CLI** - No more long command-line arguments! Beautiful, intuitive interface
- **📊 Real-time Trading** - Orders are processed immediately with live symbol activation
- **🎯 Session Management** - Create, enter, and manage trading sessions seamlessly
- **👥 Multi-Account Trading** - Switch between accounts within sessions
- **⚡ Live Order Processing** - Built-in SessionEngine processes orders in real-time
- **🔗 Seamless Monitor Integration** - Works perfectly with Whistle Monitor for live dashboards

## 🎯 **Key Features**

- **🎮 Interactive CLI**: Beautiful, application-like command-line interface
- **⚡ Real-time SessionEngine**: Automatically starts processing orders when you enter a session
- **🎯 SymbolCoordinator Integration**: Actually activates symbols and processes orders
- **👥 Multi-Account Trading**: Switch between accounts and trade as different users
- **📊 Live Session Overview**: See session status, active symbols, and trading activity
- **🔗 File-based Communication**: Seamless integration with Whistle Monitor
- **🎨 Beautiful Terminal UI**: Colored output, ASCII art, and intuitive navigation

## 📋 **Quick Start**

### 1. Launch the Interactive CLI
```bash
# Start the interactive playground
cargo run --bin whistle-playground
```

### 2. Create a Trading Session
```
🎮 WHISTLE PLAYGROUND
Welcome to the interactive trading environment!

Main Menu:
  [s]essions - List and enter sessions
  [c]reate - Create new session
  [e]xit - Exit playground

> create

📝 Creating New Session
Session name: my-trading
Number of accounts: 5
Number of symbols: 8

✅ Session 'my-trading' created successfully!
```

### 3. Enter the Session
```
> sessions

📋 Available Sessions:
  🎯 my-trading (8 symbols, 5 accounts)
  🎯 test-engine (4 symbols, 3 accounts)

Enter session name or 'back' to return: my-trading

🎯 Entering Session: my-trading
🔄 Starting SessionEngine...
✅ SessionEngine started successfully!

📊 Session Overview - my-trading
  🎯 Top Symbols: 8 total
  👥 Accounts: 5 total
  📅 Created: Just now
  🚀 Status: Trading Active
  💡 Tip: Use 'submit' to place orders, 'symbols' to see status

<my-trading> <1> > 
```

### 4. Submit Orders Interactively
```
<my-trading> <1> > submit

📝 Submitting Order
Side (buy/sell): buy
Symbol ID (1-8): 1
Quantity: 100
Price: 150

✅ Order submitted successfully!
Order ID: 1756719366740005
Status: accepted
```

### 5. Start the Monitor (in another terminal)
```bash
# Start the session engine with beautiful dashboard
cargo run --bin whistle-monitor -- start-session my-trading --display dashboard
```

## 🎮 **Interactive CLI Commands**

### Main Menu Commands
- **`sessions`** - List and enter available sessions
- **`create`** - Create a new trading session
- **`exit`** - Exit the playground

### Session Commands (when inside a session)
- **`submit`** - Submit a new order with interactive prompts
- **`symbols`** - View all symbols and their activation status
- **`status`** - Show session and engine status
- **`account <id>`** - Switch to a different account
- **`back`** - Return to main menu
- **`help`** - Show available commands

## 🚀 **Real-time Features**

### Automatic SessionEngine
When you enter a session, the playground **automatically starts a SessionEngine** that:
- **Processes orders in real-time** from the session files
- **Activates symbols** as orders are submitted
- **Updates symbol status** to show which are active/inactive
- **Integrates with SymbolCoordinator** for proper symbol lifecycle management

### Live Symbol Status
```
<my-trading> <1> > symbols

📊 Symbol Status - my-trading
  🎯 Symbol 1: 🟢 Active (Last Trade: @ 160)
  🎯 Symbol 2: 🟢 Active (Last Trade: @ 155)
  🎯 Symbol 3: ⚪ Inactive
  🎯 Symbol 4: ⚪ Inactive
  🎯 Symbol 5: ⚪ Inactive
  🎯 Symbol 6: ⚪ Inactive
  🎯 Symbol 7: ⚪ Inactive
  🎯 Symbol 8: ⚪ Inactive

📈 Summary: 2 active, 6 inactive symbols
```

### Dynamic Input Validation
The playground automatically:
- **Shows correct symbol ID ranges** based on session configuration
- **Validates order parameters** in real-time
- **Provides contextual help** based on current session state

## 🔄 **Integration with Whistle Monitor**

The playground and monitor work together through **file-based communication**:

```
Playground (Interactive CLI)     Monitor (Real-time Dashboard)
        │                                │
        │ Creates sessions               │ Reads session files
        │ Submits orders                 │ Shows live updates
        │ Writes to files:               │ Displays:
        │ - orders.jsonl                 │ - Live order book
        │ - responses.jsonl              │ - Recent trades
        │ - trades.jsonl                 │ - Session stats
        │ - book_updates.jsonl           │ - Real-time activity
        └────────────────────────────────┘
```

### Complete Trading Workflow

1. **Create Session** (Playground Interactive CLI):
   ```
   > create
   Session name: my-trading
   Number of accounts: 5
   Number of symbols: 8
   ```

2. **Enter Session** (Playground):
   ```
   > sessions
   Enter session name: my-trading
   ```

3. **Submit Orders** (Playground):
   ```
   <my-trading> <1> > submit
   Side: buy
   Symbol: 1
   Quantity: 100
   Price: 150
   ```

4. **Watch Real-time Updates** (Monitor Dashboard):
   - See orders appear in the order book
   - Watch trades execute in real-time
   - Monitor last trade prices with color coding

## 🎨 **Order Types Supported**

### Limit Orders
- **Purpose**: Place orders at specific prices
- **Execution**: Only execute when market reaches the specified price
- **Example**: `Side: buy, Symbol: 1, Quantity: 100, Price: 150`

### Market Orders
- **Purpose**: Execute immediately at best available price
- **Execution**: Execute against existing orders in the book
- **Example**: `Side: sell, Symbol: 2, Quantity: 50, Price: (auto)`

## 🔧 **Advanced Usage**

### Multi-Account Trading Scenarios

#### Scenario 1: Market Making
```
<my-trading> <1> > submit
Side: buy
Symbol: 1
Quantity: 200
Price: 150

<my-trading> <1> > submit  
Side: sell
Symbol: 1
Quantity: 200
Price: 155

<my-trading> <1> > account 2
Switched to account 2

<my-trading> <2> > submit
Side: buy
Symbol: 1
Quantity: 50
Price: 155
```

#### Scenario 2: Price Discovery
```
<my-trading> <1> > submit
Side: buy
Symbol: 1
Quantity: 100
Price: 145

<my-trading> <2> > submit
Side: buy
Symbol: 1
Quantity: 150
Price: 150

<my-trading> <3> > submit
Side: sell
Symbol: 1
Quantity: 120
Price: 155
```

## 🚨 **Error Handling**

The interactive CLI provides clear error messages and guidance:
- **Session not found**: Use `create` to make a new session
- **Invalid parameters**: Real-time validation with helpful hints
- **Symbol out of range**: Shows correct range for current session

## 🔮 **Future Enhancements**

- **Real-time Order Book Display**: View live order book within the playground
- **Trade History**: Complete trade history per account
- **Position Tracking**: P&L and position management
- **Risk Management**: Position limits and exposure controls
- **Web Interface**: Browser-based trading interface

## 🛠️ **Development**

### Building
```bash
# Build the playground
cargo build --bin whistle-playground

# Run the interactive CLI
cargo run --bin whistle-playground
```

### Testing the Integration
```bash
# Terminal 1: Start playground
cargo run --bin whistle-playground

# Terminal 2: Start monitor
cargo run --bin whistle-monitor -- start-session my-trading --display dashboard

# Use playground to submit orders, watch monitor for live updates!
```

## 📚 **Related Documentation**

- [Whistle Monitor README](../whistle-monitor/README.md) - Real-time dashboard and monitoring
- [Whistle Engine Documentation](../../engine/whistle/README.md) - Core matching engine
- [Session System Documentation](./src/session/README.md) - Session management internals

## 🎉 **What's New in This Version**

- **🎮 Interactive CLI**: Beautiful, intuitive interface - no more long commands!
- **⚡ Real-time Processing**: Built-in SessionEngine processes orders immediately
- **🎯 Symbol Activation**: Symbols become active as orders are submitted
- **👥 Seamless Account Switching**: Switch between accounts within sessions
- **📊 Live Session Overview**: See session status and trading activity in real-time
- **🔗 Perfect Monitor Integration**: Works seamlessly with existing monitor tools
