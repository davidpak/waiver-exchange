# Whistle Monitor

A real-time monitoring dashboard for the Whistle exchange simulation.

## Overview

The `whistle-monitor` CLI tool provides a comprehensive monitoring interface for the Whistle matching engine. It can simulate multiple symbols, display real-time market data, and provide insights into order book activity and trade execution.

## Features

- **Real-time Dashboard**: Live monitoring of multiple symbols with market data updates
- **Multi-Symbol Simulation**: Run multiple Whistle engines simultaneously
- **Market Data Tracking**: Last trade prices, bid/ask spreads, trade history
- **Order Book Visualization**: View order book depth and changes
- **Trade Analysis**: Monitor trade execution and patterns

## Commands

### Dashboard Mode
Start a real-time monitoring dashboard:
```bash
cargo run --bin whistle-monitor -- dashboard --symbols 3 --update-ms 100
```

**Options:**
- `--symbols`: Number of symbols to simulate (default: 3)
- `--update-ms`: Update frequency in milliseconds (default: 100)

### Simulation Mode
Run a timed simulation with statistics:
```bash
cargo run --bin whistle-monitor -- simulate --symbols 5 --duration 30
```

**Options:**
- `--symbols`: Number of symbols to simulate (default: 3)
- `--duration`: Simulation duration in seconds (default: 30)

### Order Book View
Display order book for a specific symbol:
```bash
cargo run --bin whistle-monitor -- order-book --symbol 1
```

### Trade History
Show recent trades for a specific symbol:
```bash
cargo run --bin whistle-monitor -- trades --symbol 1 --count 20
```

### Manual Simulation
Run a comprehensive manual simulation that tests all matching engine features:
```bash
cargo run --bin whistle-monitor -- manual-simulation --symbol 1 --tick-delay-ms 2000
```

This simulation tests:
- Initial liquidity setup
- Market order execution
- Partial fills
- Multiple level matching
- POST-ONLY orders (acceptance and rejection)
- Self-match prevention
- IOC orders
- Price-time priority
- Full book sweeps

### Test Partial Fill
Test the partial fill scenario specifically:
```bash
cargo run --bin whistle-monitor -- test-partial-fill --symbol 1
```

## Dashboard Display

The real-time dashboard shows:

- **Symbol Summary**: Each symbol with its current market state
- **Last Trade**: Price, quantity, and timestamp of the most recent trade
- **Bid/Ask Spread**: Current best bid and ask prices with quantities
- **Recent Trades**: Last 3 trades with side indicators (green=buy, red=sell)
- **Market Activity**: Real-time order submission and matching

## Example Output

```
🎯 WHISTLE EXCHANGE MONITOR
  Tick: 105 | Time: 1703123456

📈 Symbol 1
  💰 Last Trade: 15 @ 155 (tick: 104)
  📊 Spread: 5 | Bid: 10@150 | Ask: 8@155
  🔄 Recent Trades: 15@155 12@150 8@155

📈 Symbol 2
  💰 Last Trade: 20 @ 160 (tick: 103)
  📊 Spread: 10 | Bid: 15@155 | Ask: 12@165
  🔄 Recent Trades: 20@160 18@155 5@165
```

## Integration with Whistle Playground

The monitoring tool works independently but can be used alongside the `whistle-playground` tool:

1. **Run the playground** in one terminal for manual testing:
   ```bash
   cargo run --bin whistle-playground -- interactive
   ```

2. **Run the monitor** in another terminal for real-time observation:
   ```bash
   cargo run --bin whistle-monitor -- dashboard
   ```

## Architecture

The monitoring tool uses:
- **ExchangeSimulator**: Manages multiple Whistle engines
- **MarketData**: Tracks real-time market state for each symbol
- **Event Processing**: Processes Whistle events to update market data
- **Real-time Display**: Updates the dashboard at configurable intervals

## Future Enhancements

- **WebSocket Integration**: Real-time data streaming to web clients
- **Order Book Depth**: Full order book visualization with multiple levels
- **Trade Analytics**: Volume analysis, price impact, and market microstructure
- **Historical Data**: Persistent storage and replay capabilities
- **Alert System**: Price movement and volume alerts
