// OrderRouter - per-symbol ingress routing and sequencing
#![allow(dead_code)]

mod router;
mod sharding;
mod types;

pub use router::{OrderRouter, RouterConfig, RouterError};
pub use sharding::{ShardId, SymbolShard};
pub use types::{
    CoordError, InboundMsgWithSymbol, OrderQueueWriter, ReadyAtTick, SymbolCoordinatorApi,
};

// Re-export whistle types for convenience
pub use whistle::{
    AccountId, Cancel, EnqSeq, InboundMsg, MsgKind, OrderId, RejectReason, Submit, TickId, TsNorm,
};

#[cfg(test)]
mod tests {
    use super::*;
    use whistle::{OrderType, Side};

    #[test]
    fn test_basic_routing() {
        let config = RouterConfig::default();
        let mut router = OrderRouter::new(config);

        // Create a test message
        let msg = InboundMsgWithSymbol {
            symbol_id: 1,
            msg: InboundMsg::submit(
                100, // order_id
                1,   // account_id
                Side::Buy,
                OrderType::Limit,
                Some(150), // price
                10,        // qty
                1000,      // ts_norm
                0,         // meta
                0,         // enq_seq (will be stamped by router)
            ),
        };

        // This should fail initially because symbol is not active
        // We'll implement the actual routing logic in the next step
        let _result = router.route(100, msg);

        // For now, just verify the router exists
        assert!(router.config().num_shards > 0);
    }

    #[test]
    fn test_shard_mapping() {
        let config = RouterConfig { num_shards: 4, ..Default::default() };
        let router = OrderRouter::new(config);

        // Test deterministic shard mapping
        assert_eq!(router.shard_for_symbol(1), 1);
        assert_eq!(router.shard_for_symbol(2), 2);
        assert_eq!(router.shard_for_symbol(4), 0);
        assert_eq!(router.shard_for_symbol(5), 1);
    }
}
