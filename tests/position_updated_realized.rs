//! `PositionUpdated` names the PnL an update booked to cash without a fill.
//! Bytes written before the field existed decode with nothing booked.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::types::{Event, Side};
use serde::Serialize;

const OWNER: [u8; 20] = [0xB2; 20];

/// The variant as it was encoded before `realized_pnl`.
#[derive(Serialize)]
enum LegacyEvent {
    PositionUpdated {
        owner: [u8; 20],
        market: u32,
        side: Side,
        entry_price: u64,
        size: u64,
    },
}

#[test]
fn a_legacy_update_decodes_with_nothing_booked() {
    let bytes = rmp_serde::to_vec(&LegacyEvent::PositionUpdated {
        owner: OWNER,
        market: 1,
        side: Side::Buy,
        entry_price: 90,
        size: 10,
    })
    .expect("legacy shape encodes");
    let updated: Event = rmp_serde::from_slice(&bytes).expect("legacy bytes decode");
    let Event::PositionUpdated { realized_pnl, .. } = updated else {
        panic!("expected PositionUpdated, got {updated:?}");
    };
    assert_eq!(realized_pnl, 0);
}

#[test]
fn a_realized_gain_round_trips() {
    let event = Event::PositionUpdated {
        owner: OWNER,
        market: 1,
        side: Side::Buy,
        entry_price: 94,
        size: 10,
        realized_pnl: 40,
    };
    let bytes = rmp_serde::to_vec(&event).expect("encodes");
    let decoded: Event = rmp_serde::from_slice(&bytes).expect("decodes");
    let Event::PositionUpdated { realized_pnl, .. } = decoded else {
        panic!("expected PositionUpdated, got {decoded:?}");
    };
    assert_eq!(realized_pnl, 40);
}
