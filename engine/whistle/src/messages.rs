#![allow(dead_code)]

use crate::{AccountId, EnqSeq, OrderId, OrderType, Price, Qty, Side, TsNorm};

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MsgKind {
    Submit = 0,
    Cancel = 1,
}

#[derive(Debug, Clone, Copy)]
pub struct Submit {
    pub order_id: OrderId,
    pub account_id: AccountId,
    pub side: Side,
    pub typ: OrderType,
    pub price: Option<Price>,
    pub qty: Qty,
    pub ts_norm: TsNorm,
    pub meta: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Cancel {
    pub order_id: OrderId,
    pub ts_norm: TsNorm,
}

/// Inbound message from OrderRouter via SPSC queue
#[derive(Debug, Clone)]
pub struct InboundMsg {
    pub kind: MsgKind,
    pub submit: Option<Submit>,
    pub cancel: Option<Cancel>,
    pub enq_seq: EnqSeq,
}

impl InboundMsg {
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        order_id: OrderId,
        account_id: AccountId,
        side: Side,
        typ: OrderType,
        price: Option<Price>,
        qty: Qty,
        ts_norm: TsNorm,
        meta: u64,
        enq_seq: EnqSeq,
    ) -> Self {
        Self {
            kind: MsgKind::Submit,
            submit: Some(Submit { order_id, account_id, side, typ, price, qty, ts_norm, meta }),
            cancel: None,
            enq_seq,
        }
    }

    pub fn submit_builder() -> SubmitBuilder {
        SubmitBuilder::default()
    }

    pub fn cancel(order_id: OrderId, ts_norm: TsNorm, enq_seq: EnqSeq) -> Self {
        Self {
            kind: MsgKind::Cancel,
            submit: None,
            cancel: Some(Cancel { order_id, ts_norm }),
            enq_seq,
        }
    }

    pub fn order_id(&self) -> OrderId {
        match self.kind {
            MsgKind::Submit => self.submit.as_ref().unwrap().order_id,
            MsgKind::Cancel => self.cancel.as_ref().unwrap().order_id,
        }
    }

    pub fn ts_norm(&self) -> TsNorm {
        match self.kind {
            MsgKind::Submit => self.submit.as_ref().unwrap().ts_norm,
            MsgKind::Cancel => self.cancel.as_ref().unwrap().ts_norm,
        }
    }

    pub fn priority_key(&self) -> (TsNorm, EnqSeq) {
        (self.ts_norm(), self.enq_seq)
    }
}

/// Builder for creating Submit messages with many parameters
#[derive(Debug, Default)]
pub struct SubmitBuilder {
    order_id: Option<OrderId>,
    account_id: Option<AccountId>,
    side: Option<Side>,
    typ: Option<OrderType>,
    price: Option<Price>,
    qty: Option<Qty>,
    ts_norm: Option<TsNorm>,
    meta: Option<u64>,
    enq_seq: Option<EnqSeq>,
}

impl SubmitBuilder {
    pub fn order_id(mut self, order_id: OrderId) -> Self {
        self.order_id = Some(order_id);
        self
    }

    pub fn account_id(mut self, account_id: AccountId) -> Self {
        self.account_id = Some(account_id);
        self
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    pub fn typ(mut self, typ: OrderType) -> Self {
        self.typ = Some(typ);
        self
    }

    pub fn price(mut self, price: Option<Price>) -> Self {
        self.price = price;
        self
    }

    pub fn qty(mut self, qty: Qty) -> Self {
        self.qty = Some(qty);
        self
    }

    pub fn ts_norm(mut self, ts_norm: TsNorm) -> Self {
        self.ts_norm = Some(ts_norm);
        self
    }

    pub fn meta(mut self, meta: u64) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn enq_seq(mut self, enq_seq: EnqSeq) -> Self {
        self.enq_seq = Some(enq_seq);
        self
    }

    pub fn build(self) -> Result<InboundMsg, &'static str> {
        let order_id = self.order_id.ok_or("order_id is required")?;
        let account_id = self.account_id.ok_or("account_id is required")?;
        let side = self.side.ok_or("side is required")?;
        let typ = self.typ.ok_or("typ is required")?;
        let qty = self.qty.ok_or("qty is required")?;
        let ts_norm = self.ts_norm.ok_or("ts_norm is required")?;
        let meta = self.meta.ok_or("meta is required")?;
        let enq_seq = self.enq_seq.ok_or("enq_seq is required")?;

        Ok(InboundMsg {
            kind: MsgKind::Submit,
            submit: Some(Submit {
                order_id,
                account_id,
                side,
                typ,
                price: self.price,
                qty,
                ts_norm,
                meta,
            }),
            cancel: None,
            enq_seq,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_message_creation() {
        let msg =
            InboundMsg::submit(123, 456, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);

        assert_eq!(msg.kind, MsgKind::Submit);
        assert_eq!(msg.order_id(), 123);
        assert_eq!(msg.ts_norm(), 1000);
        assert_eq!(msg.enq_seq, 1);
        assert!(msg.submit.is_some());
        assert!(msg.cancel.is_none());

        let submit = msg.submit.unwrap();
        assert_eq!(submit.order_id, 123);
        assert_eq!(submit.account_id, 456);
        assert_eq!(submit.side, Side::Buy);
        assert_eq!(submit.typ, OrderType::Limit);
        assert_eq!(submit.price, Some(150));
        assert_eq!(submit.qty, 10);
    }

    #[test]
    fn cancel_message_creation() {
        let msg = InboundMsg::cancel(123, 1000, 1);

        assert_eq!(msg.kind, MsgKind::Cancel);
        assert_eq!(msg.order_id(), 123);
        assert_eq!(msg.ts_norm(), 1000);
        assert_eq!(msg.enq_seq, 1);
        assert!(msg.submit.is_none());
        assert!(msg.cancel.is_some());
    }

    #[test]
    fn priority_key_ordering() {
        let msg1 = InboundMsg::submit(1, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 1);
        let msg2 = InboundMsg::submit(2, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1000, 0, 2);
        let msg3 = InboundMsg::submit(3, 1, Side::Buy, OrderType::Limit, Some(150), 10, 1001, 0, 1);

        assert!(msg1.priority_key() < msg2.priority_key());

        assert!(msg1.priority_key() < msg3.priority_key());

        assert!(msg2.priority_key() < msg3.priority_key());
    }
}
