//! Version 2 node read responses: public views encoded as MessagePack maps keyed by field name.
//! A view carries only public data, uses `Option` for absence, and ends each enum with `Unknown`,
//! so readers skip keys and variants they do not know and storage can change underneath.

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::types::{
    ClientTriggerGroupId, ClientTriggerId, EventId, FundingRate, MarketId, OrderId, PositionEpoch,
    ProposalId, RegistryVersion, SignatureThreshold, TriggerSlippageBps,
};

pub fn encode<T: Serialize>(view: &T) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    rmp_serde::to_vec_named(view)
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, rmp_serde::decode::Error> {
    rmp_serde::from_slice(bytes)
}

/// A 20-byte account address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Address(#[serde(with = "serde_bytes")] pub [u8; 20]);

/// A 32-byte hash, digest or chain id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash32(#[serde(with = "serde_bytes")] pub [u8; 32]);

/// A 32-byte Solana account address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SolanaAddress(#[serde(with = "serde_bytes")] pub [u8; 32]);

/// A sub-account's 32-byte label, as its owner set it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubAccountName(#[serde(with = "serde_bytes")] pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Branch {
    Yes,
    No,
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// Markets and events
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketKind {
    Perpetual,
    Conditional {
        event_id: EventId,
        branch: Branch,
    },
    PredictionBinary {
        event_id: EventId,
        branch: Branch,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkSource {
    OracleOnly,
    Median,
    #[serde(other)]
    Unknown,
}

/// How a perpetual's mark price is derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkConfig {
    pub source: MarkSource,
    /// Oldest oracle reading a margin read accepts; `None` means the guard is unset and the
    /// market has no mark until governance sets it.
    pub max_oracle_age_ms: Option<u64>,
    /// Top-of-book spread above which the book mid leaves the median.
    pub max_spread_bps: u32,
    /// Oldest composite exchange price the median accepts.
    pub cex_composite_max_age_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeTier {
    pub min_30d_volume_micro_usdc: u64,
    /// Negative is a maker rebate.
    pub maker_fee_tenth_bps: i16,
    pub taker_fee_tenth_bps: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Market {
    pub market: MarketId,
    pub ticker: Option<String>,
    pub kind: MarketKind,
    pub sz_decimals: u8,
    pub tick_size: Option<u64>,
    pub lot_size: Option<u64>,
    pub im_bps: u32,
    pub mm_bps: u32,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    pub fee_tiers: Vec<FeeTier>,
    /// `None` when the market pays no funding.
    pub funding_interval_ms: Option<u64>,
    pub max_funding_rate_bps: u32,
    pub max_position_size: Option<u64>,
    pub max_open_interest: Option<u64>,
    /// Resting orders older than this are cancelled; `None` means they rest until cancelled.
    pub default_ttl_ms: Option<u64>,
    /// `None` for event books, which mark off their own book.
    pub mark: Option<MarkConfig>,
    pub status: MarketStatus,
    /// Effective funding dead-band half-width in bps; `None` when the market pays no funding.
    pub funding_dead_band_bps: Option<u32>,
    /// Effective funding interest leg per 8h in units of 1e-8; `None` when the market pays no funding.
    pub funding_interest_rate: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketStatus {
    Trading,
    ReduceOnly,
    Halted,
    Delisted,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Trading,
    PreResolution,
    Resolved,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Yes,
    No,
    Void,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceComparison {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolutionSource {
    /// Yes when `market`'s oracle price compares to `strike_price` as `comparison` says.
    MarketOracle {
        market: MarketId,
        strike_price: u64,
        comparison: PriceComparison,
    },
    RelayerAttested,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedConditional {
    pub underlying_market: MarketId,
    pub yes_market: MarketId,
    pub no_market: MarketId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub event_id: EventId,
    pub binary_market: MarketId,
    pub question: String,
    pub settlement_ms: u64,
    pub resolution_window_ms: u64,
    pub status: EventStatus,
    /// Set once the event resolves.
    pub outcome: Option<Outcome>,
    pub created_ms: u64,
    pub resolved_ms: Option<u64>,
    pub resolution_source: ResolutionSource,
    pub attached_conditionals: Vec<AttachedConditional>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpenInterest {
    pub market: MarketId,
    pub long: u64,
    pub short: u64,
}

/// Every market, event and open interest from one committed state view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketsSnapshot {
    pub chain_id: Hash32,
    pub height: u64,
    pub markets: Vec<Market>,
    pub events: Vec<Event>,
    pub open_interest: Vec<MarketOpenInterest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeSchedule {
    pub market: MarketId,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    pub fee_tiers: Vec<FeeTier>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funding {
    pub market: MarketId,
    /// Whole bps, truncated toward zero; `None` until the first funding tick.
    pub funding_rate_bps: Option<i64>,
    pub cumulative_funding: Option<i64>,
    pub last_funding_time_ms: Option<u64>,
    /// Written by fills and by the funding tick's re-anchor toward the oracle; `None` until
    /// either writes it.
    pub mark_price_ewma: Option<u64>,
    pub oracle_price: Option<u64>,
    pub funding_interval_ms: Option<u64>,
    pub max_funding_rate_bps: u32,
    /// Exact rate of the last funding interval; `None` until the first funding tick.
    #[serde(default)]
    pub funding_rate: Option<FundingRate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: u64,
    pub total_quantity: u64,
    pub order_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orderbook {
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

/// A profitable position ranked for auto-deleveraging, highest score first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdlQueueEntry {
    pub owner: Address,
    pub market: MarketId,
    pub side: Side,
    pub size: u64,
    pub upnl_now: i64,
    pub adl_score: i64,
}

// ---------------------------------------------------------------------------
// Orders, withdrawals, sub-accounts
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub order_id: OrderId,
    pub market: MarketId,
    pub owner: Address,
    pub side: Side,
    pub price: u64,
    pub remaining_quantity: u64,
    pub original_quantity: Option<u64>,
    pub filled_quantity: u64,
    pub client_order_id: Option<u64>,
    pub created_ms: Option<u64>,
    pub post_only: bool,
    pub reduce_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalStatus {
    Pending,
    Completed,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Withdrawal {
    pub withdrawal_id: u64,
    pub owner: Address,
    pub amount: u64,
    pub solana_destination: SolanaAddress,
    pub status: WithdrawalStatus,
    pub request_height: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubAccount {
    pub master: Address,
    pub sub_account_id: u32,
    pub address: Address,
    pub name: SubAccountName,
    pub created_height: u64,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// One position valued at its own current mark. Per-position margin figures do not
/// sum to the account's; those are the binding-scenario values on [`Account`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionSummary {
    pub market: MarketId,
    pub side: Side,
    pub entry_price: u64,
    pub size: u64,
    /// The position's epoch generation; `None` on a legacy position whose
    /// epoch sidecar row has not been backfilled.
    pub position_epoch: Option<PositionEpoch>,
    /// `None` for prediction binaries, which have no current mark.
    pub upnl_now: Option<i64>,
    pub mm_now: u64,
    pub im_now: u64,
    pub pnl_if_fires: i64,
    pub pnl_if_dies: i64,
    pub funding_since: i64,
    pub adl_score: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingOutcome {
    pub event_id: EventId,
    pub branch: Branch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub balance: u64,
    pub equity: i64,
    pub total_mm: u64,
    pub total_im: u64,
    /// Binding-scenario equity over maintenance margin; `None` when no scenario carries risk.
    pub margin_ratio_bps: Option<u64>,
    pub binding_scenario: Vec<BindingOutcome>,
    pub fees_accrued: i64,
    pub volume_30d_micro_usdc: u64,
    pub cashout_equity: i64,
    pub positions: Vec<PositionSummary>,
}

/// A position as the ledger holds it, with the funding it owes or is owed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerPosition {
    pub market: MarketId,
    pub side: Side,
    pub size: u64,
    /// Average entry price, rounded down.
    pub entry_price: u64,
    /// Total entry cost in µUSDC, rounded up.
    pub entry_cost: u64,
    /// Funding accrued since the position last settled, to the owner; negative is owed.
    pub pending_funding: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub owner: Address,
    pub finalized_height: u64,
    pub balance: u64,
    pub positions: Vec<LedgerPosition>,
}

/// Current stored trading-agent grants. Grants have no stored expiry or per-action scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedAgents {
    pub owner: Address,
    pub finalized_height: u64,
    /// Strictly increasing address order, with at most `AUTHORIZED_AGENTS_LIMIT` entries.
    pub agents: Vec<Address>,
    /// False when further grants exist beyond the bounded result.
    pub complete: bool,
}

pub const AUTHORIZED_AGENTS_LIMIT: usize = 100;

/// An account's fee override; `None` on a side means the market's fee applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeOverride {
    pub taker_fee_bps: Option<u32>,
    pub maker_fee_bps: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollingVolume {
    pub last_update_ms: u64,
    pub volume_micro_usdc: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountLedger {
    pub owner: Address,
    pub balance: Option<u64>,
    pub fees_accrued: Option<i64>,
    pub fee_override: Option<FeeOverride>,
    pub rolling_volume: Option<RollingVolume>,
    pub positions: Vec<LedgerPosition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketLedger {
    pub market: MarketId,
    pub pool_id: u8,
    pub sz_decimals: u8,
    pub maker_fee_bps: u32,
    pub taker_fee_bps: u32,
    pub fee_tiers: Vec<FeeTier>,
    pub funding_interval_ms: Option<u64>,
    pub max_funding_rate_bps: u32,
    pub cumulative_funding: Option<i64>,
    pub funding_rate_bps: Option<i64>,
    pub last_funding_time_ms: Option<u64>,
    pub mark_price_ewma: Option<u64>,
    /// Exact rate of the last funding interval.
    #[serde(default)]
    pub funding_rate: Option<FundingRate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsurancePool {
    pub pool_id: u8,
    pub balance: Option<i64>,
}

/// The ledger layout a [`FinancialState`] projects. Numbered with the v1
/// positional ledger formats it is read from, so a reader can refuse a
/// snapshot whose layout it does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FinancialFormat(pub u8);

/// Selected raw ledgers at one finalized height.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinancialState {
    pub format: FinancialFormat,
    pub finalized_height: u64,
    pub finalized_time_ms: u64,
    pub accounts: Vec<AccountLedger>,
    pub markets: Vec<MarketLedger>,
    pub fee_pool_balance: Option<i64>,
    pub insurance_pools: Vec<InsurancePool>,
}

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerLimbState {
    Armed,
    Filled,
    Partial,
    NoFill,
    Rejected,
    Cancelled,
    Invalidated,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerOutcomeReason {
    PositionClosed,
    PositionSideChanged,
    BelowMaintenance,
    IndeterminateAccount,
    MarkUnavailable,
    MarkStale,
    NoEligibleLiquidity,
    ExecutionRejected,
    OracleSessionClosed,
    OracleExpiredSource,
    OraclePolicyMissing,
    InsufficientMargin,
    InsufficientBalance,
    ReduceOnlyWouldIncrease,
    PositionLimitExceeded,
    OpenInterestLimitExceeded,
    MarketClosed,
    EngineFault,
    MarketHalted,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerOutcome {
    pub requested_quantity: u64,
    pub filled_quantity: u64,
    pub reason: Option<TriggerOutcomeReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerLimbInfo {
    pub trigger_price: u64,
    pub max_slippage_bps: TriggerSlippageBps,
    pub client_trigger_id: Option<ClientTriggerId>,
    pub state: TriggerLimbState,
    pub last_outcome: Option<TriggerOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionTriggers {
    pub owner: Address,
    pub market: MarketId,
    pub position_epoch: PositionEpoch,
    pub position_side: Side,
    pub client_group_id: Option<ClientTriggerGroupId>,
    pub stop_loss: Option<TriggerLimbInfo>,
    pub take_profit: Option<TriggerLimbInfo>,
}

// ---------------------------------------------------------------------------
// Oracle permissions
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OraclePermission {
    Satisfied,
    Unavailable,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleReason {
    InvalidPolicy,
    UpdateLimitExceeded,
    ClockRegression,
    ManualHalt,
    SessionUnknown,
    SessionClosed,
    BadQuality,
    ReferenceUnavailable,
    MovementBound,
    MissingSource,
    ExpiredSource,
    OutsideSession,
    RecoveryPending,
    Fresh,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedPrice {
    pub price: u64,
    pub provider_time_ms: u64,
}

/// Whether the oracle permits trading `market` at the finalized height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OraclePermissions {
    pub market: MarketId,
    pub finalized_height: u64,
    pub permission: OraclePermission,
    /// The price trading may use at this height.
    pub certified: Option<CertifiedPrice>,
    /// Why trading is not permitted; `None` when permitted or no verdict is committed.
    pub reason: Option<OracleReason>,
}

// ---------------------------------------------------------------------------
// Governance, upgrades, configuration
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ratio {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationConfig {
    pub target_ratio_bps: u32,
    /// Accounts below this equity-to-maintenance ratio close in full.
    pub full_close_threshold: Ratio,
    pub partial_penalty_bps: u32,
    pub takeover_discount_bps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpiryReason {
    Ttl,
    RegistryChanged,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Executed,
    Failed {
        code: u32,
    },
    Rejected {
        by: Address,
    },
    Expired {
        reason: ExpiryReason,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub proposal_id: ProposalId,
    /// A pending proposal past its expiry reads as expired.
    pub status: ProposalStatus,
    pub registry_version: RegistryVersion,
    pub threshold: SignatureThreshold,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub rejections: Vec<Address>,
    pub created_height: u64,
    pub created_ms: u64,
    pub expiry_ms: u64,
    pub action_tag: u8,
    #[serde(with = "serde_bytes")]
    pub action_canonical_bytes: Vec<u8>,
    pub content_hash: Hash32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalPage {
    pub items: Vec<Proposal>,
    pub next_cursor: Option<ProposalId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineRelease {
    pub major: u32,
    pub minor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpgradeStatus {
    /// The successor engine executes from block `target_height` on.
    Scheduled {
        target_height: u64,
        /// The sha256 of the successor engine library.
        successor_sha256: Hash32,
    },
    Activated {
        activated_height: u64,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upgrade {
    /// `None` for a plan stored before plans named their release.
    pub release: Option<EngineRelease>,
    pub status: UpgradeStatus,
}

/// Activated upgrades in activation order, then the scheduled one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upgrades {
    pub upgrades: Vec<Upgrade>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerRegistry {
    pub version: RegistryVersion,
    pub threshold: SignatureThreshold,
    pub members: Vec<Address>,
}

/// `registry` is `None` while multisig administration is disabled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignerRegistryInfo {
    pub registry: Option<SignerRegistry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorities {
    pub relayer: Vec<Address>,
    pub oracle: Vec<Address>,
    pub cex_composite: Vec<Address>,
    pub custody: Vec<Address>,
    pub market_params: Vec<Address>,
    pub scheduled_ops: Vec<Address>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;
