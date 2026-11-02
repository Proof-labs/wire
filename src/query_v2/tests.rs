use super::*;

fn round_trip<T>(view: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + core::fmt::Debug,
{
    let bytes = encode(view).expect("view encodes");
    let decoded: T = decode(&bytes).expect("view decodes");
    assert_eq!(&decoded, view);
}

/// A MessagePack map header for `len` entries, derived from the format's fixmap rule.
fn fixmap(len: u8) -> u8 {
    assert!(len < 16, "fixmap holds at most 15 entries");
    0x80 | len
}

fn order() -> Order {
    Order {
        order_id: 7,
        market: 1,
        owner: Address([1; 20]),
        side: Side::Sell,
        price: 66_750_000_000,
        remaining_quantity: 3,
        original_quantity: Some(5),
        filled_quantity: 2,
        client_order_id: None,
        created_ms: Some(1_700_000_000_000),
        post_only: false,
        reduce_only: true,
    }
}

#[test]
fn views_encode_as_maps_keyed_by_field_name() {
    let bytes = encode(&order()).expect("order encodes");
    assert_eq!(
        bytes.first(),
        Some(&fixmap(12)),
        "a view is a map, not an array"
    );
    let needle = b"remaining_quantity";
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "field names are the keys"
    );
}

#[test]
fn decoder_ignores_a_key_it_does_not_know() {
    #[derive(Serialize)]
    struct NewerOrder {
        #[serde(flatten)]
        order: Order,
        added_later: String,
    }
    let newer = NewerOrder {
        order: order(),
        added_later: "ignored".to_string(),
    };
    let bytes = encode(&newer).expect("newer order encodes");
    let decoded: Order = decode(&bytes).expect("older reader decodes newer bytes");
    assert_eq!(decoded, order());
}

#[test]
fn unknown_enum_values_decode_as_unknown() {
    #[derive(Serialize)]
    struct NewerLimb {
        state: &'static str,
        reason: &'static str,
    }
    #[derive(Deserialize)]
    struct OlderLimb {
        state: TriggerLimbState,
        reason: TriggerOutcomeReason,
    }
    let bytes = encode(&NewerLimb {
        state: "expired",
        reason: "added_later",
    })
    .expect("newer limb encodes");
    let decoded: OlderLimb = decode(&bytes).expect("older reader decodes newer variants");
    assert_eq!(decoded.state, TriggerLimbState::Unknown);
    assert_eq!(decoded.reason, TriggerOutcomeReason::Unknown);
}

#[test]
fn unknown_tagged_variants_decode_as_unknown() {
    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum NewerKind {
        Basket { legs: u8 },
    }
    let bytes = encode(&NewerKind::Basket { legs: 3 }).expect("newer kind encodes");
    assert_eq!(
        decode::<MarketKind>(&bytes).expect("older reader decodes"),
        MarketKind::Unknown
    );
}

#[test]
fn unknown_state_tags_decode_as_unknown() {
    #[derive(Serialize)]
    #[serde(tag = "state", rename_all = "snake_case")]
    enum NewerState {
        Vetoed { by: u8 },
    }
    let bytes = encode(&NewerState::Vetoed { by: 1 }).expect("newer state encodes");
    assert_eq!(
        decode::<ProposalStatus>(&bytes).expect("older reader decodes a proposal status"),
        ProposalStatus::Unknown
    );
    assert_eq!(
        decode::<UpgradeStatus>(&bytes).expect("older reader decodes an upgrade status"),
        UpgradeStatus::Unknown
    );
}

#[test]
fn market_round_trips() {
    round_trip(&Market {
        market: 42,
        ticker: Some("BTC-1-CPY".to_string()),
        kind: MarketKind::Conditional {
            event_id: EventId(1),
            branch: Branch::Yes,
        },
        sz_decimals: 5,
        tick_size: Some(10_000),
        lot_size: None,
        im_bps: 3_334,
        mm_bps: 1_667,
        taker_fee_bps: 5,
        maker_fee_bps: 2,
        fee_tiers: vec![FeeTier {
            min_30d_volume_micro_usdc: 0,
            maker_fee_tenth_bps: -5,
            taker_fee_tenth_bps: 40,
        }],
        funding_interval_ms: None,
        max_funding_rate_bps: 0,
        max_position_size: None,
        max_open_interest: Some(1_000),
        default_ttl_ms: Some(60_000),
        mark: None,
    });
}

#[test]
fn event_and_snapshot_round_trip() {
    let event = Event {
        event_id: EventId(3),
        binary_market: 30,
        question: "Will it rain?".to_string(),
        settlement_ms: 10,
        resolution_window_ms: 20,
        status: EventStatus::Resolved,
        outcome: Some(Outcome::No),
        created_ms: 1,
        resolved_ms: Some(11),
        resolution_source: ResolutionSource::MarketOracle {
            market: 1,
            strike_price: 5,
            comparison: PriceComparison::GreaterThan,
        },
        attached_conditionals: vec![AttachedConditional {
            underlying_market: 1,
            yes_market: 31,
            no_market: 32,
        }],
    };
    round_trip(&event);
    round_trip(&MarketsSnapshot {
        chain_id: Hash32([9; 32]),
        height: 100,
        markets: Vec::new(),
        events: vec![event],
        open_interest: vec![MarketOpenInterest {
            market: 1,
            long: 4,
            short: 3,
        }],
    });
}

#[test]
fn trigger_view_round_trips() {
    round_trip(&PositionTriggers {
        owner: Address([2; 20]),
        market: 1,
        position_epoch: PositionEpoch(4),
        position_side: Side::Buy,
        client_group_id: Some(ClientTriggerGroupId(9)),
        stop_loss: Some(TriggerLimbInfo {
            trigger_price: 60_000_000_000,
            max_slippage_bps: TriggerSlippageBps(50),
            client_trigger_id: None,
            state: TriggerLimbState::Partial,
            last_outcome: Some(TriggerOutcome {
                requested_quantity: 10,
                filled_quantity: 4,
                reason: Some(TriggerOutcomeReason::NoEligibleLiquidity),
            }),
        }),
        take_profit: None,
    });
}

#[test]
fn account_views_round_trip() {
    round_trip(&Account {
        balance: 100,
        equity: -5,
        total_mm: 10,
        total_im: 20,
        margin_ratio_bps: None,
        binding_scenario: vec![BindingOutcome {
            event_id: EventId(1),
            branch: Branch::No,
        }],
        fees_accrued: 3,
        volume_30d_micro_usdc: 4,
        cashout_equity: 90,
        positions: vec![PositionSummary {
            market: 1,
            side: Side::Buy,
            entry_price: 10,
            size: 2,
            position_epoch: Some(PositionEpoch(1)),
            upnl_now: None,
            mm_now: 1,
            im_now: 2,
            pnl_if_fires: 3,
            pnl_if_dies: -3,
            funding_since: 0,
            adl_score: 0,
        }],
    });
    round_trip(&PositionSummary {
        market: 2,
        side: Side::Sell,
        entry_price: 4,
        size: 1,
        position_epoch: None,
        upnl_now: Some(-6),
        mm_now: 1,
        im_now: 2,
        pnl_if_fires: 0,
        pnl_if_dies: 0,
        funding_since: 0,
        adl_score: 0,
    });
    round_trip(&FinancialState {
        format: FinancialFormat(3),
        finalized_height: 9,
        finalized_time_ms: 99,
        accounts: vec![AccountLedger {
            owner: Address([3; 20]),
            balance: Some(1),
            fees_accrued: None,
            fee_override: Some(FeeOverride {
                taker_fee_bps: Some(4),
                maker_fee_bps: None,
            }),
            rolling_volume: Some(RollingVolume {
                last_update_ms: 5,
                volume_micro_usdc: 6,
            }),
            positions: vec![LedgerPosition {
                market: 1,
                side: Side::Sell,
                size: 3,
                entry_price: 7,
                entry_cost: 22,
                pending_funding: -2,
            }],
        }],
        markets: Vec::new(),
        fee_pool_balance: Some(-1),
        insurance_pools: vec![InsurancePool {
            pool_id: 0,
            balance: None,
        }],
    });
}

#[test]
fn proposal_status_round_trips_with_payload() {
    round_trip(&ProposalStatus::Rejected {
        by: Address([4; 20]),
    });
    round_trip(&ProposalStatus::Expired {
        reason: ExpiryReason::Ttl,
    });
}

#[test]
fn proposal_page_round_trips() {
    let page = ProposalPage {
        items: vec![Proposal {
            proposal_id: ProposalId(1),
            status: ProposalStatus::Failed { code: 77 },
            registry_version: RegistryVersion(2),
            threshold: SignatureThreshold(3),
            proposer: Address([5; 20]),
            approvals: vec![Address([5; 20])],
            rejections: Vec::new(),
            created_height: 10,
            created_ms: 11,
            expiry_ms: 12,
            action_tag: 0x0F,
            action_canonical_bytes: vec![1, 2, 3],
            content_hash: Hash32([6; 32]),
        }],
        next_cursor: None,
    };
    round_trip(&page);
}

#[test]
fn oracle_permissions_round_trip() {
    round_trip(&OraclePermissions {
        market: 1,
        finalized_height: 10,
        permission: Some(OraclePermission::Satisfied),
        certified: Some(CertifiedPrice {
            price: 100,
            provider_time_ms: 8,
        }),
        reason: None,
    });
    round_trip(&OraclePermissions {
        market: 1,
        finalized_height: 10,
        permission: Some(OraclePermission::Unavailable),
        certified: None,
        reason: Some(OracleReason::SessionClosed),
    });
}

#[test]
fn configuration_views_round_trip() {
    round_trip(&LiquidationConfig {
        target_ratio_bps: 10_500,
        full_close_threshold: Ratio {
            numerator: 2,
            denominator: 3,
        },
        partial_penalty_bps: 100,
        takeover_discount_bps: 50,
    });
    round_trip(&Upgrades {
        upgrades: vec![
            Upgrade {
                release: Some(EngineRelease { major: 9, minor: 0 }),
                status: UpgradeStatus::Activated {
                    activated_height: 50,
                },
            },
            Upgrade {
                release: None,
                status: UpgradeStatus::Scheduled {
                    target_height: 100,
                    successor_sha256: Hash32([7; 32]),
                },
            },
        ],
    });
    round_trip(&SignerRegistryInfo { registry: None });
    round_trip(&Funding {
        market: 1,
        funding_rate_bps: None,
        cumulative_funding: Some(-4),
        last_funding_time_ms: None,
        mark_price_ewma: None,
        oracle_price: Some(5),
        funding_interval_ms: Some(3_600_000),
        max_funding_rate_bps: 100,
    });
    round_trip(&Withdrawal {
        withdrawal_id: 1,
        owner: Address([8; 20]),
        amount: 5,
        solana_destination: SolanaAddress([9; 32]),
        status: WithdrawalStatus::Pending,
        request_height: 4,
    });
}
