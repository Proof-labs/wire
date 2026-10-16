//! The `PartialLiquidationFee` and `FullLiquidationFee` receipts: msgpack
//! round trip, ABCI attributes and chain-coordinate classification.
#![allow(clippy::expect_used, clippy::panic)]
use proof_wire::{abci_event::AbciEventWriter, types::Event};

#[test]
fn penalty_receipt_attributes_and_round_trip_are_exact() {
    let event = Event::PartialLiquidationFee {
        owner: [0xab; 20],
        market: 7,
        pool_id: 2,
        fill_id: 99,
        config_sequence: 8,
        assessed: 5,
        collected: 3,
        waived: 2,
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
    expected.begin_event("partial_liquidation_fee", 8);
    expected.write_attr_hex("owner", &[0xab; 20]);
    expected.write_attr_u64("market", 7);
    expected.write_attr_display("pool_id", &2_u8);
    expected.write_attr_u64("fill_id", 99);
    expected.write_attr_u64("config_sequence", 8);
    expected.write_attr_u64("assessed", 5);
    expected.write_attr_u64("collected", 3);
    expected.write_attr_u64("waived", 2);
    assert_eq!(actual.into_vec(), expected.into_vec());
}

#[test]
fn full_close_penalty_receipt_attributes_and_round_trip_are_exact() {
    let event = Event::FullLiquidationFee {
        owner: [0xcd; 20],
        market: 3,
        pool_id: 1,
        fill_id: 41,
        config_sequence: 2,
        assessed: 7,
        collected: 4,
        waived: 3,
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
    expected.begin_event("full_liquidation_fee", 8);
    expected.write_attr_hex("owner", &[0xcd; 20]);
    expected.write_attr_u64("market", 3);
    expected.write_attr_display("pool_id", &1_u8);
    expected.write_attr_u64("fill_id", 41);
    expected.write_attr_u64("config_sequence", 2);
    expected.write_attr_u64("assessed", 7);
    expected.write_attr_u64("collected", 4);
    expected.write_attr_u64("waived", 3);
    assert_eq!(actual.into_vec(), expected.into_vec());
}
