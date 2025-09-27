# Waiver Exchange - Conversion Standards

## Overview

This document defines the standardized conversion factors used throughout the Waiver Exchange system to ensure consistency across all components.

## Core Conversion Standards

### Quantity Conversion (Shares)

**Standard**: 10000 basis points = 1 share

- **Database Storage**: Quantities stored as basis points (1/10000th of a share)
- **API Input/Output**: Quantities in whole shares
- **Internal Calculations**: All calculations use basis points

**Examples**:
- 1 share = 10000 basis points
- 0.5 shares = 5000 basis points  
- 10 shares = 100,000 basis points
- 100 shares = 1,000,000 basis points

### Price Conversion (Currency)

**Standard**: 1 cent = 1 unit

- **Database Storage**: Prices stored in cents
- **API Input/Output**: Prices in cents
- **Internal Calculations**: All calculations use cents

**Examples**:
- $1.00 = 100 cents
- $15.00 = 1500 cents
- $16.00 = 1600 cents

### Position Value Calculation

**Formula**: `position_value = (quantity_bp * price_cents) / 10000`

Where:
- `quantity_bp` = quantity in basis points
- `price_cents` = price in cents
- Division by 10000 converts basis points to shares

**Example**:
- 100,000 basis points × 1500 cents ÷ 10000 = 15,000 cents = $150.00

### Trade Settlement Calculations

**Buy Order**:
```rust
let cost_cents = (quantity_bp * price_cents) / 10000;
account.cash_balance -= cost_cents;
```

**Sell Order**:
```rust
let proceeds_cents = (quantity_bp * price_cents) / 10000;
let cost_basis_cents = (quantity_bp * avg_cost_cents) / 10000;
let realized_pnl = proceeds_cents - cost_basis_cents;
account.cash_balance += proceeds_cents;
account.realized_pnl += realized_pnl;
```

### Unrealized P&L Calculation

**Formula**: `unrealized_pnl = (current_price - avg_cost) * quantity_bp / 10000`

**Example**:
- Position: 100,000 basis points @ $15.00 avg cost
- Current price: $16.00
- Unrealized P&L = (1600 - 1500) × 100,000 ÷ 10000 = 1,000 cents = $10.00

## Implementation Guidelines

### Constants

Define these constants at the top of each module:

```rust
const QTY_SCALE: i64 = 10000;  // 10000 basis points = 1 share
const CENTS: i64 = 1;          // 1 cent = 1 unit
```

### Database Schema

```sql
-- Positions table
CREATE TABLE positions (
    quantity BIGINT NOT NULL,  -- Stored as basis points (1/10000th of a share)
    avg_cost BIGINT NOT NULL,  -- Average cost in cents
    -- ...
);

-- Trades table  
CREATE TABLE trades (
    quantity BIGINT NOT NULL,  -- Stored as basis points (1/10000th of a share)
    price BIGINT NOT NULL,     -- Price in cents
    -- ...
);
```

### API Contracts

**Order Placement**:
```json
{
    "quantity": 10,    // Whole shares
    "price": 1600      // Cents
}
```

**Position Response**:
```json
{
    "quantity": 100000,  // Basis points (10 shares)
    "avg_cost": 1500,    // Cents
    "current_value": 15000  // Cents
}
```

## Migration Notes

### From Previous System

The system has been standardized on 10,000 basis points = 1 share to match the database schema and account service implementation.

### Database Migration

Existing data in the database already uses the correct 10,000:1 ratio, so no data migration is required.

## Testing

### Validation Examples

```rust
// Test position value calculation
let quantity_bp = 100000;  // 10 shares
let price_cents = 1500;    // $15.00
let expected_value = 15000; // $150.00

let actual_value = (quantity_bp * price_cents) / QTY_SCALE;
assert_eq!(actual_value, expected_value);
```

### Edge Cases

- **Zero quantity**: Handle division by zero gracefully
- **Negative quantities**: Support short positions if needed
- **Precision**: Use integer arithmetic to avoid floating point errors

## Compliance

All components must use these conversion standards:

- ✅ **Database Layer**: Positions, trades, reservations
- ✅ **API Layer**: Order placement, position queries
- ✅ **Business Logic**: EVS, order matching, settlement
- ✅ **Frontend**: Display formatting, input validation
- ✅ **Tests**: All test data and assertions

## Version History

- **v1.0** (2025-09-26): Standardized on 10000:1 basis point ratio to match database schema
- **v0.x**: Inconsistent ratios across components (deprecated)
