//! `ConditionalSettled` carries whether a winning conditional became a
//! perpetual position: a defaulted `converted_size` and `fallback_reason`
//! appended to the variant. Bytes written before the fields existed decode as
//! a cash-only settlement, and the eight existing ABCI attributes render
//! exactly as they did.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::types::{ConversionFallbackReason, Event, EventId, Side};
use serde::Serialize;

const OWNER: [u8; 20] = [0xB1; 20];

/// `ConditionalSettled` as it was encoded before conversion: the same
/// variant name and the first eight fields, in order.
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
    Event::ConditionalSettled {
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
/// preceded by the event header with `attr_count` attributes.
fn legacy_attributes(writer: &mut AbciEventWriter, attr_count: u16) {
    writer.begin_event("conditional_settled", attr_count);
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
        Event::ConditionalSettled {
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
        other => panic!("expected ConditionalSettled, got {other:?}"),
    }
}

#[test]
fn conversion_fields_round_trip_byte_stable() {
    let cases = [
        (10, None),
        (0, Some(ConversionFallbackReason::InitialMargin)),
        (0, Some(ConversionFallbackReason::PositionSizeCap)),
        (0, Some(ConversionFallbackReason::OpenInterestCap)),
        (0, Some(ConversionFallbackReason::CannotPriceOrMargin)),
        (0, Some(ConversionFallbackReason::InsufficientBalance)),
    ];
    for (converted, reason) in cases {
        let bytes = rmp_serde::to_vec(&settled(converted, reason)).expect("encodes");
        let decoded: Event = rmp_serde::from_slice(&bytes).expect("decodes");
        match &decoded {
            Event::ConditionalSettled {
                converted_size,
                fallback_reason,
                ..
            } => {
                assert_eq!(*converted_size, converted);
                assert_eq!(*fallback_reason, reason);
            }
            other => panic!("expected ConditionalSettled, got {other:?}"),
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
    fell_back.write_attr("fallback_reason", "initial_margin");
    assert_eq!(
        render(&settled(0, Some(ConversionFallbackReason::InitialMargin))),
        fell_back.into_vec()
    );
}

#[test]
fn fallback_reason_literals_stay_stable() {
    for (reason, literal) in [
        (ConversionFallbackReason::InitialMargin, "initial_margin"),
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
