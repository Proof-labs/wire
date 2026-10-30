//! `EventKind` names every `Event` variant exactly as the ABCI encoder does,
//! so consumers can match event types exhaustively instead of keeping string
//! lists.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::HashSet;
use std::str::FromStr;

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::types::{CancelReason, Event, EventKind, UnknownEventKind};

#[test]
fn every_kind_round_trips_through_its_name() {
    let mut seen = HashSet::new();
    for &kind in EventKind::ALL {
        assert!(seen.insert(kind.as_str()), "duplicate name {kind}");
        assert_eq!(EventKind::from_str(kind.as_str()), Ok(kind));
    }
}

#[test]
fn unknown_name_is_refused() {
    assert_eq!(EventKind::from_str("not_an_event"), Err(UnknownEventKind));
    assert_eq!(EventKind::from_str("OrderCancelled"), Err(UnknownEventKind));
}

#[test]
fn kind_matches_the_encoded_event_type() {
    let event = Event::OrderCancelled {
        order_id: 1,
        market: 2,
        owner: [0xA1; 20],
        reason: CancelReason::UserRequested,
        client_order_id: 0,
        original_quantity: 5,
        remaining_quantity: 5,
        filled_quantity: 0,
    };
    let mut writer = AbciEventWriter::new();
    event.encode_abci(&mut writer);
    let bytes = writer.into_vec();
    let len = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let encoded = std::str::from_utf8(&bytes[2..2 + len]).expect("utf8 event type");

    assert_eq!(event.kind(), EventKind::OrderCancelled);
    assert_eq!(EventKind::from(&event), EventKind::OrderCancelled);
    assert_eq!(event.kind().as_str(), encoded);
}
