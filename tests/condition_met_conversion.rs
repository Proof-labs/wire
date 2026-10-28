//! `ConditionMet` carries whether a winning conditional became a
//! perpetual position: a defaulted `converted_size` and `fallback_reason`
//! appended to the variant. Bytes written before the fields existed decode as
//! a cash-only settlement, and the eight existing ABCI attributes render
//! exactly as they did.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::types::{ConversionFallbackReason, Event, EventId, Side};
use serde::Serialize;

const OWNER: [u8; 20] = [0xB1; 20];

/// `ConditionMet` as it was encoded before conversion: the former variant
/// name `ConditionalSettled` and the first eight fields, in order.
#[derive(Serialize)]
enum LegacyEvent {
    ConditionalSettled {
        event_id: EventId,
        market: u32,
        owner: [u8; 20],
        side: Side,
        size: u64,
        entry_price: u64,
        settlement_price: u64,
        realized_pnl: i64,
    },
}

fn legacy() -> LegacyEvent {
    LegacyEvent::ConditionalSettled {
        event_id: EventId(42),
        market: 100,
        owner: OWNER,
        side: Side::Buy,
        size: 10,
        entry_price: 95_000_000,
        settlement_price: 100_000_000,
        realized_pnl: 50_000_000,
    }
}

fn settled(converted_size: u64, fallback_reason: Option<ConversionFallbackReason>) -> Event {
    Event::ConditionMet {
        event_id: EventId(42),
        market: 100,
        owner: OWNER,
        side: Side::Buy,
        size: 10,
        entry_price: 95_000_000,
        settlement_price: 100_000_000,
        realized_pnl: 50_000_000,
        converted_size,
        fallback_reason,
    }
}

fn render(event: &Event) -> Vec<u8> {
    let mut writer = AbciEventWriter::new();
    event.encode_abci(&mut writer);
    writer.into_vec()
}

/// The eight attributes a cash-only settlement rendered before conversion,
/// preceded by the event header, now under the name `condition_met`, with
/// `attr_count` attributes.
fn legacy_attributes(writer: &mut AbciEventWriter, attr_count: u16) {
    writer.begin_event("condition_met", attr_count);
    writer.write_attr_u64("event_id", 42);
    writer.write_attr_u64("market", 100);
    writer.write_attr_hex("owner", &OWNER);
    writer.write_attr_display("side", &Side::Buy);
    writer.write_attr_u64("size", 10);
    writer.write_attr_u64("entry_price", 95_000_000);
    writer.write_attr_u64("settlement_price", 100_000_000);
    writer.write_attr_display("realized_pnl", &50_000_000_i64);
}

#[test]
fn legacy_bytes_decode_as_a_cash_only_settlement() {
    let bytes = rmp_serde::to_vec(&legacy()).expect("legacy shape encodes");
    match rmp_serde::from_slice::<Event>(&bytes).expect("legacy bytes decode") {
        Event::ConditionMet {
            size,
            realized_pnl,
            converted_size,
            fallback_reason,
            ..
        } => {
            assert_eq!(size, 10);
            assert_eq!(realized_pnl, 50_000_000);
            assert_eq!(converted_size, 0, "nothing converted before conversion");
            assert_eq!(fallback_reason, None);
        }
        other => panic!("expected ConditionMet, got {other:?}"),
    }
}

#[test]
fn conversion_fields_round_trip_byte_stable() {
    let cases = [
        (10, None),
        (0, Some(ConversionFallbackReason::MaintenanceMargin)),
        (0, Some(ConversionFallbackReason::PositionSizeCap)),
        (0, Some(ConversionFallbackReason::OpenInterestCap)),
        (0, Some(ConversionFallbackReason::CannotPriceOrMargin)),
        (0, Some(ConversionFallbackReason::InsufficientBalance)),
    ];
    for (converted, reason) in cases {
        let bytes = rmp_serde::to_vec(&settled(converted, reason)).expect("encodes");
        let decoded: Event = rmp_serde::from_slice(&bytes).expect("decodes");
        match &decoded {
            Event::ConditionMet {
                converted_size,
                fallback_reason,
                ..
            } => {
                assert_eq!(*converted_size, converted);
                assert_eq!(*fallback_reason, reason);
            }
            other => panic!("expected ConditionMet, got {other:?}"),
        }
        let re_encoded = rmp_serde::to_vec(&decoded).expect("re-encodes");
        assert_eq!(bytes, re_encoded, "{reason:?}: encoding is a fixed point");
    }
}

#[test]
fn existing_attributes_render_unchanged_and_the_new_two_append() {
    let mut expected = AbciEventWriter::new();
    legacy_attributes(&mut expected, 10);
    expected.write_attr_u64("converted_size", 0);
    expected.write_attr("fallback_reason", "");
    assert_eq!(
        render(&settled(0, None)),
        expected.into_vec(),
        "a value built without the fields keeps the eight attributes it rendered, then appends two"
    );

    let mut converted = AbciEventWriter::new();
    legacy_attributes(&mut converted, 10);
    converted.write_attr_u64("converted_size", 10);
    converted.write_attr("fallback_reason", "");
    assert_eq!(render(&settled(10, None)), converted.into_vec());

    let mut fell_back = AbciEventWriter::new();
    legacy_attributes(&mut fell_back, 10);
    fell_back.write_attr_u64("converted_size", 0);
    fell_back.write_attr("fallback_reason", "maintenance_margin");
    assert_eq!(
        render(&settled(
            0,
            Some(ConversionFallbackReason::MaintenanceMargin)
        )),
        fell_back.into_vec()
    );
}

#[test]
fn fallback_reason_literals_stay_stable() {
    for (reason, literal) in [
        (
            ConversionFallbackReason::MaintenanceMargin,
            "maintenance_margin",
        ),
        (
            ConversionFallbackReason::PositionSizeCap,
            "position_size_cap",
        ),
        (
            ConversionFallbackReason::OpenInterestCap,
            "open_interest_cap",
        ),
        (
            ConversionFallbackReason::CannotPriceOrMargin,
            "cannot_price_or_margin",
        ),
        (
            ConversionFallbackReason::InsufficientBalance,
            "insufficient_balance",
        ),
    ] {
        assert_eq!(reason.to_string(), literal);
    }
}

/// The fallback reason as the releases before the rename declared it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ReasonBeforeRename {
    InitialMargin = 1,
}

/// The rename changes no byte: the margin fallback encodes exactly as the
/// variant it replaced, and each side decodes the other's.
#[test]
fn the_margin_fallback_encodes_as_it_did_before_the_rename() {
    let before = rmp_serde::to_vec(&Some(ReasonBeforeRename::InitialMargin)).expect("encodes");
    let after =
        rmp_serde::to_vec(&Some(ConversionFallbackReason::MaintenanceMargin)).expect("encodes");
    assert_eq!(after, before, "the encoded name is unchanged");
    assert_eq!(
        rmp_serde::from_slice::<Option<ConversionFallbackReason>>(&before)
            .expect("new decodes old"),
        Some(ConversionFallbackReason::MaintenanceMargin)
    );
    assert_eq!(
        rmp_serde::from_slice::<Option<ReasonBeforeRename>>(&after).expect("old decodes new"),
        Some(ReasonBeforeRename::InitialMargin)
    );
}

/// `ConditionNotMet` as it was encoded under its former name.
#[derive(Serialize)]
enum LegacyNotMet {
    ConditionalVoided {
        event_id: EventId,
        market: u32,
        owner: [u8; 20],
        side: Side,
        size: u64,
    },
}

#[test]
fn bytes_under_the_former_names_decode_to_the_renamed_variants() {
    let met = rmp_serde::to_vec(&legacy()).expect("legacy shape encodes");
    let decoded: Event = rmp_serde::from_slice(&met).expect("former name decodes");
    assert!(
        matches!(decoded, Event::ConditionMet { .. }),
        "got {decoded:?}"
    );

    let not_met = rmp_serde::to_vec(&LegacyNotMet::ConditionalVoided {
        event_id: EventId(42),
        market: 101,
        owner: OWNER,
        side: Side::Sell,
        size: 7,
    })
    .expect("legacy shape encodes");
    match rmp_serde::from_slice::<Event>(&not_met).expect("former name decodes") {
        Event::ConditionNotMet { size, side, .. } => {
            assert_eq!(size, 7);
            assert_eq!(side, Side::Sell);
        }
        other => panic!("expected ConditionNotMet, got {other:?}"),
    }
}

#[test]
fn the_renamed_variants_encode_and_render_under_the_new_names() {
    let not_met = Event::ConditionNotMet {
        event_id: EventId(42),
        market: 101,
        owner: OWNER,
        side: Side::Sell,
        size: 7,
    };
    for (event, name) in [
        (settled(0, None), "ConditionMet"),
        (not_met.clone(), "ConditionNotMet"),
    ] {
        let bytes = rmp_serde::to_vec(&event).expect("encodes");
        let decoded: Event = rmp_serde::from_slice(&bytes).expect("decodes");
        assert_eq!(
            rmp_serde::to_vec(&decoded).expect("re-encodes"),
            bytes,
            "{name}: encoding is a fixed point"
        );
        assert!(
            !String::from_utf8_lossy(&bytes).contains("Conditional"),
            "{name}: the former name is never written"
        );
    }

    let mut expected = AbciEventWriter::new();
    expected.begin_event("condition_not_met", 5);
    expected.write_attr_u64("event_id", 42);
    expected.write_attr_u64("market", 101);
    expected.write_attr_hex("owner", &OWNER);
    expected.write_attr_display("side", &Side::Sell);
    expected.write_attr_u64("size", 7);
    assert_eq!(render(&not_met), expected.into_vec());
}
