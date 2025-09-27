# EVS Integration Test Specification

## Overview

This document provides a comprehensive specification for the Equity Valuation Service (EVS) integration test suite. The test validates the complete trade lifecycle from order placement through equity calculation and database persistence.

## Test Environment Setup

### Prerequisites
- PostgreSQL database running on `localhost:5432`
- Redis running on `localhost:6379`
- All Waiver Exchange services running (OrderGateway, ExecutionManager, EVS, etc.)

### Test Accounts
| Account ID | API Key | Google ID | Initial Cash | Initial Position |
|------------|---------|-----------|--------------|------------------|
| 7 | `ak_test_7_abcdef1234567890` | `user7` | $50,000.00 | 10 shares @ $15.00 |
| 8 | `ak_test_1234567890abcdef` | `user123` | $10,000.00 | 20 shares @ $15.00 |
| 9 | `ak_admin_abcdef1234567890` | `admin` | $20,000.00 | 15 shares @ $15.00 |

### Conversion Standards
- **Quantity**: 10,000 basis points = 1 share
- **Price**: 1 cent = 1 unit
- **Position Value**: `(quantity_bp * price_cents) / 10000`

## Test Scenarios

### Scenario 1: Bootstrap Setup

**Purpose**: Initialize test data and verify starting conditions

**Steps**:
1. Clear all test data from database
2. Create test accounts (7, 8, 9) with correct `google_id` mappings
3. Insert initial positions in database
4. Verify initial state

**Expected Results**:
```
Account 7: 100,000 basis points (10 shares) @ $15.00
Account 8: 200,000 basis points (20 shares) @ $15.00  
Account 9: 150,000 basis points (15 shares) @ $15.00
```

**Database Verification**:
```sql
-- Positions table
account_id | symbol_id | quantity | avg_cost
7         | 764       | 100000   | 1500
8         | 764       | 200000   | 1500
9         | 764       | 150000   | 1500
```

### Scenario 2: Basic Trade Execution and EVS Calculation

**Purpose**: Test single trade execution and equity recalculation

**Trade Details**:
- **Seller**: Account 8
- **Buyer**: Account 9
- **Quantity**: 10 shares (100,000 basis points)
- **Price**: $16.00 (1600 cents)

**Expected Trade Flow**:

#### Step 1: Order Placement
```
Account 8 SELL order: 100,000 bp @ 1600 cents
Account 9 BUY order: 100,000 bp @ 1600 cents
```

#### Step 2: Trade Settlement
**Account 8 (Seller)**:
- Cash received: `(100,000 * 1600) / 10000 = 16,000 cents = $160.00`
- Position reduced: `200,000 - 100,000 = 100,000 bp (10 shares)`
- Cost basis: `(100,000 * 1500) / 10000 = 15,000 cents = $150.00`
- Realized P&L: `16,000 - 15,000 = 1,000 cents = $10.00`

**Account 9 (Buyer)**:
- Cash spent: `(100,000 * 1600) / 10000 = 16,000 cents = $160.00`
- Position increased: `150,000 + 100,000 = 250,000 bp (25 shares)`
- New average cost: `((150,000 * 1500) + (100,000 * 1600)) / 250,000 = 1,540 cents`

#### Step 3: Equity Calculation
**Account 8**:
- Cash balance: `$10,000.00 + $160.00 = $10,160.00`
- Position value: `(100,000 * 1600) / 10000 = 16,000 cents = $160.00`
- Total equity: `$10,160.00 + $160.00 = $10,320.00`
- Realized P&L: `$10.00`
- Unrealized P&L: `(1600 - 1500) * 100,000 / 10000 = 1,000 cents = $10.00`

**Account 9**:
- Cash balance: `$20,000.00 - $160.00 = $19,840.00`
- Position value: `(250,000 * 1600) / 10000 = 40,000 cents = $400.00`
- Total equity: `$19,840.00 + $400.00 = $20,240.00`
- Realized P&L: `$0.00`
- Unrealized P&L: `(1600 - 1540) * 250,000 / 10000 = 1,500 cents = $15.00`

### Scenario 3: Quantity Flow and Basis Point Scaling

**Purpose**: Verify basis point conversions throughout the system

**Verification Points**:
1. Order placement uses correct basis point conversion
2. Trade settlement maintains basis point precision
3. Position updates preserve basis point accuracy
4. Equity calculations use proper scaling

**Expected Database State**:
```sql
-- Trades table
account_id | symbol_id | side | quantity | price
8         | 764       | Sell | 100000   | 1600
9         | 764       | Buy  | 100000   | 1600

-- Positions table  
account_id | symbol_id | quantity | avg_cost
7         | 764       | 100000   | 1500
8         | 764       | 100000   | 1500
9         | 764       | 250000   | 1540
```

### Scenario 4: Equity Persistence and REST API

**Purpose**: Test database persistence and REST API integration

**Steps**:
1. Verify equity snapshots are persisted to `equity_timeseries` table
2. Test REST API endpoint `/api/accounts/{id}/summary`
3. Validate equity history retrieval

**Expected Database State**:
```sql
-- equity_timeseries table
account_id | total_equity | cash_balance | position_value | unrealized_pnl | realized_pnl
8         | 1032000      | 1016000      | 16000          | 1000           | 1000
9         | 2024000      | 1984000      | 40000          | 1500           | 0
```

### Scenario 5: Complex Multi-Trade Scenario

**Purpose**: Test multiple trades and complex position calculations

#### Trade 1: Account 8 → Account 9
- **Quantity**: 5 shares (50,000 basis points)
- **Price**: $14.00 (1400 cents)

**Expected Results**:
**Account 8**:
- Position: `100,000 - 50,000 = 50,000 bp (5 shares)`
- Cash: `$10,160.00 + $70.00 = $10,230.00`
- Realized P&L: `$10.00 + $5.00 = $15.00`

**Account 9**:
- Position: `250,000 + 50,000 = 300,000 bp (30 shares)`
- Cash: `$19,840.00 - $70.00 = $19,770.00`
- New avg cost: `((250,000 * 1540) + (50,000 * 1400)) / 300,000 = 1,516.67 cents`

#### Trade 2: Account 7 → Account 8
- **Quantity**: 3 shares (30,000 basis points)
- **Price**: $18.00 (1800 cents)

**Expected Results**:
**Account 7**:
- Position: `100,000 - 30,000 = 70,000 bp (7 shares)`
- Cash: `$50,000.00 + $54.00 = $50,054.00`
- Realized P&L: `$9.00`

**Account 8**:
- Position: `50,000 + 30,000 = 80,000 bp (8 shares)`
- Cash: `$10,230.00 - $54.00 = $10,176.00`
- New avg cost: `((50,000 * 1500) + (30,000 * 1800)) / 80,000 = 1,612.5 cents`

## Final Expected State

### Position Summary
| Account | Shares | Avg Cost | Current Value | Unrealized P&L |
|---------|--------|----------|---------------|----------------|
| 7 | 7 | $15.00 | $126.00 | $21.00 |
| 8 | 8 | $16.13 | $128.00 | $1.00 |
| 9 | 30 | $15.17 | $480.00 | $25.00 |

### Cash Balances
| Account | Initial | Final | Change |
|---------|---------|-------|--------|
| 7 | $50,000.00 | $50,054.00 | +$54.00 |
| 8 | $10,000.00 | $10,176.00 | +$176.00 |
| 9 | $20,000.00 | $19,770.00 | -$230.00 |

### Total Equity
| Account | Cash | Position | Total | Day Change |
|---------|------|----------|-------|------------|
| 7 | $50,054.00 | $126.00 | $50,180.00 | +$180.00 |
| 8 | $10,176.00 | $128.00 | $10,304.00 | +$304.00 |
| 9 | $19,770.00 | $480.00 | $20,250.00 | +$250.00 |

## Test Validation Points

### 1. Trade Settlement Accuracy
- [ ] Correct cash balance updates
- [ ] Accurate position quantity changes
- [ ] Proper realized P&L calculations
- [ ] Correct average cost calculations

### 2. Equity Calculation Precision
- [ ] Position values calculated correctly
- [ ] Unrealized P&L reflects current market price
- [ ] Total equity = cash + position value
- [ ] Day change calculations accurate

### 3. Database Consistency
- [ ] Trades table records all transactions
- [ ] Positions table reflects current holdings
- [ ] Equity snapshots persisted correctly
- [ ] No data corruption or inconsistencies

### 4. API Integration
- [ ] REST API returns correct account summaries
- [ ] WebSocket authentication works
- [ ] Order placement successful
- [ ] Real-time equity updates

### 5. Conversion Standards Compliance
- [ ] All quantities use 10,000:1 basis point ratio
- [ ] Prices stored in cents
- [ ] Calculations use proper scaling factors
- [ ] No floating-point precision errors

## Error Scenarios

### Expected Error Handling
1. **Insufficient Balance**: Buy orders with insufficient cash
2. **Insufficient Position**: Sell orders with insufficient shares
3. **Invalid Prices**: Orders with negative or zero prices
4. **Database Failures**: Graceful handling of persistence errors
5. **Network Issues**: WebSocket connection failures

### Recovery Testing
- [ ] System recovers from database disconnections
- [ ] Partial trade failures don't corrupt state
- [ ] WebSocket reconnection works
- [ ] Data consistency maintained after errors

## Performance Benchmarks

### Expected Performance
- Trade settlement: < 100ms
- Equity calculation: < 50ms
- Database persistence: < 200ms
- REST API response: < 500ms
- WebSocket latency: < 50ms

### Load Testing
- [ ] Handle 100+ concurrent trades
- [ ] Process 1000+ trades per minute
- [ ] Maintain accuracy under load
- [ ] No memory leaks or performance degradation

## Success Criteria

The test suite passes when:
1. ✅ All 5 scenarios complete successfully
2. ✅ All expected values match actual results
3. ✅ Database state is consistent
4. ✅ No calculation errors or precision issues
5. ✅ API endpoints return correct data
6. ✅ Performance benchmarks met
7. ✅ Error scenarios handled gracefully

## Troubleshooting Guide

### Common Issues
1. **Conversion Errors**: Check QTY_SCALE constant (should be 10000)
2. **Authentication Failures**: Verify API keys in OrderGateway
3. **Database Inconsistencies**: Check account_id mappings
4. **Precision Errors**: Ensure integer arithmetic throughout
5. **WebSocket Issues**: Verify connection and message format

### Debug Commands
```sql
-- Check positions
SELECT * FROM positions WHERE account_id IN (7,8,9);

-- Check trades
SELECT * FROM trades WHERE account_id IN (7,8,9) ORDER BY timestamp;

-- Check equity history
SELECT * FROM equity_timeseries WHERE account_id IN (7,8,9) ORDER BY timestamp;
```

This specification ensures comprehensive testing of the EVS system and validates that all conversion standards are correctly implemented throughout the entire trade lifecycle.
