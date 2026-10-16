//! The `LiquidationProgressReported` receipt: msgpack round trip, ABCI
//! attributes, chain-coordinate classification and the outcome encoding.
#![allow(clippy::expect_used, clippy::panic)]
use std::collections::BTreeSet;

use proof_wire::{
    abci_event::AbciEventWriter, liquidation_progress::LiquidationProgress, types::Event,
};

#[test]
fn progress_receipt_attributes_and_round_trip_are_exact() {
    let event = Event::LiquidationProgressReported {
        owner: [0xab; 20],
        outcome: LiquidationProgress::MaintenanceRestoredBelowTarget,
        config_sequence: 4,
    };
    let bytes = rmp_serde::to_vec(&event).expect("receipt encodes");
    let decoded: Event = rmp_serde::from_slice(&bytes).expect("receipt decodes");
    assert_eq!(
        rmp_serde::to_vec(&decoded).expect("receipt re-encodes"),
        bytes
    );
    assert!(event.requires_trigger_coordinates());

    let mut actual = AbciEventWriter::new();
    event.encode_abci(&mut actual);
    let mut expected = AbciEventWriter::new();
    expected.begin_event("liquidation_progress_reported", 3);
    expected.write_attr_hex("owner", &[0xab; 20]);
    expected.write_attr("outcome", "maintenance_restored_below_target");
    expected.write_attr_u64("config_sequence", 4);
    assert_eq!(actual.into_vec(), expected.into_vec());
}

#[test]
fn every_outcome_round_trips_under_a_distinct_attribute_value() {
    let mut values = BTreeSet::new();
    for outcome in LiquidationProgress::ALL {
        let bytes = rmp_serde::to_vec(&outcome).expect("outcome encodes");
        assert_eq!(
            bytes,
            rmp_serde::to_vec(&format!("{outcome:?}")).expect("name encodes"),
            "an outcome encodes as its variant name"
        );
        let decoded: LiquidationProgress = rmp_serde::from_slice(&bytes).expect("outcome decodes");
        assert_eq!(decoded, outcome);
        assert_eq!(outcome.to_string(), outcome.as_str());
        assert!(
            values.insert(outcome.as_str()),
            "attribute value {} is used twice",
            outcome.as_str()
        );
    }
}
