//! `SetLiquidationConfig` (inner admin tag 0x11): positional bytes, the
//! canonical fixed point the proposal content hash commits, the validation
//! bounds, and the `LiquidationConfigSet` ABCI attributes.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::codec::{canonical_admin_action_bytes, canonicalize_admin_action};
use proof_wire::liquidation_config::*;
use proof_wire::types::{AdminAction, AdminActionType, Event};

fn config() -> LiquidationConfig {
    LiquidationConfig {
        target_ratio_bps: TargetRatioBps(10_500),
        full_close_threshold: FullCloseThreshold {
            numerator: 2,
            denominator: 3,
        },
        partial_penalty_bps: LiquidationPenaltyBps(100),
        max_book_orders: LiquidationWorkCount(20),
        max_committed_fills: LiquidationWorkCount(10),
    }
}

fn update(predecessor: u64, sequence: u64, config: LiquidationConfig) -> SetLiquidationConfig {
    SetLiquidationConfig {
        predecessor: LiquidationConfigSequence(predecessor),
        sequence: LiquidationConfigSequence(sequence),
        config,
    }
}

/// The action is the externally tagged variant name over a positional
/// array; every newtype is its bare integer. The expected bytes are built
/// from a plain map of tuples, so they do not depend on the derives under
/// test. The frozen hex is the same value, printed by
/// `hex::encode(canonical_admin_action_bytes(&action))`.
#[test]
fn set_liquidation_config_bytes_are_positional_and_canonical() {
    let action = AdminAction::SetLiquidationConfig(update(1, 2, config()));
    let bytes = canonical_admin_action_bytes(&action).expect("action encodes");

    let config_fields = (10_500_u32, (2_u32, 3_u32), 100_u32, 20_u32, 10_u32);
    let tagged = BTreeMap::from([("SetLiquidationConfig", (1_u64, 2_u64, config_fields))]);
    assert_eq!(
        bytes,
        rmp_serde::to_vec(&tagged).expect("expected bytes encode")
    );
    assert_eq!(
        hex::encode(&bytes),
        concat!(
            "81b45365744c69717569646174696f6e436f6e666967", // {"SetLiquidationConfig":
            "930102",                                       // [1, 2,
            "95cd29049202036414",                           // [10500, [2, 3], 100, 20,
            "0a",                                           // 10]]}
        )
    );

    let (decoded, recanonical) = canonicalize_admin_action(&bytes).expect("action decodes");
    assert_eq!(
        recanonical, bytes,
        "canonical encoding must be a fixed point"
    );
    assert!(matches!(
        decoded,
        AdminAction::SetLiquidationConfig(cmd) if cmd == update(1, 2, config())
    ));
    assert_eq!(action.action_type(), AdminActionType::SetLiquidationConfig);
    assert_eq!(action.action_tag(), 0x11);
}

#[test]
fn the_decided_configuration_is_valid() {
    assert_eq!(config().validate(), Ok(()));
    assert_eq!(update(1, 2, config()).validate(), Ok(()));
}

#[test]
fn each_bound_refuses_with_the_rule_it_breaks() {
    let cases = [
        (
            LiquidationConfig {
                target_ratio_bps: TargetRatioBps(10_000),
                ..config()
            },
            LiquidationConfigError::TargetRatioNotAboveOne {
                target_ratio_bps: 10_000,
            },
        ),
        (
            LiquidationConfig {
                full_close_threshold: FullCloseThreshold {
                    numerator: 0,
                    denominator: 3,
                },
                ..config()
            },
            LiquidationConfigError::FullCloseThresholdOutOfRange {
                numerator: 0,
                denominator: 3,
            },
        ),
        (
            LiquidationConfig {
                full_close_threshold: FullCloseThreshold {
                    numerator: 3,
                    denominator: 3,
                },
                ..config()
            },
            LiquidationConfigError::FullCloseThresholdOutOfRange {
                numerator: 3,
                denominator: 3,
            },
        ),
        (
            LiquidationConfig {
                full_close_threshold: FullCloseThreshold {
                    numerator: 1,
                    denominator: 0,
                },
                ..config()
            },
            LiquidationConfigError::FullCloseThresholdOutOfRange {
                numerator: 1,
                denominator: 0,
            },
        ),
        (
            LiquidationConfig {
                partial_penalty_bps: LiquidationPenaltyBps(101),
                ..config()
            },
            LiquidationConfigError::PenaltyAboveMaximum {
                partial_penalty_bps: 101,
            },
        ),
        (
            LiquidationConfig {
                max_committed_fills: LiquidationWorkCount(0),
                ..config()
            },
            LiquidationConfigError::ZeroWorkLimit {
                field: "max_committed_fills",
            },
        ),
    ];
    for (bad, expected) in cases {
        assert_eq!(bad.validate(), Err(expected));
        assert_eq!(update(1, 2, bad).validate(), Err(expected));
    }
    for (predecessor, sequence) in [(2, 2), (3, 2)] {
        assert_eq!(
            update(predecessor, sequence, config()).validate(),
            Err(LiquidationConfigError::SequenceNotIncreasing {
                predecessor: LiquidationConfigSequence(predecessor),
                sequence: LiquidationConfigSequence(sequence),
            })
        );
    }
}

#[test]
fn liquidation_config_set_attributes_are_exact() {
    let event = Event::LiquidationConfigSet {
        sequence: 2,
        target_ratio_bps: 10_500,
        full_close_numerator: 2,
        full_close_denominator: 3,
        partial_penalty_bps: 100,
        max_book_orders: 20,
        max_committed_fills: 10,
        proposal_id: 7,
    };
    let bytes = rmp_serde::to_vec(&event).expect("event encodes");
    let decoded: Event = rmp_serde::from_slice(&bytes).expect("event decodes");
    assert_eq!(
        rmp_serde::to_vec(&decoded).expect("event re-encodes"),
        bytes
    );

    let mut actual = AbciEventWriter::new();
    event.encode_abci(&mut actual);
    let mut expected = AbciEventWriter::new();
    expected.begin_event("liquidation_config_set", 8);
    expected.write_attr_u64("sequence", 2);
    expected.write_attr_u64("target_ratio_bps", 10_500);
    expected.write_attr_u64("full_close_numerator", 2);
    expected.write_attr_u64("full_close_denominator", 3);
    expected.write_attr_u64("partial_penalty_bps", 100);
    expected.write_attr_u64("max_book_orders", 20);
    expected.write_attr_u64("max_committed_fills", 10);
    expected.write_attr_u64("proposal_id", 7);
    assert_eq!(actual.into_vec(), expected.into_vec());
}
