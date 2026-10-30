//! Node query responses: every type a node query serializes for clients.
//! Define new response types here, never in exchange-core, so the engine,
//! SDK and gateway share one definition.
//!
//! Positional MessagePack. Append new fields at the end with
//! `#[serde(default)]` so new readers decode old bytes; older typed readers
//! still refuse the longer array.

use serde::{Deserialize, Serialize};

use crate::types::{
    AdminAction, Branch, EmergencyActionRecord, EmergencyId, EventId, EventInfo, FeeTier,
    MarketConfig, MarketId, Position, PositionEpoch, ProposalId, ProposalStatus, RegistryVersion,
    Side, SignatureThreshold, SignerAddress,
};

/// Position with response-only enrichment fields computed inline by
/// `query_account`. The first six fields match the on-state `Position`
/// struct in identical order, so clients decoding positional msgpack
/// tuples by index (0..=5) see unchanged values. Indices 6..=13 are
/// new and OK to ignore for older clients — rmp-serde arrays are
/// length-prefixed and TS/JS decoders access by numeric index.
///
/// The enrichment fields are deliberately per-leg anchors, not a decomposed
/// account-health proof. A conditional marks at its own book, clamped to the
/// band around the underlying, so clients cannot reconstruct
/// `AccountInfo.equity`, `total_mm`, or `total_im` by summing PositionBriefs;
/// those account fields are the canonical scenario result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionBrief {
    // ── Raw Position fields (indices 0-5; wire-compatible with Position) ──
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub market: MarketId,
    pub side: Side,
    pub entry_price: u64,
    pub size: u64,
    pub last_funding_index: i64,

    // ── Response-enrichment fields (indices 6-11) ──
    /// Unrealized P&L at the current mark, unconditional. For Perp + CP
    /// this equals `pnl_if_fires` (the firing settle price is the same as
    /// the current mark). For PredictionBinary there is no true
    /// "current mark" — returned as 0 placeholder; use pnl_if_fires /
    /// pnl_if_dies for the full payoff picture.
    pub upnl_now: i64,
    /// Stand-alone maintenance-margin anchor at the position's own current
    /// mark. It does not include resting-order reservations or the binding
    /// account scenario. For CP/Binary the position contributes zero in its
    /// non-firing branch. Use `AccountInfo.total_mm` for the enforceable
    /// account requirement.
    pub mm_now: u64,
    /// Stand-alone initial-margin anchor, with the same non-additive contract
    /// as `mm_now`. Resting orders and the binding scenario live only in
    /// `AccountInfo.total_im`.
    pub im_now: u64,
    /// V(p, s) when position's branch fires (μ = 1):
    ///     sign(side) × (settle × size − entry × size)
    /// Perp: always fires. CP: fires when event resolves to position's branch.
    /// Binary: fires and settles to BINARY_PRICE_MAX × size. This is a per-leg
    /// payoff anchor; it excludes resting-order reservations.
    pub pnl_if_fires: i64,
    /// V(p, s) when position's branch does NOT fire (μ = 0). Per-kind:
    ///   * **ConditionalPerp**: 0 (voided side returns IM, no settlement
    ///     cash flow).
    ///   * **PredictionBinary**: `sign(side) × (0 − entry × size)`
    ///     (binaries DO settle at $0 on the losing side — long forfeits
    ///     entry × size, short keeps it).
    ///   * **Perp**: equal to `upnl_now` (perps never die; reported here
    ///     for API symmetry so the UI renders the column uniformly).
    pub pnl_if_dies: i64,
    /// Funding accrued since the position's last settled funding index.
    /// Positive = credit to owner, negative = debit. Convention:
    ///     funding_since = -sign(side) × (cumulative - last_index) × size
    ///                      / FUNDING_SCALE
    /// Longs pay when cumulative rises, shorts receive. For CP, funding
    /// is tracked on the CP market (not the underlying).
    pub funding_since: i64,

    /// ADL queue score: `max(0, upnl_now) × leverage_used`. Used by the
    /// Tier-3 Auto-Deleveraging queue to rank profitable counterparties.
    /// Higher score = closer to the front of the ADL queue.
    ///
    /// Formula: `(positive_upnl × notional × 10_000) / im_now`, where
    /// notional = settle_price × size and im_now = notional × im_bps
    /// / 10_000. The notional cancels, so the score reduces to
    /// `upnl × 10⁸ / im_bps`: within a market the queue is ordered by
    /// absolute unrealized profit; across markets, equal profits in a
    /// lower-IM market rank higher. The account's actual leverage
    /// (notional / equity) does not enter the ranking. `im_now` is this
    /// position's stand-alone current-mark anchor, so this per-position ADL
    /// heuristic does not encode resting-order reservations or the binding
    /// account scenario.
    ///
    /// Returned as 0 for losing positions (they're never ADL'd — only
    /// the cascading-counterparty's profitable opposites are at risk).
    /// Returned as 0 when `im_now == 0` (for example prediction-binary
    /// positions, whose payoff lives in scenario equity), avoiding division
    /// by zero.
    ///
    /// Trailing field (index 11), `#[serde(default)]` so older clients
    /// reading indices 0-10 continue to work. Added 2026-04-25.
    #[serde(default)]
    pub adl_score: i64,

    /// Persistent generation of this live position. This is the canonical
    /// first-attach input for `SetPositionTriggers.expected_position_epoch`:
    /// clients must never guess `1`, because close/reopen and direct flips
    /// advance this sidecar.
    ///
    /// `None` is retained only for pre-index legacy state while the bounded
    /// backfill has not reached the row. Trailing field (index 13); existing
    /// positional decoders keep indices 0..=12 unchanged and may ignore it.
    #[serde(default)]
    pub position_epoch: Option<PositionEpoch>,
}

/// One event's outcome in the binding scenario: a two-element msgpack
/// array, byte-identical to the `(EventId, Branch)` pair it replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingScenario {
    pub event_id: EventId,
    pub branch: Branch,
}

/// Account info response — includes margin breakdown for liquidation bar.
#[derive(Serialize, Deserialize)]
pub struct AccountInfo {
    pub balance: u64,
    pub positions: Vec<PositionBrief>,
    /// Equity in the binding liquidation-ratio scenario. Signed (can be negative).
    pub equity: i64,
    /// Maintenance margin in the same binding scenario as `equity`.
    pub total_mm: u64,
    /// Total initial margin required (positions + resting orders).
    pub total_im: u64,
    /// Margin ratio = binding-scenario equity / binding-scenario total_mm. 0 if no positions.
    /// 1.0 = at liquidation threshold. >1.0 = healthy. Encoded as bps (10000 = 1.0x).
    pub margin_ratio_bps: u64,
    /// Resolution scenario with the lowest paired maintenance ratio — the
    /// "binding" liquidation-health outcome. One entry per active event the
    /// account touches through a conditional, in ascending event-id order;
    /// empty for an account with no conditional exposure. A trailing msgpack
    /// field (index 6); older SDK versions reading indices 0-5 continue to
    /// work.
    pub binding_scenario: Vec<BindingScenario>,
    /// Cumulative trading fees paid (positive) or rebates received
    /// (negative) by this account across its lifetime in micro-USDC.
    /// Updated atomically with the FeePool credit at fill time, so
    /// `Σ fees_accrued ≈ current FeePool balance` holds. Powers the
    /// "lifetime trading cost" line on the UI/SDK. New accounts read
    /// 0. Trailing field (index 7); decodes as 0 for callers reading
    /// older AccountInfo blobs thanks to `serde(default)`.
    #[serde(default)]
    pub fees_accrued: i64,
    /// Per-account rolling 30-day taker volume in micro-USDC at the last
    /// volume update. The engine uses this slot to select volume fee tiers
    /// before each fill and updates it after taker volume is recorded. New
    /// accounts read 0. Trailing field (index 8); older clients may ignore it.
    #[serde(default)]
    pub volume_30d_micro_usdc: u64,
    /// Mark-to-market account value in microUSDC for portfolio charts.
    ///
    /// Unlike `equity`, this does not select a binding resolution scenario.
    /// It uses oracle marks for perps and child-market marks for event books,
    /// nets the perps' unsettled funding, and is intended for "what could I
    /// cash out around now?" display surfaces.
    /// Child marks are raw trade EWMAs, not the clamped scenario marks.
    /// This display is neither guaranteed execution proceeds nor a
    /// conservative bound on scenario equity.
    /// Trailing field (index 9); older clients may ignore it.
    #[serde(default)]
    pub cashout_equity: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizedHeight(pub u64);

#[derive(Serialize, Deserialize)]
pub struct LevelSummary {
    pub price: u64,
    pub total_quantity: u64,
    pub order_count: u32,
}

#[derive(Serialize, Deserialize)]
pub struct OrderbookSnapshot {
    pub bids: Vec<LevelSummary>,
    pub asks: Vec<LevelSummary>,
}

/// Fee schedule read model for one market.
///
/// This intentionally includes both the flat fallback fees and the
/// volume-tier table. MM integrators can tell the difference between a
/// market with no volume tiers and one whose first tier starts at zero
/// without decoding the full `MarketConfig` registry.
#[derive(Serialize, Deserialize)]
pub struct FeeTierInfo {
    pub market: MarketId,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    pub fee_tiers: Vec<FeeTier>,
}

/// Raw finalized ledger facts only. No mark, funding accrual, equity, margin,
/// withdrawal capacity, or trading authorization is implied by this response.
#[derive(Serialize, Deserialize)]
pub struct AccountStateInfo {
    pub owner: SignerAddress,
    pub finalized_height: FinalizedHeight,
    pub balance: u64,
    pub positions: Vec<Position>,
}

/// The operator-authorization sets as concatenated addresses (#422 / #318):
/// the three genesis presence sets plus the three capability sets. Read-only
/// audit surface — until now the allowlists could only be membership-tested,
/// never enumerated.
#[derive(Serialize, Deserialize)]
pub struct AuthoritiesSnapshot {
    pub relayer: Vec<SignerAddress>,
    pub oracle: Vec<SignerAddress>,
    pub cex_composite: Vec<SignerAddress>,
    pub custody: Vec<SignerAddress>,
    pub market_params: Vec<SignerAddress>,
    pub scheduled_ops: Vec<SignerAddress>,
}

/// Funding info response for a single market.
#[derive(Serialize, Deserialize)]
pub struct FundingInfo {
    pub market: MarketId,
    /// Current funding rate in basis points (signed). 0 if no funding has run.
    pub funding_rate_bps: i64,
    /// Cumulative funding index (sum of all funding rates × prices).
    pub cumulative_funding: i64,
    /// Last funding timestamp in milliseconds. 0 if never funded.
    pub last_funding_time_ms: u64,
    /// EWMA mark price. 0 if no trades yet.
    pub mark_price_ewma: u64,
    /// Oracle price. 0 if not set.
    pub oracle_price: u64,
    /// Funding interval from market config.
    pub funding_interval_ms: u64,
    /// Max funding rate from market config.
    pub max_funding_rate_bps: u32,
}

/// One row of the ADL queue response — a profitable position on a
/// specific market, sorted by `adl_score` desc.
///
/// The 6-tuple wire shape mirrors the documented `AdlQueueEntry` SDK
/// type; appending fields is supported via `#[serde(default)]` in the
/// SDK decoder.
#[derive(Serialize, Deserialize)]
pub struct AdlQueueEntry {
    pub owner: [u8; 20],
    pub market: MarketId,
    pub side: Side,
    pub size: u64,
    /// Unrealized PnL at the current mark, signed.
    pub upnl_now: i64,
    /// `max(0, upnl_now) × leverage_bps` — same formula as
    /// PositionBrief.adl_score so per-account UI rendering and the
    /// fleet-wide ADL queue use a single ranking definition.
    pub adl_score: i64,
}

/// The installed admin signer registry, as stored.
#[derive(Serialize, Deserialize)]
pub struct AdminSignerRegistryRoster {
    pub version: RegistryVersion,
    pub threshold: SignatureThreshold,
    pub members: Vec<SignerAddress>,
}

/// Signer-registry read. `None` = no registry installed, meaning
/// multisig administration is disabled.
#[derive(Serialize, Deserialize)]
pub struct AdminSignerRegistryInfo {
    pub registry: Option<AdminSignerRegistryRoster>,
}

/// One proposal, shaped for display and offline verification. The
/// context fields plus `action_canonical_bytes` are exactly what the
/// offline signer needs to rebuild an `ApproveAdminAction` without
/// trusting anything the Web Admin rendered.
#[derive(Serialize, Deserialize)]
pub struct ProposalDisplayInfo {
    pub proposal_id: ProposalId,
    /// Status as written by the handlers.
    pub status_stored: ProposalStatus,
    /// One published rule, applied engine-side: a stored `Pending`
    /// past its `expiry_ms` (relative to the last committed block
    /// time) reads as `Expired { Ttl }`. The `status` filter matches
    /// THIS field, so a status-filtered page is never wrong by
    /// staleness, and no two readers can disagree by applying
    /// different client-side rules.
    pub status_effective: ProposalStatus,
    pub registry_version: RegistryVersion,
    pub threshold: SignatureThreshold,
    pub proposer: SignerAddress,
    pub approvals: Vec<SignerAddress>,
    pub rejections: Vec<SignerAddress>,
    pub created_height: u64,
    pub created_ms: u64,
    pub expiry_ms: u64,
    pub action_tag: u8,
    /// Typed decode of the stored action, for display. `None` when the stored
    /// bytes predate a breaking change to that action's wire shape; they stay
    /// in `action_canonical_bytes`.
    pub action: Option<AdminAction>,
    /// The EXACT stored engine-canonical bytes the content hash covers.
    pub action_canonical_bytes: Vec<u8>,
    pub content_hash: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct ProposalPage {
    pub items: Vec<ProposalDisplayInfo>,
    /// Last-seen proposal id; resubmit as `cursor` for the next page.
    pub next_cursor: Option<ProposalId>,
}

#[derive(Serialize, Deserialize)]
pub struct EmergencyActionPage {
    pub items: Vec<EmergencyActionRecord>,
    /// Last-seen emergency id; resubmit as `cursor` for the next page.
    pub next_cursor: Option<EmergencyId>,
}

/// Atomic same-height read for the markets read-model poller: one
/// committed state view yields the chain id, the height, both market
/// registries and per-market open interest in a single response — nothing
/// to bracket, no torn snapshot possible.
#[derive(Serialize, Deserialize)]
pub struct MarketsSnapshot {
    pub chain_id: [u8; 32],
    pub height: u64,
    pub markets: Vec<MarketConfig>,
    pub events: Vec<EventInfo>,
    /// One entry per market that has had positions, ascending market id; an
    /// absent market has none.
    #[serde(default)]
    pub open_interest: Vec<MarketOpenInterest>,
}

/// One market's open interest, in base-asset units per side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpenInterest {
    pub market: MarketId,
    pub long: u64,
    pub short: u64,
}

/// The pending protocol-upgrade plan, shaped for display.
#[derive(Serialize, Deserialize)]
pub struct PendingUpgradePlanInfo {
    pub target_height: u64,
    pub major: u32,
    pub minor: u32,
    pub successor_sha256: [u8; 32],
    pub scheduled_height: u64,
}

/// One executed upgrade from the append-only activation ledger.
#[derive(Serialize, Deserialize)]
pub struct ExecutedUpgradeInfo {
    pub activated_height: u64,
    pub major: u32,
    pub minor: u32,
}

/// Upgrade-plan read: the one pending plan (state source, written by a
/// governed `ScheduleUpgrade`) and the executed-activation ledger.
#[derive(Serialize, Deserialize)]
pub struct UpgradesInfo {
    pub plan: Option<PendingUpgradePlanInfo>,
    pub executed: Vec<ExecutedUpgradeInfo>,
}

/// Per-market oracle status: the last accepted publish timestamp.
/// Used by the node's Prometheus exporter to surface oracle freshness without
/// going through the indexer. Only `publish_time_ms` is needed for the gauge;
/// `oracle_price` is not read, halving the per-market state reads on the
/// consensus Commit path.
#[derive(Serialize, Deserialize)]
pub struct OracleStatusEntry {
    pub market: MarketId,
    pub publish_time_ms: Option<u64>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn binding_scenario_entry_is_the_pair_it_replaces() {
        let entry = BindingScenario {
            event_id: EventId(42),
            branch: Branch::Yes,
        };
        assert_eq!(
            rmp_serde::to_vec(&entry).expect("entry serializes"),
            rmp_serde::to_vec(&(EventId(42), Branch::Yes)).expect("pair serializes")
        );
    }
}
