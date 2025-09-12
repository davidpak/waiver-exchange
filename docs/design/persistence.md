# Persistence Layer Design

## 1. Overview

The persistence layer provides durable storage for the trading system without using traditional databases. It uses a **Write-Ahead Log (WAL) + Snapshots** approach to ensure all trading data survives system restarts while maintaining the high-performance, in-memory order book architecture.

## 2. Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Trading       │    │   Persistence   │    │   Storage       │
│   System        │    │   Layer         │    │   Backend       │
│                 │    │                 │    │                 │
│  • Order Books  │───▶│  • WAL Writer   │───▶│  • Local Files  │
│  • Market Data  │    │  • Snapshot     │    │  • S3 Cloud     │
│  • User State   │    │  • Recovery     │    │  • Glacier      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 3. Core Components

### 3.1 Write-Ahead Log (WAL)

**Purpose**: Log every trading event before it's applied to memory

**Benefits**:
- **Durability**: No data loss even during crashes
- **Audit Trail**: Complete history of all trading activity
- **Recovery**: Can rebuild exact system state
- **Performance**: Sequential writes are very fast

**WAL Entry Format**:
```json
{
  "timestamp": 1640995200000,
  "tick": 12345,
  "event_type": "order_submitted",
  "symbol_id": 1,
  "order_id": "ord_123",
  "account_id": "user_456",
  "side": "BUY",
  "price": 150,
  "quantity": 10,
  "order_type": "LIMIT",
  "execution_id": "exec_789"
}
```

### 3.2 Snapshots

**Purpose**: Periodic "photos" of the entire system state

**Benefits**:
- **Fast Recovery**: Start from snapshot instead of replaying all WAL
- **Storage Efficiency**: Compress historical data
- **Point-in-Time Recovery**: Restore to specific moments

**Snapshot Format**:
```json
{
  "snapshot_id": "snap_001",
  "timestamp": 1640995200000,
  "tick": 12345,
  "order_books": {
    "1": {
      "bids": [[150, 10], [149, 5]],
      "asks": [[151, 8], [152, 12]],
      "last_trade": {"price": 150, "qty": 5}
    }
  },
  "user_positions": {
    "user_456": {"symbol_1": 100, "symbol_2": -50}
  },
  "system_state": {
    "current_tick": 12345,
    "active_symbols": [1, 2, 3, 4, 5]
  }
}
```

### 3.3 Recovery Manager

**Purpose**: Restore system state from WAL + snapshots

**Recovery Process**:
1. **Load Latest Snapshot** - Restore base state
2. **Replay WAL** - Apply all events since snapshot
3. **Validate State** - Ensure data integrity
4. **Resume Trading** - System ready for new orders

## 4. Storage Strategy

### 4.1 Local Storage (Development & Production)

**File Structure**:
```
/data/
├── wal/
│   ├── wal_000001.log    # Active WAL entries
│   ├── wal_000002.log    # Recent WAL entries
│   └── wal_000003.log    # Recent WAL entries
├── snapshots/
│   ├── snap_000001.json  # Full system state
│   └── snap_000002.json  # Previous snapshot
└── config/
    └── persistence.json  # Persistence configuration
```

**Benefits**:
- **Zero Latency**: Local writes are microseconds
- **Simple Setup**: No external dependencies
- **Cost Effective**: Only local disk space
- **Fast Recovery**: Direct file access

### 4.2 Cloud Storage (Production)

**Hybrid Approach**:
- **Local WAL**: All writes go to local disk immediately
- **Cloud Backup**: Background upload to S3 every 5 minutes
- **Tiered Storage**: S3 → Glacier for long-term retention

**Storage Tiers**:
```
Local Disk (Active) → S3 Standard (30 days) → Glacier (7 years) → Delete
```

**Cost Analysis**:
```
7 Year Retention:
- WAL Files: ~252GB × $0.023/GB = $5.80/year
- Snapshots: ~25GB × $0.023/GB = $0.58/year
- Total: ~$6.38/year
```

## 5. Implementation Design

### 5.1 Abstract Persistence Interface

```rust
pub trait PersistenceBackend {
    async fn write_event(&mut self, event: &TradingEvent) -> Result<()>;
    async fn take_snapshot(&self, state: &SystemState) -> Result<()>;
    async fn load_snapshot(&self) -> Result<Option<SystemState>>;
    async fn replay_wal(&self, from_tick: TickId) -> Result<Vec<TradingEvent>>;
    async fn cleanup_old_files(&self) -> Result<()>;
}
```

### 5.2 Local Persistence Implementation

```rust
pub struct LocalPersistence {
    wal_dir: PathBuf,
    snapshot_dir: PathBuf,
    retention_policy: RetentionPolicy,
    current_wal_file: File,
    wal_file_counter: usize,
}

impl PersistenceBackend for LocalPersistence {
    async fn write_event(&mut self, event: &TradingEvent) -> Result<()> {
        // Serialize event to JSON
        let json = serde_json::to_string(event)?;
        
        // Write to current WAL file
        writeln!(self.current_wal_file, "{}", json)?;
        self.current_wal_file.sync_all()?;  // Force to disk
        
        // Rotate WAL file if needed
        if self.should_rotate_wal() {
            self.rotate_wal_file().await?;
        }
        
        Ok(())
    }
    
    async fn take_snapshot(&self, state: &SystemState) -> Result<()> {
        let snapshot = Snapshot {
            timestamp: SystemTime::now(),
            tick: state.get_current_tick(),
            order_books: state.get_all_order_books(),
            user_positions: state.get_all_positions(),
            system_state: state.get_system_state(),
        };
        
        // Save to file with atomic write
        let filename = format!("snapshots/snap_{:06}.json", self.snapshot_counter);
        let temp_filename = format!("{}.tmp", filename);
        
        let file = File::create(&temp_filename)?;
        serde_json::to_writer(file, &snapshot)?;
        
        // Atomic rename
        std::fs::rename(&temp_filename, &filename)?;
        
        Ok(())
    }
}
```

### 5.3 Cloud Persistence Implementation

```rust
pub struct CloudPersistence {
    local_backend: LocalPersistence,  // Still use local for speed
    s3_client: S3Client,
    upload_scheduler: UploadScheduler,
    retention_manager: RetentionManager,
}

impl CloudPersistence {
    pub async fn start_background_upload(&self) {
        tokio::spawn(async move {
            loop {
                // Upload completed WAL files every 5 minutes
                self.upload_completed_files().await;
                tokio::time::sleep(Duration::from_secs(300)).await;
            }
        });
    }
    
    async fn upload_completed_files(&self) -> Result<()> {
        // Find completed WAL files (not currently being written to)
        let completed_files = self.find_completed_wal_files().await?;
        
        for file_path in completed_files {
            // Upload to S3
            self.upload_to_s3(&file_path).await?;
            
            // Optionally delete local file after successful upload
            if self.config.delete_after_upload {
                std::fs::remove_file(&file_path)?;
            }
        }
        
        Ok(())
    }
}
```

## 6. Configuration

### 6.1 Persistence Configuration

```toml
[persistence]
backend = "local"  # "local" or "cloud"

[persistence.local]
wal_dir = "./data/wal"
snapshot_dir = "./data/snapshots"
max_wal_files = 100
max_snapshots = 10
snapshot_interval = 1000  # Every 1000 orders
cleanup_interval = 3600   # Every hour

[persistence.cloud]
s3_bucket = "waiver-exchange-wal"
region = "us-east-1"
upload_interval = 300     # 5 minutes
retention_days = 2555     # 7 years
delete_after_upload = false  # Keep local copies

[persistence.retention]
local_wal_files = 100
local_snapshots = 10
s3_standard_days = 30
glacier_days = 2555
delete_after_days = 2555
```

### 6.2 Retention Policy

```rust
pub struct RetentionPolicy {
    pub max_wal_files: usize,        // Keep 100 WAL files locally
    pub max_snapshots: usize,        // Keep 10 snapshots locally
    pub snapshot_interval: usize,    // Take snapshot every 1000 orders
    pub cleanup_interval: Duration,  // Cleanup every hour
    pub s3_retention_days: u32,      // Keep 30 days in S3
    pub glacier_retention_days: u32, // Keep 7 years in Glacier
}
```

## 7. Performance Characteristics

### 7.1 Write Performance

**Local WAL Writes**:
- **Latency**: < 1ms per event
- **Throughput**: 100,000+ events/second
- **Durability**: Immediate fsync to disk

**Snapshot Creation**:
- **Frequency**: Every 1000 orders
- **Duration**: < 100ms for full system snapshot
- **Size**: ~10MB per snapshot

### 7.2 Recovery Performance

**From Snapshot + WAL**:
- **Snapshot Load**: < 1 second
- **WAL Replay**: ~1000 events/second
- **Total Recovery**: < 5 minutes for 1M events

**From WAL Only**:
- **Full Replay**: ~1000 events/second
- **Total Recovery**: ~17 minutes for 1M events

## 8. Data Integrity

### 8.1 Atomic Operations

**WAL Writes**:
- Write to temporary file
- Atomic rename to final filename
- fsync to ensure disk persistence

**Snapshot Creation**:
- Write to temporary file
- Atomic rename to final filename
- Verify file integrity

### 8.2 Consistency Checks

**WAL Validation**:
- JSON format validation
- Event sequence validation
- Checksum verification

**Snapshot Validation**:
- JSON format validation
- State consistency checks
- Cross-reference with WAL

## 9. Monitoring and Observability

### 9.1 Metrics

- **WAL write latency** - P50, P95, P99
- **Snapshot creation time** - Duration and frequency
- **Recovery time** - Time to restore from crash
- **Storage usage** - Disk space consumption
- **File counts** - WAL files and snapshots

### 9.2 Health Checks

- **WAL file integrity** - Verify all files are readable
- **Snapshot validity** - Ensure snapshots can be loaded
- **Storage space** - Monitor disk usage
- **Recovery capability** - Test recovery process

### 9.3 Alerts

- **WAL write failures** - Immediate alert
- **Snapshot failures** - Alert within 5 minutes
- **Storage space low** - Alert at 80% usage
- **Recovery failures** - Immediate alert

## 10. Testing Strategy

### 10.1 Unit Tests

- **WAL write/read operations**
- **Snapshot creation/loading**
- **Event serialization/deserialization**
- **Retention policy enforcement**

### 10.2 Integration Tests

- **End-to-end persistence flow**
- **Recovery from various failure scenarios**
- **File rotation and cleanup**
- **Configuration changes**

### 10.3 Stress Tests

- **High-frequency event writing**
- **Large snapshot creation**
- **Recovery under load**
- **Storage space exhaustion**

### 10.4 Chaos Tests

- **Disk full scenarios**
- **File corruption recovery**
- **Network failures during cloud upload**
- **Concurrent access issues**

## 11. Migration Strategy

### 11.1 Local to Cloud Migration

**Zero-Downtime Migration**:
1. Deploy with cloud persistence enabled
2. Background upload starts immediately
3. Local files remain for fast access
4. Gradual cleanup of old local files

**Configuration Change**:
```toml
# Change this line in config
backend = "cloud"  # Was "local"
```

### 11.2 Data Migration

**Automatic Migration**:
- Existing WAL files uploaded to S3
- Snapshots migrated to cloud storage
- Retention policies applied automatically
- No manual intervention required

## 12. Security Considerations

### 12.1 Data Protection

- **Encryption at rest** - S3 server-side encryption
- **Encryption in transit** - HTTPS for cloud uploads
- **Access controls** - IAM roles for S3 access
- **Audit logging** - All access logged

### 12.2 Compliance

- **Data retention** - 7-year retention for audit
- **Immutable logs** - WAL files cannot be modified
- **Audit trail** - Complete trading history
- **Recovery procedures** - Documented recovery process

## 13. Future Enhancements

### 13.1 Performance Optimizations

- **Compression** - Compress WAL files and snapshots
- **Parallel uploads** - Multiple concurrent S3 uploads
- **Incremental snapshots** - Only save changed data
- **Memory-mapped files** - Faster WAL access

### 13.2 Advanced Features

- **Point-in-time recovery** - Restore to specific timestamps
- **Cross-region replication** - Multi-region backup
- **Encryption key rotation** - Automatic key management
- **Compliance reporting** - Automated audit reports

---

This persistence layer design provides a robust, high-performance storage solution that maintains the speed of in-memory trading while ensuring complete data durability and recovery capabilities.
