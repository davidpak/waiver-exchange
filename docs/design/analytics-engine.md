> **Implementation Status:** MVP phase. Event ingestion and in-memory aggregation are implemented. Storage is in-memory HashMap (not Parquet as described). Protobuf serialization is not used (plain Rust structs instead). CLI querying works. Parquet storage and SQL query support are planned.

---

---
title: "AnalyticsEngine Design Document"
description: "Comprehensive analytics and observability system for the Waiver Exchange"
status: "Draft"
owner: "Waiver Exchange Team"
audience: "Engineers, Operations, Analytics"
---

# AnalyticsEngine Design Document

## 1. Overview

The `AnalyticsEngine` is a dedicated analytics and observability service that provides comprehensive monitoring, metrics collection, and historical analysis for the Waiver Exchange system. It serves as the central hub for all system analytics, offering a **one-stop shop** for production monitoring, performance analysis, and operational insights.

The AnalyticsEngine operates as an **independent service** that subscribes to events from the `ExecutionManager`, aggregates metrics in real-time, and stores structured data in a columnar format for efficient querying and analysis. It provides both real-time monitoring capabilities and historical trend analysis to support production operations, capacity planning, and system optimization.

### Role in the System

The AnalyticsEngine sits **downstream** of the `ExecutionManager` in the event processing pipeline:

- **Ingest:** Consumes structured analytics events from `ExecutionManager` via protobuf
- **Aggregate:** Processes and aggregates metrics in real-time with configurable sampling
- **Store:** Persists data in Parquet format with tiered retention policies
- **Query:** Provides CLI interface for analytics queries and system monitoring
- **Alert:** Monitors thresholds and sends notifications (future enhancement)

### Core Responsibilities

- **Real-time Metrics Collection:** Capture performance, business, and operational metrics
- **Historical Data Storage:** Maintain time-series data with efficient columnar storage
- **Analytics Interface:** Provide CLI tools for system monitoring and analysis
- **Data Retention Management:** Implement tiered retention policies for cost optimization
- **System Health Monitoring:** Track system health, errors, and performance degradation
- **Capacity Planning Support:** Provide data for scaling and resource planning decisions

## 2. Functional Requirements

### 2.1 Event Ingestion

**Input Sources:**
- `ExecutionManager` analytics events via protobuf
- System health events from all components
- Performance metrics from Whistle engines
- Business metrics from trading activity

**Event Types:**
- **Performance Events:** Tick duration, latency, queue depths, resource utilization
- **Business Events:** Orders processed, trades executed, volume, active users
- **System Health Events:** Crashes, errors, backpressure, memory issues
- **Operational Events:** Symbol lifecycle, thread utilization, I/O metrics

**Sampling Strategy:**
- **Time-based:** Emit events every 1 second (configurable)
- **Tick-based:** Emit every 100 ticks (configurable)
- **Event-driven:** Emit on significant events (crashes, errors, high latency)
- **Adaptive:** Adjust sampling rate based on system load

### 2.2 Data Aggregation

**Real-time Processing:**
- **Streaming aggregation** of metrics within time windows
- **Rolling averages** for performance indicators
- **Percentile calculations** (P50, P95, P99, P99.9) for latency metrics
- **Rate calculations** for throughput metrics

**Aggregation Windows:**
- **1-second windows:** Real-time monitoring
- **1-minute windows:** Short-term trends
- **1-hour windows:** Medium-term analysis
- **1-day windows:** Long-term patterns

### 2.3 Data Storage

**Storage Format:**
- **Parquet files** for columnar storage efficiency
- **Partitioned by time** (hourly/daily partitions)
- **Compressed** for storage optimization
- **Schema evolution** support for future enhancements

**Data Tables:**
- **performance_metrics:** Tick duration, latency, resource usage
- **business_metrics:** Trading activity, volume, user counts
- **system_health:** Errors, crashes, system status
- **operational_metrics:** Symbol lifecycle, thread usage, I/O

**Retention Policy:**
- **Hot data (7 days):** Full resolution, fast access
- **Warm data (30 days):** Aggregated hourly, medium access
- **Cold data (90 days):** Aggregated daily, slower access

### 2.4 Query Interface

**CLI Commands:**
- **System status:** Real-time health and performance overview
- **Performance analysis:** Latency trends, throughput analysis
- **Error investigation:** Error patterns, crash analysis
- **Capacity planning:** Resource utilization trends
- **Historical queries:** Time-range analysis and reporting

**Query Capabilities:**
- **Time-range filtering:** Flexible date/time selection
- **Symbol filtering:** Per-symbol analysis
- **Metric aggregation:** Sum, average, percentiles
- **Export functionality:** CSV, JSON output formats

## 3. Technical Architecture

### 3.1 Component Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ ExecutionManager│───▶│ AnalyticsEngine │───▶│ Parquet Storage │
│ (Event Source)  │    │ (Processing)    │    │ (Data Lake)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                              │                        │
                              ▼                        ▼
                       ┌─────────────────┐    ┌─────────────────┐
                       │ Admin CLI       │    │ Retention       │
                       │ (Query Interface)│    │ Manager         │
                       └─────────────────┘    └─────────────────┘
```

### 3.2 Event Schema

**Protobuf Definition:**
```protobuf
syntax = "proto3";

package waiver.analytics;

message AnalyticsEvent {
  uint64 timestamp_ns = 1;
  uint64 tick_id = 2;
  string symbol = 3;
  EventType event_type = 4;
  
  oneof data {
    PerformanceMetrics performance = 5;
    BusinessMetrics business = 6;
    SystemHealthMetrics health = 7;
    OperationalMetrics operational = 8;
  }
}

enum EventType {
  PERFORMANCE = 0;
  BUSINESS = 1;
  SYSTEM_HEALTH = 2;
  OPERATIONAL = 3;
}

message PerformanceMetrics {
  uint64 tick_duration_ns = 1;
  uint64 event_processing_latency_ns = 2;
  uint32 queue_depth = 3;
  uint64 memory_usage_bytes = 4;
  double cpu_utilization_percent = 5;
  uint32 thread_count = 6;
}

message BusinessMetrics {
  uint32 orders_processed = 1;
  uint32 trades_executed = 2;
  uint64 volume_traded = 3;
  uint32 active_accounts = 4;
  uint32 order_book_depth = 5;
  double average_trade_size = 6;
}

message SystemHealthMetrics {
  bool engine_crashed = 1;
  uint32 queue_overflows = 2;
  uint32 memory_allocation_failures = 3;
  double error_rate_percent = 4;
  uint64 uptime_seconds = 5;
  string error_message = 6;
}

message OperationalMetrics {
  bool symbol_activated = 1;
  bool symbol_evicted = 2;
  double thread_utilization_percent = 3;
  uint64 network_bytes_sent = 4;
  uint64 disk_bytes_written = 5;
  uint32 active_symbols = 6;
}
```

### 3.3 Data Storage Schema

**Performance Metrics Table:**
```sql
CREATE TABLE performance_metrics (
  timestamp TIMESTAMP,
  tick_id BIGINT,
  symbol STRING,
  tick_duration_ns BIGINT,
  event_latency_ns BIGINT,
  queue_depth INT,
  memory_bytes BIGINT,
  cpu_percent DOUBLE,
  thread_count INT
) PARTITIONED BY (date_partition STRING);
```

**Business Metrics Table:**
```sql
CREATE TABLE business_metrics (
  timestamp TIMESTAMP,
  symbol STRING,
  orders_processed INT,
  trades_executed INT,
  volume_traded BIGINT,
  active_accounts INT,
  book_depth INT,
  avg_trade_size DOUBLE
) PARTITIONED BY (date_partition STRING);
```

**System Health Table:**
```sql
CREATE TABLE system_health (
  timestamp TIMESTAMP,
  component STRING,
  engine_crashed BOOLEAN,
  queue_overflows INT,
  memory_failures INT,
  error_rate DOUBLE,
  uptime_seconds BIGINT,
  error_message STRING
) PARTITIONED BY (date_partition STRING);
```

**Operational Metrics Table:**
```sql
CREATE TABLE operational_metrics (
  timestamp TIMESTAMP,
  event_type STRING,
  symbol_activated BOOLEAN,
  symbol_evicted BOOLEAN,
  thread_utilization DOUBLE,
  network_bytes BIGINT,
  disk_bytes BIGINT,
  active_symbols INT
) PARTITIONED BY (date_partition STRING);
```

### 3.4 Processing Pipeline

**Event Processing Flow:**
1. **Ingest:** Receive protobuf events from ExecutionManager
2. **Validate:** Check event schema and data integrity
3. **Aggregate:** Process metrics within time windows
4. **Store:** Write aggregated data to Parquet files
5. **Index:** Update query indexes for fast access

**Aggregation Logic:**
- **Streaming aggregation** using sliding windows
- **Rolling calculations** for moving averages
- **Percentile estimation** using t-digest algorithm
- **Rate calculations** for throughput metrics

## 4. Performance Requirements

### 4.1 Latency Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Event processing latency | < 1ms | End-to-end event processing |
| Query response time | < 100ms | Simple queries (last hour) |
| Query response time | < 1s | Complex queries (last day) |
| Query response time | < 10s | Historical queries (last month) |

### 4.2 Throughput Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Event ingestion rate | 100K events/sec | Peak processing capacity |
| Storage write rate | 1GB/hour | Parquet file writing |
| Query throughput | 100 queries/sec | Concurrent query handling |

### 4.3 Resource Requirements

| Resource | Requirement | Notes |
|----------|-------------|-------|
| CPU | 2-4 cores | Event processing and aggregation |
| Memory | 4-8GB | Streaming buffers and caches |
| Storage | 100GB-1TB | Depends on retention policy |
| Network | 100Mbps | Event ingestion bandwidth |

## 5. Operational Requirements

### 5.1 Monitoring and Observability

**Self-Monitoring:**
- **Processing latency** of analytics events
- **Storage utilization** and growth rates
- **Query performance** and response times
- **Error rates** and failure modes

**Health Checks:**
- **Event ingestion** health and backlog
- **Storage system** health and capacity
- **Query service** availability and performance
- **Data freshness** and staleness detection

### 5.2 Configuration Management

**Configuration Options:**
- **Sampling rates** (time-based and tick-based)
- **Retention policies** (hot, warm, cold data)
- **Storage paths** and partitioning strategy
- **Query timeouts** and resource limits

**Configuration Sources:**
- **Environment variables** for deployment settings
- **Configuration files** for operational settings
- **Runtime configuration** for dynamic adjustments

### 5.3 Error Handling and Recovery

**Error Categories:**
- **Ingestion errors:** Malformed events, schema violations
- **Processing errors:** Aggregation failures, memory issues
- **Storage errors:** Disk full, permission issues
- **Query errors:** Timeout, resource exhaustion

**Recovery Strategies:**
- **Graceful degradation** for non-critical failures
- **Retry mechanisms** for transient errors
- **Circuit breakers** for downstream failures
- **Data consistency** checks and repair

## 6. Implementation Plan

### 6.1 Phase 1: Core AnalyticsEngine (MVP)

**Components:**
- **Event ingestion** from ExecutionManager
- **Basic aggregation** logic
- **Parquet storage** implementation
- **Simple CLI** interface

**Deliverables:**
- AnalyticsEngine service
- Protobuf event definitions
- Basic CLI commands
- Storage schema implementation

### 6.2 Phase 2: Enhanced Features

**Components:**
- **Advanced aggregation** (percentiles, rolling averages)
- **Tiered retention** management
- **Rich CLI** interface with charts
- **Export functionality**

**Deliverables:**
- Enhanced query capabilities
- Data retention automation
- Improved CLI experience
- Performance optimizations

### 6.3 Phase 3: Advanced Analytics

**Components:**
- **Alerting system** with email notifications
- **Web interface** (optional)
- **Machine learning** insights
- **Advanced visualizations**

**Deliverables:**
- Alerting infrastructure
- Web dashboard (if needed)
- ML-based anomaly detection
- Advanced analytics features

## 7. Testing Strategy

### 7.1 Unit Testing

**Test Coverage:**
- **Event processing** logic
- **Aggregation algorithms** correctness
- **Storage operations** reliability
- **Query functionality** accuracy

### 7.2 Integration Testing

**Test Scenarios:**
- **End-to-end** event processing pipeline
- **Storage and retrieval** operations
- **CLI interface** functionality
- **Performance** under load

### 7.3 Performance Testing

**Load Testing:**
- **High-volume** event ingestion
- **Concurrent query** processing
- **Storage system** performance
- **Memory usage** under load

## 8. Deployment and Operations

### 8.1 Deployment Model

**Service Architecture:**
- **Independent service** deployment
- **Containerized** using Docker
- **Kubernetes** orchestration (optional)
- **Health check** endpoints

### 8.2 Operational Procedures

**Monitoring:**
- **Service health** monitoring
- **Performance metrics** tracking
- **Storage utilization** monitoring
- **Query performance** analysis

**Maintenance:**
- **Data retention** cleanup
- **Storage optimization** (compaction)
- **Performance tuning** based on usage
- **Backup and recovery** procedures

## 9. Future Enhancements

### 9.1 Advanced Analytics

**Machine Learning:**
- **Anomaly detection** for system health
- **Predictive analytics** for capacity planning
- **Pattern recognition** for optimization
- **Automated insights** generation

### 9.2 Enhanced Interfaces

**Web Dashboard:**
- **Real-time** monitoring dashboards
- **Interactive** charts and visualizations
- **Customizable** views and alerts
- **Mobile-responsive** design

### 9.3 Integration Capabilities

**External Systems:**
- **Grafana** integration for visualization
- **Prometheus** metrics export
- **Slack/Discord** alerting
- **API endpoints** for external access

---

## 10. Conclusion

The AnalyticsEngine provides a comprehensive solution for monitoring, analyzing, and understanding the Waiver Exchange system's performance and behavior. By implementing a dedicated analytics service with efficient storage and powerful query capabilities, we enable data-driven decision making, proactive system monitoring, and continuous optimization of the trading platform.

The phased implementation approach ensures we deliver core functionality quickly while building a foundation for advanced analytics capabilities. The independent service architecture provides flexibility and scalability while maintaining system reliability and performance.

This design establishes the AnalyticsEngine as the central hub for all system analytics, providing the **one-stop shop** for production monitoring and operational insights that enables effective system management and optimization.
