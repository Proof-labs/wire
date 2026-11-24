#![allow(clippy::arithmetic_side_effects, clippy::expect_used, clippy::panic)]

use super::*;
use crate::codec::{
    canonical_action_type, decode_tx, encode_signed_tx, encode_signed_tx_raw, Action, ActionType,
};
use crate::types::{ClientTriggerId, TriggerSlippageBps};

const OWNER: [u8; 20] = [0x0A; 20];

fn payload_hex<T: Serialize>(payload: &T) -> String {
    hex::encode(rmp_serde::to_vec(payload).expect("payload encodes"))
}

fn decode<T: serde::de::DeserializeOwned>(hex_bytes: &str) -> T {
    rmp_serde::from_slice(&hex::decode(hex_bytes.trim()).expect("vector is valid hex"))
        .expect("vector decodes")
}

fn entry(market: MarketId, side: Side, price: u64) -> OrderEntry {
    OrderEntry::limit(market, side, price, 10)
}

/// A post-only limit buy with a stop-loss, and a reduce-only market sell
/// that stops at a worst price.
fn two_orders() -> PlaceOrders {
    PlaceOrders {
        owner: OWNER,
        orders: EntryList::try_from(vec![
            OrderEntry {
                kind: OrderKind::Limit {
                    price: 99_000_000,
                    time_in_force: TimeInForce::Gtc,
                    post_only: true,
                },
                client_order_id: Some(1),
                stop_loss: Some(TriggerLimb {
                    trigger_price: 95_000,
                    max_slippage_bps: TriggerSlippageBps(75),
                    client_trigger_id: Some(ClientTriggerId(11)),
                }),
                ..entry(1, Side::Buy, 99_000_000)
            },
            OrderEntry {
                kind: OrderKind::Market {
                    worst_price: Some(101_000_000),
                },
                reduce_only: true,
                ..OrderEntry::market(2, Side::Sell, 10)
            },
        ])
        .expect("two orders are in range"),
    }
}

/// Regenerate by encoding `two_orders()` and the cancels below with
/// `rmp_serde::to_vec` and writing the hex to the named vectors. A limit
/// entry is `[market, side, quantity, {"Limit": [price, time_in_force,
/// post_only]}, reduce_only, client_order_id, stop_loss, take_profit]`; a
/// market entry carries `{"Market": [worst_price]}` instead.
#[test]
fn list_shape_vectors_round_trip() {
    let place = include_str!("../vectors/place_orders_list.hex").trim();
    assert_eq!(payload_hex(&two_orders()), place);
    let decoded: PlaceOrders = decode(place);
    assert_eq!(payload_hex(&decoded), place);
    assert_eq!(decoded.orders.as_slice().len(), 2);

    let cancel = include_str!("../vectors/cancel_orders_list.hex").trim();
    let orders = CancelOrders {
        owner: OWNER,
        order_ids: EntryList::try_from(vec![42, 43, 44]).expect("three cancels"),
    };
    assert_eq!(payload_hex(&orders), cancel);
    let decoded: CancelOrders = decode(cancel);
    assert_eq!(decoded.order_ids.as_slice(), &[42, 43, 44]);

    let client = include_str!("../vectors/cancel_client_orders_list.hex").trim();
    let orders = CancelClientOrders {
        owner: OWNER,
        client_order_ids: EntryList::try_from(vec![7, 8]).expect("two cancels"),
    };
    assert_eq!(payload_hex(&orders), client);
    let decoded: CancelClientOrders = decode(client);
    assert_eq!(decoded.client_order_ids.as_slice(), &[7, 8]);

    let tx = encode_signed_tx(&Action::PlaceOrders(two_orders()), 9, &[0; 32], &[0; 64])
        .expect("tx encodes");
    assert_eq!(canonical_action_type(&tx), Some(ActionType::PlaceOrders));
}

/// A list of one is a list like any other: `[owner, [entry]]`.
#[test]
fn a_list_of_one_is_a_list() {
    let one = PlaceOrders::entry(OWNER, entry(1, Side::Buy, 100));
    assert_eq!(
        payload_hex(&one),
        hex::encode(rmp_serde::to_vec(&(OWNER, [entry(1, Side::Buy, 100)])).expect("encodes")),
    );
    let tx =
        encode_signed_tx(&Action::PlaceOrders(one), 1, &[0; 32], &[0; 64]).expect("tx encodes");
    assert_eq!(canonical_action_type(&tx), Some(ActionType::PlaceOrders));
}

fn decodes(action_type: ActionType, payload: Vec<u8>) -> bool {
    let tx = encode_signed_tx_raw(action_type as u8, &payload, 1, &[0; 32], &[0; 64])
        .expect("envelope encodes");
    decode_tx(&tx).is_ok()
}

#[test]
fn lists_are_bounded_at_decode() {
    for (len, accepted) in [
        (0, false),
        (1, true),
        (MAX_LIST_ENTRIES, true),
        (MAX_LIST_ENTRIES + 1, false),
    ] {
        let orders: Vec<_> = (0..len as u64)
            .map(|i| entry(1, Side::Buy, 1_000 + i))
            .collect();
        let ids: Vec<u64> = (1..=len as u64).collect();
        let place = rmp_serde::to_vec(&(OWNER, orders)).expect("encodes");
        let cancel = rmp_serde::to_vec(&(OWNER, &ids)).expect("encodes");
        assert_eq!(
            decodes(ActionType::PlaceOrders, place),
            accepted,
            "{len} orders"
        );
        assert_eq!(
            decodes(ActionType::CancelOrders, cancel.clone()),
            accepted,
            "{len} cancels"
        );
        assert_eq!(
            decodes(ActionType::CancelClientOrders, cancel),
            accepted,
            "{len} client cancels"
        );
    }
    assert_eq!(
        EntryList::<u64>::try_from(Vec::new()).map(|_| ()),
        Err(ListSizeOutOfRange(0))
    );
}

/// The SDK bindings read and write the list as a map.
#[test]
fn human_readable_formats_round_trip() {
    let json = serde_json::to_string(&two_orders()).expect("list encodes");
    let decoded: PlaceOrders = serde_json::from_str(&json).expect("list decodes");
    assert_eq!(payload_hex(&decoded), payload_hex(&two_orders()));
}

#[test]
fn list_events_render_one_json_object_per_entry() {
    let placements = OrderPlacements(vec![
        OrderPlacement {
            market: 1,
            client_order_id: Some(4),
            outcome: PlacementOutcome::Resting {
                order_id: 9,
                side: Side::Buy,
                price: 100,
                quantity: 5,
                time_in_force: TimeInForce::Gtc,
                post_only: true,
                reduce_only: false,
            },
        },
        OrderPlacement {
            market: 2,
            client_order_id: None,
            outcome: PlacementOutcome::Rejected { code: 22 },
        },
        OrderPlacement {
            market: 3,
            client_order_id: None,
            outcome: PlacementOutcome::Executed {
                order_id: 10,
                requested_quantity: 5,
                filled_quantity: 0,
                unfilled: Some(UnfilledReason::NoLiquidity),
            },
        },
    ]);
    assert_eq!(
        placements.to_string(),
        r#"[{"outcome":"resting","market":"1","client_order_id":"4","order_id":"9","side":"buy","price":"100","quantity":"5","time_in_force":"gtc","post_only":"true","reduce_only":"false"},{"outcome":"rejected","market":"2","client_order_id":"","code":"22"},{"outcome":"executed","market":"3","client_order_id":"","order_id":"10","requested_quantity":"5","filled_quantity":"0","unfilled":"no_liquidity"}]"#
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&placements.to_string()).expect("valid JSON");
    assert_eq!(parsed.as_array().map(Vec::len), Some(3));

    let cancellations = OrderCancellations(vec![
        OrderCancellation::Cancelled {
            order_id: 9,
            market: 1,
            client_order_id: None,
            original_quantity: 5,
            remaining_quantity: 3,
            filled_quantity: 2,
        },
        OrderCancellation::Refused {
            target: CancelTarget::OrderId(10),
            code: 2,
        },
        OrderCancellation::Refused {
            target: CancelTarget::ClientOrderId(11),
            code: 60,
        },
    ]);
    assert_eq!(
        cancellations.to_string(),
        r#"[{"outcome":"cancelled","order_id":"9","market":"1","client_order_id":"","original_quantity":"5","remaining_quantity":"3","filled_quantity":"2"},{"outcome":"refused","order_id":"10","code":"2"},{"outcome":"refused","client_order_id":"11","code":"60"}]"#
    );
}
