//! `BinaryIssuedFromMTM` names the conditional value under one binary unit
//! that it does not issue. Bytes written before the field existed decode with
//! nothing dropped.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::types::Event;
use serde::Serialize;

const OWNER: [u8; 20] = [0xB1; 20];

/// The variant as it was encoded before `value_dropped`.
#[derive(Serialize)]
enum LegacyEvent {
    BinaryIssuedFromMTM {
        owner: [u8; 20],
        cp_market: u32,
        binary_market: u32,
        signed_delta: i64,
        final_size: u64,
        final_entry: u64,
        cash_pnl_realized: i64,
    },
}

#[test]
fn a_legacy_issue_decodes_with_nothing_dropped() {
    let bytes = rmp_serde::to_vec(&LegacyEvent::BinaryIssuedFromMTM {
        owner: OWNER,
        cp_market: 100,
        binary_market: 102,
        signed_delta: 10_000,
        final_size: 1,
        final_entry: 0,
        cash_pnl_realized: 0,
    })
    .expect("legacy shape encodes");
    let issued: Event = rmp_serde::from_slice(&bytes).expect("legacy bytes decode");
    let Event::BinaryIssuedFromMTM { value_dropped, .. } = issued else {
        panic!("expected BinaryIssuedFromMTM, got {issued:?}");
    };
    assert_eq!(value_dropped, 0);
}

#[test]
fn a_dropped_value_round_trips() {
    let event = Event::BinaryIssuedFromMTM {
        owner: OWNER,
        cp_market: 100,
        binary_market: 102,
        signed_delta: -52_500_021,
        final_size: 5_250,
        final_entry: 0,
        cash_pnl_realized: 0,
        value_dropped: -21,
    };
    let bytes = rmp_serde::to_vec(&event).expect("encodes");
    let decoded: Event = rmp_serde::from_slice(&bytes).expect("decodes");
    let Event::BinaryIssuedFromMTM { value_dropped, .. } = decoded else {
        panic!("expected BinaryIssuedFromMTM");
    };
    assert_eq!(value_dropped, -21);
}
