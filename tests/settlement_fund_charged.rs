//! The settlement events name what the insurance fund paid for the owner.
//! Bytes written before the field existed decode with nothing charged.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::types::{Event, EventId, Side};
use serde::Serialize;

const OWNER: [u8; 20] = [0xB2; 20];

/// The variants as they were encoded before `fund_charged`.
#[derive(Serialize)]
enum LegacyEvent {
    PredictionSettled {
        event_id: EventId,
        market: u32,
        owner: [u8; 20],
        side: Side,
        size: u64,
        payoff_per_share: u64,
        cash_delta: i64,
    },
    ConditionalVoidSettled {
        event_id: EventId,
        market: u32,
        owner: [u8; 20],
        side: Side,
        size: u64,
        entry_price: u64,
        settlement_price: u64,
        branch_weight: u64,
        realized_pnl: i64,
    },
}

fn fund_charged(event: &Event) -> u64 {
    match event {
        Event::PredictionSettled { fund_charged, .. }
        | Event::ConditionMet { fund_charged, .. }
        | Event::ConditionalVoidSettled { fund_charged, .. } => *fund_charged,
        other => panic!("not a settlement event: {other:?}"),
    }
}

#[test]
fn legacy_settlements_decode_with_nothing_charged() {
    let legacy = [
        LegacyEvent::PredictionSettled {
            event_id: EventId(42),
            market: 102,
            owner: OWNER,
            side: Side::Buy,
            size: 100,
            payoff_per_share: 0,
            cash_delta: -400,
        },
        LegacyEvent::ConditionalVoidSettled {
            event_id: EventId(42),
            market: 100,
            owner: OWNER,
            side: Side::Sell,
            size: 10,
            entry_price: 95,
            settlement_price: 100,
            branch_weight: 400_000,
            realized_pnl: -20,
        },
    ];
    for event in legacy {
        let bytes = rmp_serde::to_vec(&event).expect("legacy shape encodes");
        let decoded: Event = rmp_serde::from_slice(&bytes).expect("legacy bytes decode");
        assert_eq!(fund_charged(&decoded), 0);
    }
}

#[test]
fn a_fund_charge_round_trips() {
    let events = [
        Event::PredictionSettled {
            event_id: EventId(42),
            market: 102,
            owner: OWNER,
            side: Side::Buy,
            size: 100,
            payoff_per_share: 0,
            cash_delta: -400,
            fund_charged: 300,
        },
        Event::ConditionMet {
            event_id: EventId(42),
            market: 100,
            owner: OWNER,
            side: Side::Sell,
            size: 10,
            entry_price: 95,
            settlement_price: 100,
            realized_pnl: -50,
            converted_size: 0,
            fallback_reason: None,
            fund_charged: 300,
        },
        Event::ConditionalVoidSettled {
            event_id: EventId(42),
            market: 100,
            owner: OWNER,
            side: Side::Sell,
            size: 10,
            entry_price: 95,
            settlement_price: 100,
            branch_weight: 400_000,
            realized_pnl: -20,
            fund_charged: 300,
        },
    ];
    for event in events {
        let bytes = rmp_serde::to_vec(&event).expect("encodes");
        let decoded: Event = rmp_serde::from_slice(&bytes).expect("decodes");
        assert_eq!(fund_charged(&decoded), 300);
    }
}
