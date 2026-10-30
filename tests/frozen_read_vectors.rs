//! Frozen bytes of the events and query responses consumers decode.
//!
//! `vectors/events/<event type>.hex` holds one `Event` variant, named by the
//! ABCI event type it renders as. `vectors/responses/<wire type>.hex` holds one
//! read-model response, named by its type in snake case. Each fixture below
//! must encode to its file, and the file must decode and re-encode to the same
//! bytes. A mismatch prints the current encoding; re-pin only on an intended
//! layout change.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::query::{AccountInfo, BindingScenario, PositionBrief};
use proof_wire::types::{
    AttachedConditional, Branch, ClientTriggerId, ConversionFallbackReason, Event, EventId,
    EventInfo, EventOracleSource, EventStatus, FeeTier, MarkSourceMode, MarketConfig, MarketKind,
    Outcome, OwnerAddress, PendingTriggerDiscardReason, PositionEpoch, PriceComparison, Side,
    TradeReason, TriggerLimb, TriggerSlippageBps,
};
use serde::{de::DeserializeOwned, Serialize};

const OWNER: [u8; 20] = [0xA1; 20];
const COUNTERPARTY: [u8; 20] = [0xB2; 20];
const SIGNER: [u8; 20] = [0xC3; 20];
const AGENT_PUBKEY: [u8; 32] = [0xD4; 32];
const SOLANA_ACCOUNT: [u8; 32] = [0xE5; 32];
const MARKET: u32 = 7;
const CONDITIONAL_MARKET: u32 = 9;
const EVENT: EventId = EventId(42);

fn vector_path(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vectors")
        .join(kind)
        .join(format!("{name}.hex"))
}

fn vector_names(kind: &str) -> BTreeSet<String> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vectors")
        .join(kind);
    fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("reading {}: {error}", directory.display()))
        .map(|entry| entry.expect("vector entry is readable").path())
        .map(|path| {
            assert_eq!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("hex"),
                "{} is not a .hex vector",
                path.display()
            );
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .expect("vector names are UTF-8")
                .to_owned()
        })
        .collect()
}

fn assert_frozen<T: Serialize + DeserializeOwned>(kind: &str, name: &str, fixture: &T) {
    let encoded = hex::encode(rmp_serde::to_vec(fixture).expect("fixture encodes"));
    let path = vector_path(kind, name);
    let frozen = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("reading {}: {error}; encoding is {encoded}", path.display())
    });
    let frozen = frozen.trim();
    assert_eq!(encoded, frozen, "{kind}/{name}.hex drifted");
    let bytes = hex::decode(frozen).expect("vector is valid hex");
    let decoded: T = rmp_serde::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("{kind}/{name}.hex does not decode: {error}"));
    assert_eq!(
        rmp_serde::to_vec(&decoded).expect("decoded vector re-encodes"),
        bytes,
        "{kind}/{name}.hex does not re-encode to itself"
    );
}

/// The ABCI event type an event renders as: the first length-prefixed string
/// `encode_abci` writes.
fn abci_event_type(event: &Event) -> String {
    let mut writer = AbciEventWriter::new();
    event.encode_abci(&mut writer);
    let bytes = writer.into_vec();
    let length = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    String::from_utf8(bytes[2..][..length].to_vec()).expect("event type is UTF-8")
}

/// Every event variant the indexer folds into a derived table.
fn folded_events() -> Vec<Event> {
    let owner = OwnerAddress::new(OWNER);
    let stop_loss = TriggerLimb {
        trigger_price: 95_000_000,
        max_slippage_bps: TriggerSlippageBps(50),
        client_trigger_id: Some(ClientTriggerId(11)),
    };
    let take_profit = TriggerLimb {
        trigger_price: 120_000_000,
        max_slippage_bps: TriggerSlippageBps(25),
        client_trigger_id: None,
    };
    vec![
        Event::TradeExecuted {
            fill_id: 1001,
            market: MARKET,
            price: 100_000_000,
            quantity: 5,
            maker_order_id: 2001,
            maker_client_order_id: 3001,
            maker_owner: COUNTERPARTY,
            maker_side: Side::Sell,
            taker_order_id: 2002,
            taker_owner: OWNER,
            taker_client_order_id: 3002,
            taker_fee: 250,
            maker_fee: -50,
            mark_price: Some(100_010_000),
            oracle_price: Some(99_990_000),
            reason: TradeReason::Liquidation,
        },
        Event::FundingApplied {
            market: MARKET,
            funding_rate_bps: -3,
            cumulative_funding: -1_234_567,
            timestamp_ms: 1_767_225_600_000,
        },
        Event::PositionUpdated {
            owner: OWNER,
            market: MARKET,
            side: Side::Buy,
            entry_price: 100_000_000,
            size: 5,
            realized_pnl: 40,
        },
        Event::PositionClosed {
            owner: OWNER,
            market: MARKET,
            realized_pnl: -75,
        },
        Event::PositionAutoDeleveraged {
            owner: OWNER,
            market: MARKET,
            side: Side::Sell,
            size: 3,
            close_price: 101_000_000,
            close_price_spec: 101_500_000,
            realized_pnl: 3_000_000,
        },
        Event::PositionMigrated {
            owner: OWNER,
            from_market: CONDITIONAL_MARKET,
            to_market: MARKET,
            side: Side::Buy,
            entry_price: 640_000,
            size: 12,
            cash_delta: -1_500,
        },
        Event::ConditionMet {
            event_id: EVENT,
            market: CONDITIONAL_MARKET,
            owner: OWNER,
            side: Side::Buy,
            size: 4,
            entry_price: 98_000_000,
            settlement_price: 102_000_000,
            realized_pnl: 16_000_000,
            converted_size: 0,
            fallback_reason: Some(ConversionFallbackReason::MaintenanceMargin),
        },
        Event::ConditionNotMet {
            event_id: EVENT,
            market: CONDITIONAL_MARKET,
            owner: OWNER,
            side: Side::Sell,
            size: 4,
        },
        Event::PredictionSettled {
            event_id: EVENT,
            market: CONDITIONAL_MARKET,
            owner: OWNER,
            side: Side::Buy,
            size: 10,
            payoff_per_share: 1_000_000,
            cash_delta: 3_600_000,
        },
        Event::OrderbookLevelUpdated {
            market: MARKET,
            side: Side::Buy,
            price: 99_500_000,
            total_quantity: 17,
            order_count: 3,
        },
        Event::MarketCreated {
            market: MARKET,
            im_bps: 1_000,
            mm_bps: 500,
            taker_fee_bps: 5,
            maker_fee_bps: 2,
            funding_interval_ms: 3_600_000,
            max_funding_rate_bps: 100,
            signer: Some(SIGNER),
        },
        Event::PriceUpdated {
            market: MARKET,
            price: 100_250_000,
            signer: Some(SIGNER),
        },
        Event::AgentApproved {
            owner: OWNER,
            agent: COUNTERPARTY,
            agent_pubkey: AGENT_PUBKEY,
        },
        Event::AgentRevoked {
            owner: OWNER,
            agent: COUNTERPARTY,
            agent_pubkey: AGENT_PUBKEY,
        },
        Event::EventResolved {
            event_id: EVENT,
            outcome: Outcome::Yes,
            timestamp_ms: 1_767_229_200_000,
            signer: Some(SIGNER),
        },
        Event::DepositConfirmed {
            owner: OWNER,
            amount: 5_000_000,
            new_balance: 12_000_000,
            solana_tx_sig: vec![0xF6; 64],
            signer: Some(SIGNER),
        },
        Event::WithdrawRequested {
            withdrawal_id: 77,
            owner: OWNER,
            amount: 2_000_000,
            solana_destination: SOLANA_ACCOUNT,
        },
        Event::WithdrawalConfirmed {
            withdrawal_id: 77,
            solana_tx_sig: vec![0xF7; 64],
            signer: Some(SIGNER),
        },
        Event::WithdrawalFailed {
            withdrawal_id: 78,
            owner: OWNER,
            amount: 2_000_000,
            new_balance: 10_000_000,
            reason: "destination_rejected".to_owned(),
            signer: Some(SIGNER),
        },
        Event::PositionTriggersSet {
            owner,
            market: MARKET,
            position_epoch: 3,
            group_id: 501,
            client_group_id: 601,
            stop_limb_id: 701,
            stop_client_trigger_id: 11,
            take_profit_limb_id: 702,
            take_profit_client_trigger_id: 12,
            accepted_height: 1_000,
            active_from_height: 1_001,
            replaced_group_id: 500,
            source_order_id: 2002,
        },
        Event::PositionTriggersCancelled {
            owner: OWNER,
            market: MARKET,
            position_epoch: 3,
            group_id: 501,
        },
        Event::PositionTriggersInvalidated {
            owner,
            market: MARKET,
            position_epoch: 3,
            group_id: 501,
            invalidation_reason: 2,
        },
        Event::PositionTriggerActivated {
            owner,
            market: MARKET,
            position_epoch: 3,
            group_id: 501,
            limb_id: 701,
            client_group_id: 601,
            client_trigger_id: 11,
            limb_kind: "stop_loss".to_owned(),
            trigger_price: 95_000_000,
            frozen_mark: 94_900_000,
            limit_price: 94_425_000,
            requested_quantity: 5,
            execution_order_id: 2003,
        },
        Event::PositionTriggerExecuted {
            owner,
            market: MARKET,
            position_epoch: 3,
            group_id: 501,
            limb_id: 701,
            client_group_id: 601,
            client_trigger_id: 11,
            limb_kind: "stop_loss".to_owned(),
            trigger_price: 95_000_000,
            frozen_mark: 94_900_000,
            limit_price: 94_425_000,
            requested_quantity: 5,
            filled_quantity: 4,
            residual_quantity: 1,
            execution_order_id: 2003,
            total_fee: 190,
            result: "partially_filled".to_owned(),
            reason: "no_liquidity".to_owned(),
        },
        Event::PositionTriggerDeferred {
            owner,
            market: MARKET,
            position_epoch: 3,
            group_id: 501,
            limb_id: 702,
            client_group_id: 601,
            client_trigger_id: 12,
            limb_kind: "take_profit".to_owned(),
            trigger_price: 120_000_000,
            frozen_mark: 120_100_000,
            requested_quantity: 5,
            reason: "market_closed".to_owned(),
        },
        Event::TriggerMarketDeferred {
            market: MARKET,
            reason: "oracle_stale".to_owned(),
        },
        Event::TriggerMarketResumed {
            market: MARKET,
            previous_reason: "oracle_stale".to_owned(),
        },
        Event::PendingTriggersAttached {
            owner,
            market: MARKET,
            order_id: 2004,
            client_order_id: 3004,
            stop_loss: Some(stop_loss),
            take_profit: Some(take_profit),
        },
        Event::PendingTriggersDiscarded {
            owner,
            market: MARKET,
            order_id: 2004,
            reason: PendingTriggerDiscardReason::OrderCancelled,
        },
        Event::SubAccountCreated {
            owner,
            sub_account_id: 2,
            address: OwnerAddress::new(COUNTERPARTY),
            name: [0x61; 32],
        },
    ]
}

fn position_brief() -> PositionBrief {
    PositionBrief {
        owner: OWNER,
        market: MARKET,
        side: Side::Buy,
        entry_price: 100_000_000,
        size: 5,
        last_funding_index: -1_234_567,
        upnl_now: 1_250_000,
        mm_now: 25_000_000,
        im_now: 50_000_000,
        pnl_if_fires: 1_250_000,
        pnl_if_dies: 1_250_000,
        funding_since: -320,
        adl_score: 2_500_000,
        position_epoch: Some(PositionEpoch(3)),
    }
}

fn account_info() -> AccountInfo {
    AccountInfo {
        balance: 12_000_000_000,
        positions: vec![position_brief()],
        equity: 13_250_000_000,
        total_mm: 25_000_000,
        total_im: 60_000_000,
        margin_ratio_bps: 5_300_000,
        binding_scenario: vec![BindingScenario {
            event_id: EVENT,
            branch: Branch::No,
        }],
        fees_accrued: -4_200,
        volume_30d_micro_usdc: 870_000_000_000,
        cashout_equity: 13_240_000_000,
    }
}

fn market_config() -> MarketConfig {
    MarketConfig {
        market: CONDITIONAL_MARKET,
        im_bps: 1_000,
        mm_bps: 500,
        taker_fee_bps: 5,
        maker_fee_bps: 2,
        funding_interval_ms: 3_600_000,
        max_funding_rate_bps: 100,
        kind: MarketKind::ConditionalPerp {
            event_id: EVENT,
            branch: Branch::Yes,
        },
        max_position_size: 1_000_000,
        default_ttl_ms: 86_400_000,
        net_delta_margin: true,
        pool_id: 1,
        mark_price_max_oracle_age_ms: 30_000,
        fee_tiers: vec![FeeTier {
            min_30d_volume_micro_usdc: 1_000_000_000_000,
            maker_fee_tenth_bps: -5,
            taker_fee_tenth_bps: 40,
        }],
        tick_size: 10_000,
        lot_size: 1,
        primary_oracle_signer: Some(SIGNER),
        oracle_staleness_ms: 60_000,
        mark_source_mode: MarkSourceMode::Median,
        max_mark_spread_bps: 200,
        cex_composite_staleness_ms: 15_000,
        sz_decimals: 3,
        ticker: "BTC-USD-FEDCUT-Y".to_owned(),
        max_open_interest: 50_000_000,
    }
}

fn event_info() -> EventInfo {
    EventInfo {
        event_id: EVENT,
        eby_market: 8,
        question: "Will the Fed cut rates?".to_owned(),
        settlement_ms: 1_767_225_600_000,
        resolution_window_ms: 86_400_000,
        status: EventStatus::Resolved(Outcome::Yes),
        created_ms: 1_764_547_200_000,
        resolved_ms: 1_767_229_200_000,
        oracle_source: Some(EventOracleSource::MarketOracle {
            market: MARKET,
            strike_price: 100_000_000,
            comparison: PriceComparison::GreaterThan,
        }),
        attached_conditionals: vec![AttachedConditional {
            underlying_market: MARKET,
            cpy_market: CONDITIONAL_MARKET,
            cpn_market: 10,
        }],
    }
}

#[test]
fn every_event_vector_matches_its_fixture() {
    let events = folded_events();
    let mut names = BTreeSet::new();
    for event in &events {
        let name = abci_event_type(event);
        assert_frozen("events", &name, event);
        names.insert(name);
    }
    assert_eq!(names.len(), events.len(), "one fixture per event type");
    assert_eq!(vector_names("events"), names);
}

#[test]
fn every_response_vector_matches_its_fixture() {
    assert_frozen("responses", "account_info", &account_info());
    assert_frozen("responses", "event_info", &event_info());
    assert_frozen("responses", "market_config", &market_config());
    assert_frozen("responses", "position_brief", &position_brief());
    let names = [
        "account_info",
        "event_info",
        "market_config",
        "position_brief",
    ];
    assert_eq!(
        vector_names("responses"),
        names.into_iter().map(str::to_owned).collect()
    );
}
