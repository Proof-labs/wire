//! `PositionLiquidated` and `LiquidationFailed`: msgpack round trip, ABCI
//! attributes, chain-coordinate classification and the enum encodings.
#![allow(clippy::expect_used, clippy::panic)]
use std::collections::BTreeSet;

use proof_wire::{
    abci_event::AbciEventWriter,
    liquidation_report::{BindingOutcome, LiquidationMode, LiquidationRefusal},
    query::BindingScenario,
    types::{Branch, Event, EventId, Side},
};

fn round_trips(event: &Event) {
    let bytes = rmp_serde::to_vec(event).expect("event encodes");
    let decoded: Event = rmp_serde::from_slice(&bytes).expect("event decodes");
    assert_eq!(
        rmp_serde::to_vec(&decoded).expect("event re-encodes"),
        bytes
    );
    assert!(event.requires_trigger_coordinates());
}

#[test]
fn position_liquidated_attributes_and_round_trip_are_exact() {
    let event = Event::PositionLiquidated {
        owner: [0xab; 20],
        market: 7,
        side: Side::Sell,
        mode: LiquidationMode::Partial,
        size_before: 100,
        size_closed: 12,
        size_after: 88,
        penalty_collected: 9,
        penalty_waived: 3,
        equity: -5,
        maintenance: 500,
        binding_outcome: BindingOutcome(vec![
            BindingScenario {
                event_id: EventId(4),
                branch: Branch::Yes,
            },
            BindingScenario {
                event_id: EventId(9),
                branch: Branch::No,
            },
        ]),
        config_sequence: 2,
    };
    round_trips(&event);

    let mut actual = AbciEventWriter::new();
    event.encode_abci(&mut actual);
    let mut expected = AbciEventWriter::new();
    expected.begin_event("position_liquidated", 13);
    expected.write_attr_hex("owner", &[0xab; 20]);
    expected.write_attr_u64("market", 7);
    expected.write_attr("side", "sell");
    expected.write_attr("mode", "partial");
    expected.write_attr_u64("size_before", 100);
    expected.write_attr_u64("size_closed", 12);
    expected.write_attr_u64("size_after", 88);
    expected.write_attr_u64("penalty_collected", 9);
    expected.write_attr_u64("penalty_waived", 3);
    expected.write_attr("equity", "-5");
    expected.write_attr_u64("maintenance", 500);
    expected.write_attr("binding_outcome", "4:yes,9:no");
    expected.write_attr_u64("config_sequence", 2);
    assert_eq!(actual.into_vec(), expected.into_vec());
}

#[test]
fn liquidation_failed_attributes_and_round_trip_are_exact() {
    for (refusal, rendered) in [
        (
            Some(LiquidationRefusal::ReferenceUnavailable),
            "reference_unavailable",
        ),
        (None, ""),
    ] {
        let event = Event::LiquidationFailed {
            owner: [0xcd; 20],
            code: 111,
            refusal,
        };
        round_trips(&event);

        let mut actual = AbciEventWriter::new();
        event.encode_abci(&mut actual);
        let mut expected = AbciEventWriter::new();
        expected.begin_event("liquidation_failed", 3);
        expected.write_attr_hex("owner", &[0xcd; 20]);
        expected.write_attr_u64("code", 111);
        expected.write_attr("refusal", rendered);
        assert_eq!(actual.into_vec(), expected.into_vec());
    }
}

#[test]
fn every_mode_and_refusal_encodes_as_its_name_under_a_distinct_value() {
    let mut values = BTreeSet::new();
    for mode in LiquidationMode::ALL {
        let bytes = rmp_serde::to_vec(&mode).expect("mode encodes");
        assert_eq!(
            bytes,
            rmp_serde::to_vec(&format!("{mode:?}")).expect("name encodes")
        );
        assert_eq!(mode.to_string(), mode.as_str());
        assert!(values.insert(mode.as_str()));
    }
    let mut values = BTreeSet::new();
    for refusal in LiquidationRefusal::ALL {
        let bytes = rmp_serde::to_vec(&refusal).expect("refusal encodes");
        assert_eq!(
            bytes,
            rmp_serde::to_vec(&format!("{refusal:?}")).expect("name encodes")
        );
        let decoded: LiquidationRefusal = rmp_serde::from_slice(&bytes).expect("refusal decodes");
        assert_eq!(decoded, refusal);
        assert!(values.insert(refusal.as_str()));
    }
}
