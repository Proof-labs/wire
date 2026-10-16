//! `TradeExecuted` gains the mark and oracle price at the fill, and
//! `MarketOrderProcessed` gains a time in force, limit price and unfilled
//! reason. Bytes written
//! before the fields existed decode with them absent, and the existing ABCI
//! attributes render as before with the new ones appended.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::types::{Event, Side, TimeInForce, UnfilledReason};
use serde::Serialize;

const OWNER: [u8; 20] = [0xB1; 20];

/// Both variants as they were encoded before the new fields.
#[derive(Serialize)]
enum LegacyEvent {
    TradeExecuted {
        fill_id: u64,
        market: u32,
        price: u64,
        quantity: u64,
        maker_order_id: u64,
        maker_client_order_id: u64,
        maker_owner: [u8; 20],
        maker_side: Side,
        taker_order_id: u64,
        taker_owner: [u8; 20],
        taker_client_order_id: u64,
        taker_fee: i64,
        maker_fee: i64,
    },
    MarketOrderProcessed {
        order_id: u64,
        market: u32,
        owner: [u8; 20],
        side: Side,
        requested_quantity: u64,
        filled_quantity: u64,
        client_order_id: u64,
    },
}

fn decode(legacy: &LegacyEvent) -> Event {
    let bytes = rmp_serde::to_vec(legacy).expect("legacy shape encodes");
    rmp_serde::from_slice(&bytes).expect("legacy bytes decode")
}

#[test]
fn legacy_trade_decodes_without_prices() {
    let decoded = decode(&LegacyEvent::TradeExecuted {
        fill_id: 1,
        market: 7,
        price: 100,
        quantity: 2,
        maker_order_id: 3,
        maker_client_order_id: 0,
        maker_owner: OWNER,
        maker_side: Side::Sell,
        taker_order_id: 4,
        taker_owner: OWNER,
        taker_client_order_id: 0,
        taker_fee: 1,
        maker_fee: -1,
    });
    let Event::TradeExecuted {
        mark_price,
        oracle_price,
        ..
    } = decoded
    else {
        panic!("expected TradeExecuted, got {decoded:?}");
    };
    assert_eq!((mark_price, oracle_price), (None, None));
}

#[test]
fn legacy_summary_decodes_as_a_market_order() {
    let decoded = decode(&LegacyEvent::MarketOrderProcessed {
        order_id: 4,
        market: 7,
        owner: OWNER,
        side: Side::Buy,
        requested_quantity: 5,
        filled_quantity: 2,
        client_order_id: 0,
    });
    let Event::MarketOrderProcessed {
        time_in_force,
        limit_price,
        unfilled_reason,
        ..
    } = decoded
    else {
        panic!("expected MarketOrderProcessed, got {decoded:?}");
    };
    assert_eq!(time_in_force, TimeInForce::Ioc, "only market orders had it");
    assert_eq!(limit_price, None);
    assert_eq!(unfilled_reason, None);
}

#[test]
fn trade_executed_appends_its_new_attributes() {
    let event = Event::TradeExecuted {
        fill_id: 1,
        market: 7,
        price: 100,
        quantity: 2,
        maker_order_id: 3,
        maker_client_order_id: 0,
        maker_owner: OWNER,
        maker_side: Side::Sell,
        taker_order_id: 4,
        taker_owner: OWNER,
        taker_client_order_id: 0,
        taker_fee: 1,
        maker_fee: -1,
        mark_price: Some(1_234),
        oracle_price: None,
    };
    let mut rendered = AbciEventWriter::new();
    event.encode_abci(&mut rendered);

    let mut expected = AbciEventWriter::new();
    expected.begin_event("trade_executed", 15);
    expected.write_attr_u64("fill_id", 1);
    expected.write_attr_u64("market", 7);
    expected.write_attr_u64("price", 100);
    expected.write_attr_u64("quantity", 2);
    expected.write_attr_u64("maker_order_id", 3);
    expected.write_attr_u64("maker_client_order_id", 0);
    expected.write_attr_hex("maker_owner", &OWNER);
    expected.write_attr_display("maker_side", &Side::Sell);
    expected.write_attr_u64("taker_order_id", 4);
    expected.write_attr_hex("taker_owner", &OWNER);
    expected.write_attr_u64("taker_client_order_id", 0);
    expected.write_attr_display("taker_fee", &1_i64);
    expected.write_attr_display("maker_fee", &-1_i64);
    expected.write_attr_display("mark_price", &1_234_u64);
    expected.write_attr("oracle_price", "");
    assert_eq!(rendered.into_vec(), expected.into_vec());
}

#[test]
fn summary_appends_its_new_attributes() {
    let event = Event::MarketOrderProcessed {
        order_id: 4,
        market: 7,
        owner: OWNER,
        side: Side::Buy,
        requested_quantity: 5,
        filled_quantity: 2,
        client_order_id: 0,
        time_in_force: TimeInForce::Ioc,
        limit_price: Some(100),
        unfilled_reason: Some(UnfilledReason::NoLiquidity),
    };
    let mut rendered = AbciEventWriter::new();
    event.encode_abci(&mut rendered);

    let mut expected = AbciEventWriter::new();
    expected.begin_event("market_order_processed", 10);
    expected.write_attr_u64("order_id", 4);
    expected.write_attr_u64("market", 7);
    expected.write_attr_hex("owner", &OWNER);
    expected.write_attr_display("side", &Side::Buy);
    expected.write_attr_u64("requested_quantity", 5);
    expected.write_attr_u64("filled_quantity", 2);
    expected.write_attr_u64("client_order_id", 0);
    expected.write_attr_display("time_in_force", &TimeInForce::Ioc);
    expected.write_attr_display("limit_price", &100_u64);
    expected.write_attr("unfilled_reason", "no_liquidity");
    assert_eq!(rendered.into_vec(), expected.into_vec());
}

#[test]
fn unfilled_reason_literals_stay_stable() {
    for (reason, literal) in [
        (UnfilledReason::ReduceOnlyCap, "reduce_only_cap"),
        (UnfilledReason::SelfTrade, "self_trade"),
        (UnfilledReason::NoLiquidity, "no_liquidity"),
    ] {
        assert_eq!(reason.to_string(), literal);
    }
}
