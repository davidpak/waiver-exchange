# Whistle Playground

An interactive CLI tool for testing and exploring the Whistle matching engine in real-time.

## Features

- **Interactive Mode**: Real-time testing with a live Whistle engine
- **Demo Mode**: Quick demonstration of the engine's capabilities
- **Order Submission**: Submit various order types (limit, market, IOC, post-only)
- **Order Cancellation**: Cancel existing orders
- **Tick Processing**: Manually advance ticks and see events
- **Status Monitoring**: View engine state and queue statistics
- **Colored Output**: Easy-to-read colored terminal output

## Usage

### Interactive Mode

Start an interactive session with a Whistle engine:

```bash
# Default configuration
cargo run --bin whistle-playground interactive

# Custom configuration
cargo run --bin whistle-playground interactive \
  --symbol 42 \
  --price-floor 100 \
  --price-ceil 200 \
  --tick-size 5 \
  --batch-max 1024 \
  --arena-capacity 4096
```

### Demo Mode

Run a quick demonstration:

```bash
cargo run --bin whistle-playground demo --symbol 42
```

## Interactive Commands

Once in interactive mode, you can use these commands:

### Basic Commands
- `help`, `h` - Show available commands
- `quit`, `q`, `exit` - Exit the playground
- `status`, `s` - Show engine status
- `tick`, `t` - Process next tick
- `clear`, `c` - Clear the message queue
- `demo`, `d` - Run quick demo

### Order Management
- `submit`, `sub` - Submit order (interactive prompts)
- `cancel`, `can` - Cancel order (interactive prompt)

### Quick Commands
- `submit buy limit 150 10` - Buy 10 @ 150
- `submit sell market 5` - Sell 5 @ market
- `cancel 123` - Cancel order 123

## Examples

### Interactive Session

```
🚀 Whistle Playground - Interactive Mode
Symbol: 42, Price Range: 100-200, Tick: 5, Batch: 1024, Arena: 4096
Type 'help' for available commands

whistle> submit buy limit 150 10
📝 Submit Order
Side (buy/sell): buy
Order type (limit/market/ioc/postonly): limit
Price: 150
Quantity: 10
OK ✓ Order submitted successfully

whistle> tick
🔄 Processing tick...
  Generated 2 events:
    1. Lifecycle(EvLifecycle { symbol: 42, tick: 100, kind: Accepted, order_id: 123456789, reason: None })
    2. TickComplete(EvTickComplete { symbol: 42, tick: 100 })

whistle> status
📊 Engine Status
  Tick: 101
  Queue: 0/1024 messages
  Symbol: 42
  Price Domain: 100-200 (tick: 5)
```

### Demo Output

```
🎬 Whistle Playground - Demo Mode
Running quick demo with symbol 42

📝 Running demo sequence...
  Buy Limit 150@10
    OK ✓ Enqueued
  Sell Limit 160@5
    OK ✓ Enqueued
  Buy Market 20
    OK ✓ Enqueued
  Cancel Order 2
    OK ✓ Enqueued

🔄 Processing tick...
  Generated 4 events:
    1. Lifecycle(EvLifecycle { symbol: 42, tick: 100, kind: Accepted, order_id: 1, reason: None })
    2. Lifecycle(EvLifecycle { symbol: 42, tick: 100, kind: Accepted, order_id: 2, reason: None })
    3. Lifecycle(EvLifecycle { symbol: 42, tick: 100, kind: Accepted, order_id: 3, reason: None })
    4. TickComplete(EvTickComplete { symbol: 42, tick: 100 })

📊 Engine Status
  Tick: 101
  Queue: 0/1024 messages
  Symbol: 42
  Price Domain: 100-200 (tick: 5)
```

## Development

This tool is perfect for:

- **Testing new features** before implementing them in the main engine
- **Debugging** order processing logic
- **Demonstrating** the engine's capabilities
- **Learning** how the matching engine works
- **Performance testing** with different configurations

## Building

```bash
# Build the tool
cargo build --bin whistle-playground

# Run directly
./target/debug/whistle-playground interactive

# Or with cargo
cargo run --bin whistle-playground interactive
```

## Configuration

The playground uses the same `EngineCfg` as the main Whistle engine, so you can test with different:

- **Price domains** (floor, ceiling, tick size)
- **Queue capacities** (batch max)
- **Arena sizes** (arena capacity)
- **Symbol IDs** for multi-symbol testing

This makes it easy to test edge cases and performance characteristics.
