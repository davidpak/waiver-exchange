# The Waiver Exchange

A high-performance fantasy football trading platform built in Rust. Trade NFL players like stocks with real-time pricing, fractional share support, and a full order matching engine.

## Architecture

The system is composed of three layers: a Rust backend (17 crates), a Next.js frontend, and supporting data pipeline tools.
<img width="3215" height="1970" alt="The Waiver Exchange - Architecture" src="https://github.com/user-attachments/assets/5db43598-b0c0-4944-9065-71384e06e57e" />

### Backend Services

| Service | Binary | Port | Description |
|---------|--------|------|-------------|
| **Exchange Engine** | `waiver-exchange` | 8081 | Main service: matching engine, WebSocket API, order processing, settlement |
| **OAuth Server** | `oauth-server` | 8082 | Google OAuth authentication server |
| **REST API** | `rest-server` | 8083 | REST API for prices, account data, market snapshots |
| **Market Maker** | `market-maker` | — | Automated liquidity provider using RPE fair prices |

### Core Engine Crates

| Crate | Purpose |
|-------|---------|
| `whistle` | Deterministic per-symbol order matching engine (FIFO price-time priority) |
| `symbol-coordinator` | Per-symbol engine lifecycle management and thread placement |
| `order-router` | Sharded order routing to symbol engines via SPSC queues |
| `execution-manager` | Post-match event normalization, settlement, and dispatch |
| `simulation-clock` | Tick-based system heartbeat driving time progression |
| `persistence` | WAL + snapshot-based state persistence and recovery |

### Service Crates

| Crate | Purpose |
|-------|---------|
| `order-gateway` | WebSocket + REST API server (hosts `oauth-server` and `rest-server` binaries) |
| `account-service` | User accounts, balances, positions, Google OAuth, Sleeper integration |
| `equity-service` | Real-time equity calculation and P&L tracking |
| `rpe-engine` | Reference Price Engine — Fair Price 2.3 algorithm for player valuations |
| `market-maker` | Automated market making around fair prices |
| `analytics-engine` | Event ingestion, metrics collection, CLI querying |

### Data Pipeline Crates

| Crate | Purpose |
|-------|---------|
| `player-registry` | NFL player to trading symbol mapping (deterministic hashing) |
| `player-scraper` | Web scraper for NFL player data, leaderboards, weekly stats |
| `sportsdataio-fetcher` | SportsDataIO API integration for player stats |

### Frontend

| Stack | Description |
|-------|-------------|
| Next.js 15 + React 19 | App framework with Turbopack |
| Mantine 8 | UI component library |
| Zustand | Auth state management |
| TanStack React Query | Server state with polling |
| Framer Motion | Animations |

### Infrastructure

| Component | Purpose |
|-----------|---------|
| PostgreSQL | Primary database (accounts, positions, trades, prices) |
| Redis | Caching layer |

## Quick Start

### Prerequisites

- Rust (latest stable)
- Node.js 18+
- PostgreSQL 15+
- Redis

### 1. Build

```bash
git clone https://github.com/davidpak/waiver-exchange.git
cd waiver-exchange
cargo build --workspace
```

### 2. Database Setup

```bash
# Start PostgreSQL and Redis (Docker or local install)
docker run -d --name waiver-db -e POSTGRES_PASSWORD=password -e POSTGRES_DB=waiver_exchange -p 5432:5432 postgres:15
docker run -d --name waiver-redis -p 6379:6379 redis:alpine
```

### 3. Environment Variables

```bash
export DATABASE_URL="postgresql://postgres:password@localhost:5432/waiver_exchange"
export REDIS_URL="redis://localhost:6379"
export GOOGLE_CLIENT_ID="your-client-id"
export GOOGLE_CLIENT_SECRET="your-client-secret"
export GOOGLE_REDIRECT_URL="http://localhost:8082/auth/callback"
```

### 4. Run Backend Services

```bash
# Main exchange engine (port 8081)
cargo run -p waiver-exchange-service

# OAuth server (port 8082)
cargo run -p order-gateway --bin oauth-server

# REST API server (port 8083)
cargo run -p order-gateway --bin rest-server
```

### 5. Run Frontend

```bash
cd waiver-exchange-frontend
npm install
npm run dev
# Frontend runs on http://localhost:3000
```

### 6. Data Pipeline (Optional)

```bash
# Scrape NFL player data
cargo run -p player-scraper --bin scrape_players
cargo run -p player-scraper --bin scrape_all_weeks

# Fetch from SportsDataIO API
cargo run -p sportsdataio-fetcher --bin populate-data

# Map players between data sources
cargo run -p player-mapping-script
```

## Project Structure

```
waiver-exchange/
├── engine/                          # Rust backend (17 crates)
│   ├── whistle/                     # Matching engine
│   ├── whistle-bench/               # Performance benchmarks
│   ├── symbol-coordinator/          # Symbol lifecycle management
│   ├── order-router/                # Order routing
│   ├── execution-manager/           # Trade settlement
│   ├── simulation-clock/            # System heartbeat
│   ├── persistence/                 # WAL + snapshots
│   ├── order-gateway/               # WebSocket + REST + OAuth APIs
│   ├── account-service/             # User accounts and auth
│   ├── equity-service/              # Equity calculations
│   ├── rpe-engine/                  # Fair price algorithm
│   ├── market-maker/                # Automated market making
│   ├── analytics-engine/            # Metrics and observability
│   ├── player-registry/             # Player-symbol mapping
│   ├── player-scraper/              # NFL data scraping
│   ├── sportsdataio-fetcher/        # SportsDataIO integration
│   └── waiver-exchange-service/     # Main orchestrator binary
├── waiver-exchange-frontend/        # Next.js frontend
├── tools/
│   ├── admin-cli/                   # Administrative CLI
│   └── player-mapping-script/       # Player data mapping utility
├── scripts/
│   └── linux/                       # Linux deployment scripts
├── docs/                            # Documentation
│   ├── design/                      # System design documents
│   ├── adr/                         # Architecture Decision Records
│   ├── api/                         # API documentation
│   ├── backend/                     # Backend implementation docs
│   ├── frontend/                    # Frontend documentation
│   └── deployment/                  # Deployment guides
└── data/                            # Runtime data (players, snapshots)
```

## Key Concepts

### Trading Model
- Each NFL player = one tradeable symbol (deterministic hash mapping)
- Prices in **cents** (e.g., $50.00 = 5000)
- Quantities in **basis points** (10,000 bp = 1 full share)
- Order types: Limit, Market, IOC, Post-Only
- Self-match prevention between same-account orders

### Fair Pricing (RPE 2.3)
- Baseline from season projections
- Adjusted by NFL leaderboard rankings and weekly performance
- 80% leaderboard score + 20% momentum (EMA of recent deltas)
- Price bands: +/-30% of baseline

### System Flow
```
User → OrderGateway (WebSocket) → OrderRouter → SymbolCoordinator → Whistle (matching)
                                                                         ↓
User ← MarketData Broadcast ← ExecutionManager ← AccountService (settlement)
                                     ↓
                              Persistence (WAL + snapshots)
```

## Development

```bash
cargo build --workspace          # Build all crates
cargo test --workspace           # Run all tests
cargo fmt --all                  # Format code
cargo clippy --workspace         # Lint
cargo bench -p whistle-bench     # Run benchmarks
```

## Documentation

- [System Design Master](docs/design/master.md) — Full architecture overview
- [API Reference](docs/api/waiver-exchange-api-documentation.md) — REST + WebSocket API docs
- [Architecture Decision Records](docs/adr/) — Key design decisions
- [Development Guide](docs/DEVELOPMENT.md) — Contributing and code standards
- [Deployment Guide](docs/deployment/production_deployment_plan.md) — Production setup
- [Backend Implementation](docs/backend/) — Backend subsystem docs
- [Frontend Guide](docs/frontend/frontend_master.md) — Frontend architecture

## License

MIT
