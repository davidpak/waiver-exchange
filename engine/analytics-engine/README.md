# AnalyticsEngine

Comprehensive analytics and observability system for the Waiver Exchange.

## Overview

The AnalyticsEngine provides real-time metrics collection, historical data storage, and analytics capabilities for monitoring system performance, business metrics, and operational health.

## Features

- **Event Ingestion**: Receives analytics events from ExecutionManager
- **Real-time Aggregation**: Aggregates metrics within configurable time windows
- **Data Storage**: Stores analytics data (currently in-memory, Parquet support planned)
- **Query Engine**: Provides querying capabilities for analytics data
- **CLI Interface**: Command-line interface for system monitoring

## Architecture

```
ExecutionManager → AnalyticsEngine → Storage → Query Engine → CLI
```

## Usage

### Running the CLI

```bash
# Show system health
cargo run --bin analytics-cli -- health --hours 24

# Show performance metrics
cargo run --bin analytics-cli -- performance --hours 24

# Show business metrics
cargo run --bin analytics-cli -- business --hours 24

# Show operational metrics
cargo run --bin analytics-cli -- operational --hours 24

# Show system status
cargo run --bin analytics-cli -- status
```

### Running the Demo

```bash
cargo run --example analytics_demo
```

## Configuration

The AnalyticsEngine uses a configuration system with the following components:

- **Storage**: Data storage configuration
- **Ingestion**: Event ingestion settings
- **Aggregation**: Metrics aggregation configuration
- **Query**: Query engine settings
- **Retention**: Data retention policies

## Event Types

The system supports four types of analytics events:

1. **Performance**: Tick duration, latency, CPU, memory usage
2. **Business**: Orders, trades, volume, account activity
3. **System Health**: Crashes, errors, uptime, queue overflows
4. **Operational**: Symbol management, thread utilization, I/O

## Future Enhancements

- Full Parquet storage implementation
- SQL query support
- Web dashboard interface
- Email alerting system
- Real-time streaming analytics

## Status

✅ **Core Implementation**: Complete
✅ **CLI Interface**: Complete
✅ **Event Types**: Complete
🔄 **Parquet Storage**: Simplified (in-memory)
🔄 **SQL Queries**: Simplified (specific commands)
⏳ **Web Dashboard**: Planned
⏳ **Alerting**: Planned

This is a foundational implementation that provides the core analytics infrastructure for the Waiver Exchange system.
