//! Core domain types for the Proof exchange engine.
//!
//! All wire-format structs use MessagePack serialization (field-order dependent).
//! Monetary values are in **micro-USDC** (1 USDC = 1_000_000) unless otherwise noted.
//! Prices are unsigned 64-bit integers in quote-currency micro-units.
//!
//! ## `#[serde(with = "crate::wire_bytes")]` on byte fields
//!
//! Address/key/signature fields (`[u8; N]`, `Vec<u8>`) carry this attribute so
//! they are **format-aware**: on the wire (rmp-serde) they encode exactly as a
//! bare array — the bytes are unchanged, so this is *not* a wire change — but on
//! human-readable formats (the pyo3/wasm SDK bindings' `pythonize` /
//! `serde_wasm_bindgen`) they decode to a native `bytes` / `Uint8Array` instead
//! of a tuple of ints. Downstream effect: **Python/JS SDK callers get real byte
//! objects**, not `(0, 1, 2, …)`. The field type stays `[u8; N]`, so engine code
//! is untouched. See [`crate::wire_bytes`]. Add it to any new byte field that
//! should surface as bytes to a binding.

use core::fmt;

use proof_wire_derive::AbciEvent;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Auto-incrementing order identifier, unique across all markets.
pub type OrderId = u64;
/// Numeric market identifier (e.g., 1 = BTC-USD perp).
pub type MarketId = u32;
/// Auto-incrementing fill identifier, unique across all trades.
pub type FillId = u64;
/// Impact market family identifier (1 family owns 4 child markets — CPY/CPN/EBY/EBN).
pub type ImpactMarketId = u32;

/// Identifier of a standalone event (G17 re-root), structurally distinct
/// from [`ImpactMarketId`] per the domain-newtype convention. `#[serde(transparent)]`
/// so it encodes as a bare `u32`, keeping the wire unchanged. On DevNet the
/// underlying value space is currently shared with families (a legacy family's
/// event reuses its id value); distinct allocation is a follow-up.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
)]
#[serde(transparent)]
pub struct EventId(pub u32);

/// Well-known market ID for the BTC-USD perpetual.
pub const MARKET_BTC_USD_PERP: MarketId = 1;

/// Maximum price (in micro-USDC) for a prediction-binary share. $1.00 = 1_000_000 µUSDC.
pub const BINARY_PRICE_MAX: u64 = 1_000_000;

/// Published size scale for prediction-binary books (EBY/EBN). Fixed at 2dp
/// (hundredths of a contract) rather than inheriting the underlying perp's
/// scale — this matches the conditional-market convention on Kalshi (whole→2dp
/// fixed-point contracts) and Polymarket (2 size-decimals). Conditional perps
/// (CPY/CPN) still inherit the underlying's scale; only the binary legs are
/// pinned here. The engine honors this scale: a position of `size = 100` is
/// one whole $1 contract, and notional/PnL/margin divide by `10^2` (see
/// `MarketConfig::sz_decimals` and `engine::size_scale`).
pub const PREDICTION_BINARY_SZ_DECIMALS: u8 = 2;

/// Size lot for prediction-binary books (EBY/EBN): one unit at the 2dp size
/// scale = 0.01 share. Derived from [`PREDICTION_BINARY_SZ_DECIMALS`], NOT
/// inherited from the underlying perp (whose `lot_size` is tuned to its own
/// asset scale — e.g. a BTC lot would force binary orders into whole-share
/// multiples). Unlike `sz_decimals`, `lot_size` IS execution-relevant: the
/// order handlers reject `quantity % lot_size != 0`, so this is a real grid,
/// not a display hint. Static so binaries never pick up a coarse underlying.
pub const PREDICTION_BINARY_LOT_SIZE: u64 = 1;

/// Price tick for prediction-binary books (EBY/EBN): 1 µUSDC ($0.000001).
/// Binaries quote in µUSDC over `[0, BINARY_PRICE_MAX]`, so the tick is the
/// µUSDC unit rather than the underlying perp's (price-scaled) tick. Like
/// `lot_size`, `tick_size` is execution-relevant (`price % tick_size != 0` is
/// rejected). Static, not inherited.
pub const PREDICTION_BINARY_TICK_SIZE: u64 = 1;

// ---------------------------------------------------------------------------
// Impact market enums
// ---------------------------------------------------------------------------

/// Which branch of a binary event a conditional/prediction book represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, derive_more::Display)]
pub enum Branch {
    #[display("yes")]
    Yes = 1,
    #[display("no")]
    No = 2,
}

/// Outcome of an impact-market event resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, derive_more::Display)]
pub enum Outcome {
    #[display("yes")]
    Yes = 1,
    #[display("no")]
    No = 2,
    /// Auto-voided (neither branch won — e.g., resolver timeout under the auto-void policy).
    #[display("void")]
    Void = 3,
}

/// BE-54: how the YES/NO outcome of an impact-market event is determined
/// at deadline. Stored on [`ImpactMarketInfo`] (and carried on
/// [`CreateImpactMarket`]). `RelayerAttested` is the legacy default —
/// the resolver supplies the outcome and the engine trusts it. The two
/// auto-resolve modes derive YES/NO from an on-chain oracle reading,
/// turning the relayer-supplied `outcome` field into a verifiable assertion
/// (the engine recomputes and rejects on mismatch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
pub enum EventOracleSource {
    /// Resolution determined by the underlying perp's oracle reading at
    /// `ResolveImpactMarket` time, compared against `strike_price`. The classic
    /// "is BTC above $X at expiry?" pattern.
    #[display("underlying_price_vs_strike:{strike_price}:{comparison}")]
    UnderlyingPriceVsStrike {
        strike_price: u64,
        comparison: PriceComparison,
    },
    /// Resolution determined by a different on-chain market's oracle
    /// (e.g. ETH event whose outcome is gated on BTC's price). The
    /// `market` MUST exist and have a current oracle price at resolution
    /// time; otherwise the resolution is rejected.
    #[display("market_oracle:{market}:{strike_price}:{comparison}")]
    MarketOracle {
        market: MarketId,
        strike_price: u64,
        comparison: PriceComparison,
    },
    /// Resolution by relayer attestation only — the legacy/default path.
    /// The relayer-supplied `outcome` is taken at face value (still subject
    /// to the existing relayer-allowlist signature check). Use for events
    /// where there is no on-chain price (e.g. "did Apple announce X?").
    #[display("relayer_attested")]
    RelayerAttested,
}

/// BE-54: comparison operator used by the auto-resolve oracle modes.
/// `YES` fires iff `oracle_price <comparison> strike_price` (e.g.
/// `GreaterThan` means the event resolves YES when the oracle reading is
/// strictly greater than the strike). Equality on the boundary is
/// distinguished by the `OrEqual` variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
pub enum PriceComparison {
    #[display("gt")]
    GreaterThan,
    #[display("lt")]
    LessThan,
    #[display("gte")]
    GreaterThanOrEqual,
    #[display("lte")]
    LessThanOrEqual,
}

impl PriceComparison {
    /// Apply the comparison: returns true iff the YES branch wins.
    pub fn apply(self, oracle_price: u64, strike_price: u64) -> bool {
        match self {
            PriceComparison::GreaterThan => oracle_price > strike_price,
            PriceComparison::LessThan => oracle_price < strike_price,
            PriceComparison::GreaterThanOrEqual => oracle_price >= strike_price,
            PriceComparison::LessThanOrEqual => oracle_price <= strike_price,
        }
    }

    pub fn phrase(&self) -> &'static str {
        match self {
            PriceComparison::GreaterThan => "strictly greater than",
            PriceComparison::LessThan => "strictly less than",
            PriceComparison::GreaterThanOrEqual => "greater than or equal to",
            PriceComparison::LessThanOrEqual => "less than or equal to",
        }
    }
}

/// Kind of market stored on-chain. Stored on [`MarketConfig`].
///
/// The existing engine paths (matching, funding, liquidation) only care that a
/// market has a CLOB. The kind affects: (a) margin computation — conditional
/// books get branch-conditional max instead of per-book sum; (b) price-range
/// validation — binaries must trade inside `[0, BINARY_PRICE_MAX]`; (c)
/// resolution — conditional books freeze at resolution, binaries settle to
/// `$1` or `$0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarketKind {
    /// Regular perpetual future (BTC-PERP, ETH-PERP, SOL-PERP, etc.).
    #[default]
    Perp,
    /// Conditional perpetual — trades like a perp until the parent event
    /// resolves, then settles (if branch wins) or voids (if branch loses).
    ConditionalPerp {
        impact_market_id: ImpactMarketId,
        branch: Branch,
    },
    /// Prediction-binary token — trades on [0, BINARY_PRICE_MAX] µUSDC.
    /// Settles to $1 if the branch wins at resolution, $0 otherwise.
    /// G17 re-root: parented by an [`EventInfo`] via `event_id`, not a family.
    PredictionBinary { event_id: EventId, branch: Branch },
}

/// Mark-price source for a perp market. Selects how `get_mark_price`
/// derives the mark used for margin checks, liquidation triggers, and
/// unrealized-PnL accounting.
///
/// Defaults to `OracleOnly` (the legacy single-source path) so existing
/// on-chain `MarketConfig` records and freshly-created markets keep
/// today's behavior byte-for-byte. Operators flip individual markets
/// to `Median` via `UpdateMarketFees` once they want the multi-source
/// guard. **No big-bang switch** — each market opts in independently.
///
/// Wire layout: encoded as a positional tag (msgpack) — `OracleOnly`
/// = 0, `Median` = 1. New variants append.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkSourceMode {
    /// Mark = oracle price. Single-source; same as the pre-BE-31
    /// engine. Failure mode: a stale or attacked oracle moves mark
    /// without a second opinion.
    #[default]
    OracleOnly,
    /// Mark = median of available sources (oracle, book-mid, and —
    /// once Phase B lands — composite CEX index). When fewer than 2
    /// sources are available, falls through to the average of 2, then
    /// to the single remaining source. When zero sources are
    /// available, returns `UnknownMarket`.
    ///
    /// The thin-book guard rejects book-mid from the median when the
    /// top-of-book spread exceeds `MarketConfig.max_mark_spread_bps`
    /// (a single $50-spread quote pair on an otherwise empty book
    /// can poison the median otherwise).
    Median,
}

/// Built-in default for the thin-book spread guard, used when
/// `MarketConfig.max_mark_spread_bps` is `0` (the serde default for
/// existing on-chain records). 100 bps = 1% of mid; book-mid is
/// excluded from the median when the top-of-book spread exceeds this.
pub const DEFAULT_MAX_MARK_SPREAD_BPS: u32 = 100;

/// Built-in clamp for impact-market branch shocks used by scenario margin.
///
/// Branch shocks are inferred from child CPY/CPN top-of-book mids. Those
/// books can be thin, so consensus margin must not let one dust quote mark
/// an underlying to an arbitrary price. The shock remains useful for
/// resolution-risk margin, but is bounded to ±25% around the current
/// underlying mark unless a future config field overrides it.
pub const DEFAULT_BRANCH_SHOCK_CLAMP_BPS: u32 = 2_500;

/// Built-in default staleness threshold for the composite-CEX price,
/// used when `MarketConfig.cex_composite_staleness_ms` is `0`. 30s
/// = enough headroom for a 1s feeder cadence to miss a few cycles
/// without dropping out of the median; tight enough that a stuck
/// feeder shows up in the mark drift within minutes.
pub const DEFAULT_CEX_COMPOSITE_STALENESS_MS: u64 = 30_000;

/// W28-06 slice 1: maximum per-update oracle price deviation from the
/// stored last-good, in basis points. `handle_oracle_update` otherwise
/// lands ANY non-zero price an authorized signer submits directly in the
/// mark. Out-of-band updates are CLAMPED to `last_good ± this band` (not
/// rejected), so a genuine large move reprices over successive updates
/// while a single out-of-band spike is bounded to one band step. 2000 bps
/// = 20% per update. The first-ever price on a market (no last-good) is
/// accepted as-is. The behavior activates at the shared stored-schema v9
/// boundary; pre-v9 replay keeps the raw submitted price. (Slice 2 may move
/// this onto per-market `MarketConfig`.)
pub const DEFAULT_MAX_ORACLE_DEVIATION_BPS: u32 = 2_000;

/// W28-06 slice 1: minimum top-of-book notional for a book-mid to be trusted
/// as a mark source, in µUSDC. The floor binds PER SIDE at half this value —
/// each of bid and ask, valued at the mid, must clear
/// `MARK_MIN_BOOK_NOTIONAL_UUSDC / 2` (see `engine::book_depth_meets_floor`
/// for why a summed floor is bypassable with one riskless deep quote plus a
/// dust lot). Below the floor the book is treated as illiquid and the mark
/// falls back to the underlying anchor / remaining median sources. Kills the
/// 1-lot dust-quote attack where a fractional quote inside the spread cap
/// could otherwise drive the branch or median mark and move a solvent third
/// party's margin. $50k aggregate ($25k/side). Compared directly against
/// `engine::notional_micro`'s u128 output. The floor activates at stored schema
/// v9; pre-v9 replay keeps the historical spread-only book-mid rule. (Slice 2
/// may move this onto per-market `MarketConfig`.)
pub const MARK_MIN_BOOK_NOTIONAL_UUSDC: u128 = 50_000_000_000;

/// W28-06 slice 1: hard-cap multiple on `mark_price_max_oracle_age_ms` past
/// which the stale→last-good mark fallback stops and `get_mark_price` errors
/// (`StaleOracle`) again. Two-tier staleness policy: below the per-market
/// gate the oracle is fresh; between the gate and `gate × this factor` the
/// mark falls back to the stored last-good price (liquidation stays live
/// through routine feeder hiccups); past the hard cap the mark errors, which
/// re-freezes margin-gated actions — order placement, withdrawals, and the
/// sweep's per-account deferral — so a multi-hour outage cannot be used to
/// lever up against an arbitrarily old price. Withdrawal initiation is stricter
/// and rejects immediately after the configured freshness gate. Both policies
/// activate at stored schema v9; pre-v9 replay retains immediate staleness
/// rejection for every mark consumer. (Slice 2 may move this onto per-market
/// `MarketConfig`.)
pub const DEFAULT_STALE_LAST_GOOD_HARD_CAP_FACTOR: u64 = 10;

impl MarketKind {
    /// Returns the parent id this market belongs to, if any. For a
    /// conditional perp that is its impact-market family; for a prediction
    /// binary that is its [`EventInfo`] (G17 re-root). On DevNet the two id
    /// spaces share values, so a legacy family's binary still resolves to it.
    pub fn impact_market_id(&self) -> Option<ImpactMarketId> {
        match self {
            MarketKind::Perp => None,
            MarketKind::ConditionalPerp {
                impact_market_id, ..
            } => Some(*impact_market_id),
            MarketKind::PredictionBinary { event_id, .. } => Some(event_id.0),
        }
    }

    /// The event this market is parented by, if it is a prediction binary
    /// (G17 re-root). `None` for perps and conditional perps.
    pub fn event_id(&self) -> Option<EventId> {
        match self {
            MarketKind::PredictionBinary { event_id, .. } => Some(*event_id),
            _ => None,
        }
    }

    /// Returns the branch this market is tied to, if any.
    pub fn branch(&self) -> Option<Branch> {
        match self {
            MarketKind::Perp => None,
            MarketKind::ConditionalPerp { branch, .. }
            | MarketKind::PredictionBinary { branch, .. } => Some(*branch),
        }
    }

    pub fn is_conditional_perp(&self) -> bool {
        matches!(self, MarketKind::ConditionalPerp { .. })
    }

    pub fn is_prediction_binary(&self) -> bool {
        matches!(self, MarketKind::PredictionBinary { .. })
    }
}

/// Lifecycle status of an impact-market family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactMarketStatus {
    /// Open for trading on all 5 books.
    Trading,
    /// Past deadline, awaiting resolver signatures. New orders on child books rejected.
    PreResolution,
    /// Fully resolved. Winning conditional perp settled; losing voided.
    /// Binaries settled to $1 (winner) or $0 (loser).
    Resolved(Outcome),
}

/// Stored on-chain record for a standalone event (G17 re-root). Owns its two
/// prediction-binary books (EBY, EBN) and its resolution rule. Unlike an
/// impact-market family it has no underlying perp and no conditional legs, so
/// its binaries never enter the scenario evaluator (they are backed by the
/// DEC-140 locked reserve, out of scope for this record).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventInfo {
    pub event_id: EventId,
    /// Prediction-binary YES book.
    pub eby_market: MarketId,
    /// Prediction-binary NO book.
    pub ebn_market: MarketId,
    /// Human-readable question.
    pub question: String,
    /// Event settlement time in ms since Unix epoch.
    pub settlement_ms: u64,
    /// Grace period after `settlement_ms` before a stale-oracle event may be
    /// voided by a signer (there is no automatic void, G16).
    pub resolution_window_ms: u64,
    /// Current lifecycle status (shares the family status enum).
    pub status: ImpactMarketStatus,
    /// Block timestamp when the event was created (ms since epoch).
    pub created_ms: u64,
    /// Block timestamp when the event resolved (ms since epoch), 0 if unresolved.
    pub resolved_ms: u64,
    /// How the YES/NO outcome is determined. `None` (or absent) means
    /// `RelayerAttested` — a signer supplies the outcome.
    #[serde(default)]
    pub oracle_source: Option<EventOracleSource>,
}

/// Stored on-chain record for an impact-market family. Owns pointers to the
/// 4 child markets (CPY, CPN, EBY, EBN) plus the underlying perp.
///
/// Keep this consensus record limited to fields the engine needs for matching,
/// margin, and lifecycle transitions. Frontend body/rules copy is appended by
/// the query layer as a display DTO so lifecycle rewrites do not mutate
/// presentation metadata in consensus state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImpactMarketInfo {
    pub impact_market_id: ImpactMarketId,
    /// Underlying perp market (unconditional book 1). Must already exist.
    pub underlying_market: MarketId,
    /// Conditional-perp YES child book.
    pub cpy_market: MarketId,
    /// Conditional-perp NO child book.
    pub cpn_market: MarketId,
    /// Prediction-binary YES child book.
    pub eby_market: MarketId,
    /// Prediction-binary NO child book.
    pub ebn_market: MarketId,
    /// Human-readable question (hashed into the market metadata).
    pub question: String,
    /// Event deadline in milliseconds since Unix epoch.
    pub deadline_ms: u64,
    /// Grace period after `deadline_ms` before the auto-void path fires.
    pub resolution_window_ms: u64,
    /// Current lifecycle status.
    pub status: ImpactMarketStatus,
    /// Block timestamp when the impact market was created (ms since epoch).
    pub created_ms: u64,
    /// Block timestamp when the impact market was resolved (ms since epoch), 0 if unresolved.
    pub resolved_ms: u64,
    /// BE-54: how the YES/NO outcome is determined at deadline. Defaults
    /// to `RelayerAttested` for back-compat with pre-BE-54 records (which
    /// decode as `None` here, treated as `RelayerAttested` by the engine).
    /// Stored in addition to `CreateImpactMarket.oracle_source` so the
    /// resolver doesn't need to re-scan the original action bytes.
    #[serde(default)]
    pub oracle_source: Option<EventOracleSource>,
}

// ---------------------------------------------------------------------------
// Admin multisig governance
//
// Design record: <https://github.com/Proof-labs/ProofOfBrain/blob/dev/delivery/epics/w29-04-engine-admin-multisig.md>
// ---------------------------------------------------------------------------

/// Maximum signer-roster size, enforced at genesis, seed, and every
/// rotation. The roster target is six; sixteen leaves rotation headroom
/// while keeping the registry-update action ≈ 400 bytes worst case.
pub const MAX_ADMIN_SIGNERS: usize = 16;
/// Maximum simultaneously pending proposals; propose fails closed
/// beyond this. Bounds every governance prefix scan.
pub const MAX_PENDING_PROPOSALS: usize = 32;
/// Terminal proposals are pruned oldest-first down to this bound on
/// every terminal transition.
pub const MAX_TERMINAL_RETAINED: usize = 256;
/// Hard cap on the canonical inner `AdminAction` bytes at propose;
/// keeps the largest approval well under the gateway's 8 KiB
/// transaction envelope.
pub const MAX_ADMIN_ACTION_BYTES: usize = 2048;
/// Maximum items in an `AdminAction::Batch`. Small on purpose: a batch is
/// one reviewable signing ceremony, not a bulk loader — every item is
/// rendered in full by every approver's tooling, and the canonical pairing
/// this exists for is `[CreateMarket, CreateImpactMarket]`.
/// Four items are reachable only with short free text: the byte cap
/// (`MAX_ADMIN_ACTION_BYTES`, checked first at propose) governs, and two
/// worst-case impact actions alone exceed it — the size proof in codec.rs
/// covers the canonical pair, not four maximal items.
pub const MAX_BATCH_ADMIN_ACTIONS: usize = 4;
/// Byte caps on the impact-market free-text fields, enforced at propose.
/// Together they are what keeps the worst-case single action — and the
/// canonical `[CreateMarket, CreateImpactMarket]` batch — under
/// `MAX_ADMIN_ACTION_BYTES` without raising that cap (which would ripple
/// into the gateway's body budget).
pub const MAX_IMPACT_QUESTION_BYTES: usize = 256;
pub const MAX_IMPACT_DESCRIPTION_BYTES: usize = 768;
pub const MAX_IMPACT_RULES_BYTES: usize = 640;
/// Proposal time-to-live (72 h), observed lazily. A per-release value,
/// never per-proposal.
pub const PROPOSAL_TTL_MS: u64 = 259_200_000;
/// Pagination bound for the proposals query.
pub const MAX_PROPOSAL_PAGE: u16 = 100;
/// Emergency audit-log ring retention.
pub const MAX_EMERGENCY_RETAINED: usize = 256;
/// Global bound on emergency actions per rolling window.
pub const MAX_EMERGENCY_TOTAL_PER_WINDOW: usize = 256;
/// Per-signer bound on emergency actions per rolling window.
pub const MAX_EMERGENCY_PER_SIGNER: usize = 16;
/// Rolling emergency rate window (1 h), judged against block time.
pub const EMERGENCY_RATE_WINDOW_MS: u64 = 3_600_000;
// Retention must cover the whole global window: otherwise an in-window
// record could be ring-evicted and a reintroduced signer would bypass
// its rate limit.
const _: () = assert!(MAX_EMERGENCY_RETAINED >= MAX_EMERGENCY_TOTAL_PER_WINDOW);

/// Governance domain newtypes: consensus values are never bare
/// primitives, so misuse is a compile error. All serialize
/// transparently (a MessagePack newtype encodes as its inner value),
/// so wire and storage encodings equal the bare form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProposalId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EmergencyId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegistryVersion(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignerAddress(pub [u8; 20]);

/// Trading account an admin action targets. Distinct from [`SignerAddress`]
/// (a governance roster member) so the two cannot be swapped at a call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountAddress(pub [u8; 20]);

/// Number of member approvals required to execute an admin proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignatureThreshold(pub u32);

impl ProposalId {
    /// Big-endian key encoding, so prefix scans yield ascending id
    /// order.
    pub fn to_key_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
    pub fn from_key_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_be_bytes(bytes))
    }
}

impl EmergencyId {
    /// Same big-endian contract as `ProposalId::to_key_bytes`.
    pub fn to_key_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
    pub fn from_key_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_be_bytes(bytes))
    }
}

/// The on-chain signer roster — the single source of who may approve
/// admin actions. Registry presence is the activation switch: absent =
/// multisig administration off, legacy admin paths unchanged; present =
/// proposals mandatory, direct admin paths reject.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminSignerRegistry {
    /// Monotone, starts at 1, never reused (a rotation assigns +1).
    pub version: RegistryVersion,
    /// Approvals required to execute a proposal.
    /// Invariant: `2 <= threshold <= members.len()`.
    pub threshold: SignatureThreshold,
    /// Canonically sorted, duplicate-free,
    /// `len <= MAX_ADMIN_SIGNERS` (enforced at genesis, seed, rotation).
    pub members: Vec<SignerAddress>,
}

/// Why a `Pending` proposal expired. Stored in the status payload and
/// committed by the governance digest, so engines cannot disagree on
/// the reason.
/// Why a scheduled funding interval was skipped without catch-up
/// (`Event::FundingSkipped`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
pub enum FundingSkipReason {
    /// The market's oracle is older than `mark_price_max_oracle_age_ms`.
    #[display("oracle_stale")]
    OracleStale,
    /// The market has no `mark_price_max_oracle_age_ms` yet while the
    /// oracle-guard gate is active.
    #[display("oracle_guard_unset")]
    OracleGuardUnset,
}

/// Why an `OracleUpdate` was refused by the per-market deviation guard
/// (`Event::OracleUpdateRejected`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
pub enum OracleRejectReason {
    /// The submitted price sits outside `last_good +- max_oracle_deviation_bps`.
    #[display("deviation_exceeded")]
    DeviationExceeded,
    /// The market has no deviation band yet (`max_oracle_deviation_bps == 0`);
    /// the guard gate is live, so the default fails closed.
    #[display("deviation_band_unset")]
    DeviationBandUnset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
pub enum ExpiryReason {
    /// `expiry_ms` passed (the 72 h TTL, observed lazily).
    #[display("ttl")]
    Ttl,
    /// A registry rotation invalidated every other Pending proposal.
    #[display("registry_changed")]
    RegistryChanged,
}

/// Status of an admin proposal. Terminal payloads (`Failed.code`,
/// `Rejected.by`, `Expired.reason`) are committed by the whole-record
/// governance digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Executed,
    /// Deterministic inner-apply failure, storing the full
    /// `ExecError::code()` value unnarrowed.
    Failed {
        code: u32,
    },
    /// Proposer-cancel, or the member whose rejection reached
    /// `n − m + 1` distinct current members.
    Rejected {
        by: SignerAddress,
    },
    Expired {
        reason: ExpiryReason,
    },
}

impl ProposalStatus {
    /// Payload-free filter byte for the proposals query (`status`
    /// parameter): 0 pending, 1 executed, 2 failed, 3 rejected,
    /// 4 expired. A read-model convenience, not a stored or absorbed
    /// value.
    pub const fn status_tag(&self) -> u8 {
        match self {
            Self::Pending => 0,
            Self::Executed => 1,
            Self::Failed { .. } => 2,
            Self::Rejected { .. } => 3,
            Self::Expired { .. } => 4,
        }
    }

    /// Payload-free label for error messages and event attributes.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executed => "executed",
            Self::Failed { .. } => "failed",
            Self::Rejected { .. } => "rejected",
            Self::Expired { .. } => "expired",
        }
    }
}

/// One admin-multisig proposal. The canonical bytes, not the typed
/// value, are the authoritative content: `action_bytes` is the engine's
/// own re-encoding of the typed `AdminAction`, and every approval
/// comparison is engine-canonical vs engine-canonical.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminProposalRecord {
    pub proposal_id: ProposalId,
    /// Registry version this proposal is bound to; a rotation expires
    /// every other `Pending` proposal.
    pub registry_version: RegistryVersion,
    /// Threshold snapshot at propose time.
    pub threshold: SignatureThreshold,
    pub proposer: SignerAddress,
    /// `AdminAction` arm discriminant (indexing/display only).
    pub action_tag: u8,
    /// Engine-canonical MessagePack encoding of the typed
    /// `AdminAction` — what is stored, hashed, and byte-compared.
    #[serde(with = "crate::wire_bytes::vec")]
    pub action_bytes: Vec<u8>,
    /// Sorted, deduped; the proposer is inserted at creation
    /// (= approval #1).
    pub approvals: Vec<SignerAddress>,
    /// Sorted, deduped, disjoint from `approvals`: votes are immutable
    /// and the opposite vote fails with `ConflictingVote`.
    pub rejections: Vec<SignerAddress>,
    pub status: ProposalStatus,
    pub created_height: u64,
    /// `ctx.block_time_ms` at propose.
    pub created_ms: u64,
    /// `created_ms + PROPOSAL_TTL_MS`.
    pub expiry_ms: u64,
    /// Domain-separated commitment to the immutable proposal context.
    #[serde(with = "crate::wire_bytes")]
    pub content_hash: [u8; 32],
}

/// One executed single-signer emergency action — the on-chain audit
/// trail. Consensus rate limits are derived from this log (no separate
/// mutable counters), so `time_ms` is committed by the governance
/// digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmergencyActionRecord {
    pub emergency_id: EmergencyId,
    pub signer: SignerAddress,
    /// `EmergencyAction` arm discriminant.
    pub action_tag: u8,
    /// `None` for market-less arms (`HaltTrading`).
    pub market_id: Option<MarketId>,
    pub height: u64,
    /// Block time at execution; feeds the rolling rate window.
    pub time_ms: u64,
}

/// Closed set of actions executable through admin multisig governance.
/// Adding a variant changes the wire format; unknown variants fail decoding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AdminAction {
    /// Creates a market. The embedded signer must be zero because governance
    /// supplies the authorization.
    CreateMarket(CreateMarket),
    /// Replaces the admin signer roster and approval threshold.
    UpdateAdminSignerRegistry(UpdateAdminSignerRegistry),
    /// Creates an impact-market family (4 child books). The embedded signer
    /// must be zero, same rule as `CreateMarket`. Admitted from the chain
    /// lineage's admin-actions-v2 activation height
    /// (`crate::repo::ADMIN_ACTIONS_V2_ACTIVATIONS`).
    CreateImpactMarket(CreateImpactMarket),
    /// Creates a standalone event (2 binary books, no underlying). Same
    /// zero-signer rule as `CreateImpactMarket`.
    CreateEvent(CreateEvent),
    /// Two to `MAX_BATCH_ADMIN_ACTIONS` market-creation actions executed
    /// sequentially in ONE child overlay, so the whole batch lands
    /// atomically and a later item may reference state an earlier item
    /// created (the canonical use: a family whose underlying perp is born
    /// in the same proposal). Admitted from the same v2 activation height.
    ///
    /// The item type is deliberately NOT `AdminAction` (review finding on
    /// the first cut, which used `Vec<AdminAction>`): a recursive enum
    /// hands a consensus-relevant acceptance boundary to the msgpack
    /// parser's internal recursion limit and opens a deep-nesting stack
    /// hazard at decode — before any validation runs. With a closed item
    /// enum, a nested batch or a registry change inside a batch is not a
    /// validation refusal, it is UNDECODABLE, and the allowed item set is
    /// exhaustive at compile time. The wire bytes for valid batches are
    /// unchanged (externally tagged variant names are identical), which
    /// the frozen vector test proves.
    Batch(Vec<AdminBatchItem>),
    /// Schedules a complete trigger-policy replacement for one standalone
    /// perpetual market. Admitted only after the dormant trigger index gate.
    SetTriggerMarketConfig(crate::triggers::SetTriggerMarketConfig),
    /// Governance authorization to resume the halted bridge. Carries no
    /// payload: the quorum-executed proposal is itself the authorization.
    UnpauseBridge,
    /// Adds and/or removes addresses in one operator-authority set — the
    /// on-chain revocation and rotation path the relayer/oracle/composite
    /// allowlists otherwise lack (#422). Admitted only from the lineage's
    /// authority-governance activation height.
    UpdateAuthoritySet(UpdateAuthoritySet),
    /// Schedules (or reschedules) the pending protocol upgrade: the target
    /// height, the protocol version that must be staged, and the SHA-256 of
    /// the successor library file. One pending plan; rescheduling replaces.
    /// Governed via the signer registry; the hash is re-verified against the
    /// staged file at the swap (fail-closed on mismatch).
    ScheduleUpgrade(ScheduleUpgrade),
    /// Cancels the pending protocol upgrade plan. Must commit before the
    /// plan's target height; a cancellation that has not committed on every
    /// validator before the boundary is not a cancellation (roll-forward
    /// only, DEC-32).
    CancelUpgrade(CancelUpgrade),
    /// Cancels every resting order of one account, optionally confined to
    /// one market, through the same store path as the owner's own
    /// cancel-all: reserved margin is released and one `OrderCancelled`
    /// event is emitted per order. A one-shot sweep at quorum, not a
    /// freeze: the account may place again in the next block. Admitted
    /// only from the lineage's cancel-all-for-account activation height.
    CancelAllOrdersForAccount(CancelAllOrdersForAccount),
    /// Schedules a canonical, complete oracle-policy epoch after quorum approval.
    ConfigureOraclePolicy(ConfigureOraclePolicy),
    /// Sets the per-market oracle guards (`mark_price_max_oracle_age_ms`
    /// and `max_oracle_deviation_bps`) on one standalone perpetual through
    /// the admin quorum. Admitted only from
    /// `crate::repo::UPGRADE_HEIGHT_ORACLE_GUARDS_CONFIG`; below it the
    /// arm reads as an unknown variant exactly like an older binary.
    SetOracleGuards(SetOracleGuards),
}

/// Payload of [`AdminAction::SetOracleGuards`]: the market and the guard
/// values to write. `None` leaves a field untouched; at least one field must
/// be `Some`, and a supplied value must be non-zero, because zero is the
/// "unset" sentinel that fails closed once the guard gate is live. Clearing a
/// guard is deliberately not expressible: to stop trading on a market, halt
/// it, do not disarm its guards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetOracleGuards {
    pub market: MarketId,
    /// New `MarketConfig::mark_price_max_oracle_age_ms` in milliseconds.
    pub mark_price_max_oracle_age_ms: Option<u64>,
    /// New `MarketOracleGuards::max_oracle_deviation_bps` in basis points, at
    /// most 10_000.
    pub max_oracle_deviation_bps: Option<u32>,
}

/// One operator-authority allowlist. On the `UpdateAuthoritySet` wire the
/// domain is a serde variant *name* (externally tagged), so an unknown domain
/// fails to decode rather than silently mis-targeting a set. The `#[repr(u8)]`
/// discriminant is load-bearing for the presence-key bytes (`[prefix][cap]…`),
/// not for the wire. `Oracle`/`CexComposite`/`Relayer` are the genesis-seeded
/// presence sets; `Custody`/`MarketParams`/`ScheduledOps` are the capability
/// sets the split (#422 item 3) activates — their discriminants ship here,
/// dormant, so the wire has a single mixed-fleet decode boundary.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthorityDomain {
    Oracle = 1,
    CexComposite = 2,
    Relayer = 3,
    Custody = 4,
    MarketParams = 5,
    ScheduledOps = 6,
}

/// Payload of [`AdminAction::UpdateAuthoritySet`]: the target domain and the
/// addresses to add and remove. Both lists are canonically sorted,
/// duplicate-free, and disjoint; the net set may never be left empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateAuthoritySet {
    pub domain: AuthorityDomain,
    pub add: Vec<SignerAddress>,
    pub remove: Vec<SignerAddress>,
}

/// Payload of [`AdminAction::CancelAllOrdersForAccount`]: the account whose
/// resting orders are cancelled and, when set, the single market the cancel
/// is confined to. An account with no matching orders executes as a no-op.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelAllOrdersForAccount {
    pub owner: AccountAddress,
    pub market: Option<MarketId>,
}

/// Payload of [`AdminAction::ScheduleUpgrade`]: one pending protocol-upgrade
/// plan. `successor_sha256` pins the staged successor library file — verified
/// at schedule time and re-verified at the swap (fail-closed on mismatch).
/// `protocol_version` must differ from the active version: scheduling a
/// self-upgrade is a no-op by construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleUpgrade {
    /// Consensus height at which the successor becomes active.
    pub target_height: u64,
    /// The successor's monotonic protocol version.
    pub protocol_version: u32,
    /// SHA-256 of the staged successor library file.
    #[serde(with = "crate::wire_bytes")]
    pub successor_sha256: [u8; 32],
}

/// Payload of [`AdminAction::CancelUpgrade`]: which plan is being cancelled,
/// by target height, so a cancellation names the plan it retires. Cancelling
/// a non-existent plan is a no-op, not an error: the plan is gone either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelUpgrade {
    /// The `target_height` of the plan being cancelled.
    pub target_height: u64,
}

/// The closed set of actions a `Batch` may carry: market creations only.
/// A registry change must be its own reviewable proposal — a roster
/// rewrite hidden among market operations is precisely the review hazard
/// the closed inner allowlist exists to prevent — and nesting is
/// structurally impossible rather than merely refused.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AdminBatchItem {
    /// Same rules as the singleton arm: all-zero inner signer.
    CreateMarket(CreateMarket),
    /// Same rules as the singleton arm: all-zero inner signer.
    CreateImpactMarket(CreateImpactMarket),
}

/// Stable discriminant for the closed [`AdminAction`] namespace.
///
/// This is distinct from the outer transaction [`crate::ActionType`]
/// namespace. Values are committed by proposal hashes and must never be
/// repurposed after use on a persistent chain.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AdminActionType {
    CreateMarket = 1,
    UpdateAdminSignerRegistry = 2,
    CreateImpactMarket = 3,
    Batch = 4,
    SetTriggerMarketConfig = 5,
    UnpauseBridge = 6,
    UpdateAuthoritySet = 7,
    CancelAllOrdersForAccount = 8,
    /// Create a standalone event (G17). Governed like `CreateImpactMarket`.
    CreateEvent = 9,
    /// Reserved by RT-01. See [`AdminActionType::ReservedRt01A`].
    ReservedRt01B = 10,
    /// Reserved by RT-01. See [`AdminActionType::ReservedRt01A`].
    ReservedRt01C = 11,
    ConfigureOraclePolicy = 12,
    /// Per-market oracle guards. Admitted from
    /// `crate::repo::UPGRADE_HEIGHT_ORACLE_GUARDS_CONFIG`.
    SetOracleGuards = 13,
    /// Schedules (or reschedules) the pending protocol upgrade plan.
    /// Governed like `UpdateAuthoritySet`: signer-registry path, one
    /// pending plan, target height never decreases.
    ScheduleUpgrade = 14,
    /// Cancels the pending protocol upgrade plan. Must commit before the
    /// plan's target height to have effect.
    CancelUpgrade = 15,
}

impl AdminAction {
    /// Return the engine-owned tag committed by the proposal content hash.
    pub const fn action_type(&self) -> AdminActionType {
        match self {
            Self::CreateMarket(_) => AdminActionType::CreateMarket,
            Self::UpdateAdminSignerRegistry(_) => AdminActionType::UpdateAdminSignerRegistry,
            Self::CreateImpactMarket(_) => AdminActionType::CreateImpactMarket,
            Self::CreateEvent(_) => AdminActionType::CreateEvent,
            Self::Batch(_) => AdminActionType::Batch,
            Self::SetTriggerMarketConfig(_) => AdminActionType::SetTriggerMarketConfig,
            Self::UnpauseBridge => AdminActionType::UnpauseBridge,
            Self::UpdateAuthoritySet(_) => AdminActionType::UpdateAuthoritySet,
            Self::CancelAllOrdersForAccount(_) => AdminActionType::CancelAllOrdersForAccount,
            Self::ConfigureOraclePolicy(_) => AdminActionType::ConfigureOraclePolicy,
            Self::SetOracleGuards(_) => AdminActionType::SetOracleGuards,
            Self::ScheduleUpgrade(_) => AdminActionType::ScheduleUpgrade,
            Self::CancelUpgrade(_) => AdminActionType::CancelUpgrade,
        }
    }

    pub const fn action_tag(&self) -> u8 {
        self.action_type() as u8
    }
}

/// Replacement signer roster. The engine assigns the next registry version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateAdminSignerRegistry {
    /// Number of member approvals required by the replacement roster.
    pub new_threshold: SignatureThreshold,
    /// Members of the replacement roster.
    pub new_members: Vec<SignerAddress>,
}

/// Submits a typed admin action under the current signer registry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposeAdminAction {
    /// Address authorizing the proposal; verified against the envelope signer.
    pub proposer: SignerAddress,
    /// Registry version under which the proposal is submitted.
    pub registry_version: RegistryVersion,
    /// Admin operation proposed for multisig execution.
    pub action: AdminAction,
}

/// Approves a proposal by carrying its complete immutable context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApproveAdminAction {
    /// Address authorizing the approval; verified against the envelope signer.
    pub approver: SignerAddress,
    /// Identifier of the proposal being approved.
    pub proposal_id: ProposalId,
    /// Registry version captured when the proposal was created.
    pub registry_version: RegistryVersion,
    /// Required approval count captured when the proposal was created.
    pub threshold: SignatureThreshold,
    /// Address that created the proposal.
    pub proposer: SignerAddress,
    /// Block height at which the proposal was created.
    pub created_height: u64,
    /// Block timestamp at which the proposal was created, in milliseconds.
    pub created_ms: u64,
    /// Block timestamp after which the proposal expires, in milliseconds.
    pub expiry_ms: u64,
    /// Typed admin operation being approved.
    pub action: AdminAction,
    /// Domain-separated commitment to the immutable proposal context.
    #[serde(with = "crate::wire_bytes")]
    pub content_hash: [u8; 32],
}

/// Rejects a proposal identified by its id and content commitment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RejectAdminAction {
    /// Address authorizing the rejection; verified against the envelope signer.
    pub rejecter: SignerAddress,
    /// Identifier of the proposal being rejected.
    pub proposal_id: ProposalId,
    /// Domain-separated commitment of the proposal being rejected.
    #[serde(with = "crate::wire_bytes")]
    pub content_hash: [u8; 32],
}

/// Closed set of immediate, loss-reducing actions available to one signer.
/// Reverse transitions are intentionally absent and require multisig actions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EmergencyAction {
    PauseMarket { market_id: MarketId },
    HaltTrading {},
    SetReduceOnly { market_id: MarketId },
}

/// Stable discriminant for the closed [`EmergencyAction`] namespace,
/// committed by the on-chain audit log
/// (`EmergencyActionRecord::action_tag`). Distinct from
/// [`AdminActionType`]; values must never be repurposed after use on a
/// persistent chain — see `BYTES.md`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmergencyActionType {
    PauseMarket = 1,
    HaltTrading = 2,
    SetReduceOnly = 3,
}

impl EmergencyAction {
    /// Return the engine-owned arm type committed by the audit log.
    pub const fn action_type(&self) -> EmergencyActionType {
        match self {
            Self::PauseMarket { .. } => EmergencyActionType::PauseMarket,
            Self::HaltTrading {} => EmergencyActionType::HaltTrading,
            Self::SetReduceOnly { .. } => EmergencyActionType::SetReduceOnly,
        }
    }

    pub const fn action_tag(&self) -> u8 {
        self.action_type() as u8
    }

    /// Target market for market-scoped arms; `None` for chain-wide arms.
    pub const fn market_id(&self) -> Option<MarketId> {
        match self {
            Self::PauseMarket { market_id } | Self::SetReduceOnly { market_id } => Some(*market_id),
            Self::HaltTrading {} => None,
        }
    }
}

/// Submits an immediate emergency action without the proposal flow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergencyAdminAction {
    /// Registry member authorizing the action; verified against the envelope signer.
    pub signer: SignerAddress,
    /// Immediate loss-reducing operation to execute.
    pub action: EmergencyAction,
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Order/position direction. Discriminant values (1, 2) are part of the wire format.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
)]
pub enum Side {
    #[display("buy")]
    Buy = 1,
    #[display("sell")]
    Sell = 2,
}

/// Time-in-force policy for a `PlaceOrder`. Controls how unmatched
/// quantity is handled after crossing the book. Serialized with serde
/// default-to-0 so old wire records decode as `Gtc`.
///
/// Wire encoding: msgpack enum variant (`Gtc`, `Ioc`, `Fok`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good-Till-Cancelled: unmatched quantity rests on the book until
    /// explicitly cancelled or TTL-expired. The default.
    #[default]
    Gtc = 0,
    /// Immediate-Or-Cancel: unmatched quantity after crossing is dropped
    /// (never rests on the book). Same IOC semantics as a `MarketOrder`
    /// but with a price limit — will not cross beyond it.
    Ioc = 1,
    /// Fill-Or-Kill: the order must be fully filled immediately at or better
    /// than the limit price. If the currently visible book cannot fill the
    /// whole quantity, the engine rejects before mutating state.
    Fok = 2,
}

impl core::fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TimeInForce::Gtc => f.write_str("gtc"),
            TimeInForce::Ioc => f.write_str("ioc"),
            TimeInForce::Fok => f.write_str("fok"),
        }
    }
}

/// Resting liquidity at one price level, summed over the orders at that price.
/// Written through by the order mutators so an L2 read never has to visit an
/// `Order` record. Absence of the key is the sole representation of an empty
/// level: once `order_count` reaches zero the row is deleted rather than
/// stored zeroed, so a scan yields exactly the occupied prices.
///
/// Every transition is checked; `None` means the caller's bookkeeping is
/// inconsistent with the level and the transaction must abort rather than
/// persist a level that disagrees with its orders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelAggregate {
    pub total_quantity: u64,
    pub order_count: u32,
}

impl LevelAggregate {
    /// One order joins the level carrying `quantity` remaining.
    pub fn added(self, quantity: u64) -> Option<Self> {
        Some(Self {
            total_quantity: self.total_quantity.checked_add(quantity)?,
            order_count: self.order_count.checked_add(1)?,
        })
    }

    /// One order leaves the level, withdrawing its `quantity` remaining.
    pub fn removed(self, quantity: u64) -> Option<Self> {
        Some(Self {
            total_quantity: self.total_quantity.checked_sub(quantity)?,
            order_count: self.order_count.checked_sub(1)?,
        })
    }

    /// An order at the level is partially filled; it keeps its slot.
    pub fn reduced(self, quantity: u64) -> Option<Self> {
        if self.order_count == 0 {
            return None;
        }
        Some(Self {
            total_quantity: self.total_quantity.checked_sub(quantity)?,
            order_count: self.order_count,
        })
    }

    /// No orders rest here, so the row should be deleted rather than written.
    /// Quantity without orders is unrepresentable — it means a mutator
    /// decremented the count without withdrawing the matching quantity.
    pub fn is_vacant(self) -> bool {
        self.order_count == 0 && self.total_quantity == 0
    }

    /// Quantity remains but no order claims it.
    pub fn is_inconsistent(self) -> bool {
        self.order_count == 0 && self.total_quantity != 0
    }
}

/// A resting limit order on the order book.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub market: MarketId,
    /// Keccak-256-derived account address (first 20 bytes of pubkey hash).
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub side: Side,
    /// Limit price in micro-USDC per unit of the base asset.
    pub price: u64,
    /// Remaining quantity in base-asset units.
    pub quantity: u64,
    /// Optional client-assigned order ID. Stored on resting orders so
    /// fills/cancels can be reconciled without joining against the
    /// original placement request. Zero is reserved as the event-level
    /// "absent" sentinel; the optional storage field preserves the
    /// engine semantics.
    #[serde(default)]
    pub client_order_id: Option<u64>,
    /// Quantity originally accepted onto the book. Legacy orders decode
    /// with `0`, in which case event helpers fall back to the current
    /// remaining quantity.
    #[serde(default)]
    pub original_quantity: u64,
    /// Cumulative quantity filled while this order rested as maker.
    #[serde(default)]
    pub filled_quantity: u64,
    /// Block timestamp (ms since epoch) when this order was placed.
    /// Paired with `MarketConfig::default_ttl_ms` to power the
    /// `run_order_expiry` end-of-block sweep. Zero means "no
    /// timestamp recorded" — which is also how on-chain `Order`
    /// records written before this field existed decode thanks to
    /// `serde(default)`. An order with `created_at_ms = 0` is
    /// treated as "never expires" for safety (we don't want to
    /// accidentally cancel legacy orders that predate the TTL work).
    #[serde(default)]
    pub created_at_ms: u64,
    /// Monotonic FIFO priority within a price level. Defaults to `0` for
    /// legacy orders, in which case readers fall back to `id`. Keeping this
    /// separate lets amend preserve the public order id while still resetting
    /// queue priority when a quote moves price or increases size.
    #[serde(default)]
    pub queue_priority: u64,
    /// Time-in-force the order was placed with. Only `Gtc` orders ever
    /// rest, so this is `Gtc` for every order currently reachable here —
    /// carried through so the open-orders read surface can echo it
    /// without re-deriving from the placement action. Legacy records
    /// decode as `Gtc`.
    #[serde(default)]
    pub time_in_force: TimeInForce,
    /// Whether the order was placed post-only. Legacy records decode as
    /// `false`.
    #[serde(default)]
    pub post_only: bool,
    /// Whether the order was placed reduce-only. A resting reduce-only
    /// order is meaningful to surface so the UI can flag it. Legacy
    /// records decode as `false`.
    #[serde(default)]
    pub reduce_only: bool,
}

/// Deterministic execution context passed to every transaction handler.
///
/// Constructed exclusively by the FFI boundary from CometBFT's `FinalizeBlock` fields.
pub struct TxContext {
    /// 1-based block height.
    pub height: u64,
    /// 0-based index within the block. `u32::MAX` / `u32::MAX - 1` are sentinels
    /// for end-of-block liquidation and funding events respectively.
    pub tx_index: u32,
    /// Block timestamp in milliseconds since Unix epoch.
    pub block_time_ms: u64,
    /// 32-byte chain_id binding used by the v3 signing envelope
    /// (`crate::crypto::signing_message`). Every v2-enveloped tx at
    /// verify time requires the chain_id that was used at sign time,
    /// so the value MUST be consistent across all validators — the
    /// FFI layer sources it from genesis / snapshot-bound state and
    /// threads it through every `FinalizeBlock` call.
    ///
    /// Defaults to `crypto::UNBOUND_CHAIN_ID` ([0u8; 32]) in tests
    /// and in unbound deployments; production chains must set a
    /// non-zero value to close the cross-chain replay vector
    /// (audit B4, 2026-04-23).
    pub chain_id: [u8; 32],
}

// ---------------------------------------------------------------------------
// Position & margin types
// ---------------------------------------------------------------------------

/// Persistent position state per owner per market.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub market: MarketId,
    pub side: Side,
    /// Weighted-average entry price (in quote units, same scale as order prices).
    pub entry_price: u64,
    /// Absolute size (always > 0 while position exists).
    pub size: u64,
    /// Cumulative funding index at the time the position was last settled.
    pub last_funding_index: i64,
}

/// Signed open interest for one market, tracked as two unsigned running sums.
///
/// `long` is Σ size over all long positions, `short` over all shorts. Both
/// sides are kept because liquidation and ADL can transiently break the
/// `long == short` accounting identity, and a side flip moves size from one
/// sum to the other. The enforced cap is `max(long, short)` (see
/// `MarketConfig::max_open_interest`). Maintained write-through at the position
/// repo chokepoint (v10+) and backfilled once at the v10 upgrade height.
///
/// The counter is not itself an app-hash input, but the cap verdict it feeds
/// decides whether a fill happens, and fills move `open_order_count` /
/// `next_order_id`, which are. Activation is therefore a coordinated consensus
/// upgrade, not a free additive key; what makes a gradual *binary* rollout safe
/// is that both the maintenance and the read gate on the stored schema.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenInterest {
    pub long: u64,
    pub short: u64,
}

impl OpenInterest {
    pub const fn magnitude(self) -> u64 {
        if self.long >= self.short {
            self.long
        } else {
            self.short
        }
    }
}

/// Generation of one owner's position on one market.
///
/// The value is stored separately from [`Position`] so historical position
/// rows keep their existing encoding. It is also carried on trigger-management
/// actions to prevent a bracket from attaching to a later close-and-reopen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PositionEpoch(pub u64);

/// Client-scoped idempotency sequence for a position-trigger bracket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientTriggerGroupId(pub u64);

/// Client correlation identifier for one limb within a bracket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientTriggerId(pub u64);

/// Explicit slippage collar for one trigger attempt, in basis points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TriggerSlippageBps(pub u32);

/// Maximum byte length of a `MarketConfig::ticker` / `CreateMarket::ticker`.
/// Bounds state size and keeps the metadata field predictable. Derived impact
/// child tickers (`<underlying>-<id>-<leg>`) also stay within this for any
/// reasonable underlying ticker.
pub const MAX_TICKER_LEN: usize = 24;

/// Per-market risk parameters. Stored on-chain via `CreateMarket` admin action.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketConfig {
    pub market: MarketId,
    /// Initial margin ratio in basis points (e.g. 3334 = 33.34% → about 3x leverage).
    pub im_bps: u32,
    /// Maintenance margin ratio in basis points (e.g. 1667 = 16.67% → about 6x).
    pub mm_bps: u32,
    /// Taker fee in basis points (e.g. 5 = 0.05%).
    pub taker_fee_bps: u32,
    /// Maker fee in basis points (e.g. 2 = 0.02%).
    pub maker_fee_bps: u32,
    /// Funding interval in milliseconds. 0 = funding disabled.
    pub funding_interval_ms: u64,
    /// Maximum absolute funding rate in basis points per interval. New values
    /// are bounded to 3,000 bps (30%) by CreateMarket and UpdateMarketFees.
    pub max_funding_rate_bps: u32,
    /// Market kind (perp / conditional perp / prediction binary).
    ///
    /// `#[serde(default)]` so existing on-chain `MarketConfig` records written
    /// before impact markets existed decode cleanly as `MarketKind::Perp`.
    #[serde(default)]
    pub kind: MarketKind,
    /// Maximum absolute position size per account, in contracts. Enforced
    /// at order-placement time: a fill that would push the taker's net
    /// position (signed) beyond ±max_position_size gets rejected with
    /// `ExecError::PositionLimitExceeded`. Zero means "no limit" — which
    /// is also what existing on-chain MarketConfig records (written
    /// before this field existed) decode as thanks to serde(default).
    ///
    /// Set this via CreateMarket or UpdateMarketFees for new markets;
    /// existing markets keep `0` until explicitly updated.
    #[serde(default)]
    pub max_position_size: u64,
    /// Default order time-to-live in milliseconds. When > 0, the
    /// end-of-block `run_order_expiry` sweep cancels any resting
    /// order whose `created_at_ms + default_ttl_ms < block_time_ms`.
    /// Zero means "no TTL" — backward-compatible default for
    /// markets created before this field existed (serde(default)).
    ///
    /// Motivation: MMs and bots that crash or restart leave
    /// orphaned orders resting forever at stale prices, silently
    /// locking up their initial margin. On 2026-04-23 alice (core
    /// MM owner) had 1432 such orders across 20+ markets pinning
    /// $6M IM against $2.9M equity — every new ask bounced with
    /// `InsufficientMargin` and the BTC book went permanently
    /// one-sided. With TTL set, the same shape self-heals within
    /// `default_ttl_ms` rather than requiring manual cleanup.
    ///
    /// Recommended value for perps: 60_000 (1 minute) to match
    /// the MM refresh cadence. For impact markets: longer (minutes)
    /// because their MMs don't re-quote as often.
    #[serde(default)]
    pub default_ttl_ms: u64,
    /// When true, this market's firing legs participate in net-delta
    /// margin aggregation: all firing legs within a scenario that share
    /// the same underlying market are combined into a single net position
    /// for MM/IM. Charges `|Σ signed_size| × settle × weighted_bps` rather
    /// than summing bps-on-notional per leg.
    ///
    /// Scope: only Perp and ConditionalPerp. PredictionBinary legs are
    /// always charged per-leg (they don't add linear delta to their
    /// underlying). Flag on a binary's MarketConfig is ignored for
    /// grouping.
    ///
    /// Grouping key: the underlying perp market id. For a perp, that's
    /// the perp itself. For a conditional perp, it's the perp referenced
    /// by the impact market's `underlying_market`. Two legs group iff
    /// they share the same underlying AND both have `net_delta_margin=true`
    /// AND both fire in the current scenario.
    ///
    /// Within a group: `mm = |Σ signed_size| × settle × weighted_mm_bps /
    /// 10_000`, where `weighted_mm_bps = Σ(|size_i| × mm_bps_i) / Σ|size_i|`.
    /// Same formula for IM with im_bps. A perfectly hedged group
    /// (net_signed = 0) charges zero MM regardless of per-leg bps.
    ///
    /// Default: false — legacy per-leg scenario margin behavior.
    /// `#[serde(default)]` keeps existing on-chain MarketConfig records
    /// decoding correctly.
    ///
    /// See docs/margin-engine.md §6 for the derivation.
    #[serde(default)]
    pub net_delta_margin: bool,
    /// Insurance-fund pool grouping. Markets with the same `pool_id`
    /// share an insurance fund — a JELLY-style blowout in one pool can
    /// drain its own IF to zero without touching the IF that backs
    /// other pools. See `docs/adl-vs-socialized-loss.md` §3 for the
    /// full waterfall design (HLP → per-pool IF → socialized loss →
    /// ADL).
    ///
    /// Defaults to 0 so existing on-chain MarketConfigs (written before
    /// per-pool IF existed) all map to the legacy single-IF behavior.
    /// Pool 0 reads/writes route to the legacy `INSURANCE_FUND` key,
    /// preserving balance continuity across the upgrade.
    ///
    /// At launch we run two pools:
    ///   * Pool 0 — BTC/ETH/SOL perps + their conditional perps (the
    ///     "majors" pool). Inherits all existing balance.
    ///   * Pool 1 — high-vol prediction binaries / longtail markets.
    ///     Per-event tighter caps, isolated from majors.
    ///
    /// **Pool IDs are operator-defined free-form labels (u8) — there is
    /// no engine-level validation against a known set.** A typo (e.g.
    /// passing `99` instead of `2`) silently creates the market in an
    /// isolated pool with empty IF and empty ADL queue. Run
    /// `scripts/admin-pool-audit.ts` after every CreateMarket batch to
    /// surface markets in singleton pools.
    #[serde(default)]
    pub pool_id: u8,
    /// Maximum age (ms) of the oracle reading at the time of any
    /// margin/order/liquidation read. `0 = no check (back-compat)`.
    ///
    /// Oracle staleness was previously enforced only at `ResolveImpactMarket`
    /// (60 s window) and replay-protection in `OracleUpdate`. Order
    /// placement, margin checks, and liquidation read `get_mark_price`
    /// without checking the oracle's age, so a node with a stuck
    /// feeder silently mispriced everything. With this field set on a
    /// market, the engine refuses to read an oracle whose
    /// `publish_time_ms` is older than `block_time_ms -
    /// mark_price_max_oracle_age_ms` and returns `ExecError::StaleOracle`
    /// (BE-33, 2026-05-03).
    ///
    /// Skipped on impact-family markets (CPY/CPN/EBY/EBN) — those
    /// mark off the book directly via the EWMA fallback and have no
    /// continuous oracle layer post the 2026-04-26 redesign.
    ///
    /// Recommended value: 30_000 (30 s) for major perps. Existing
    /// MarketConfig records decode with `mark_price_max_oracle_age_ms = 0`
    /// thanks to `#[serde(default)]`, preserving back-compat.
    #[serde(default)]
    pub mark_price_max_oracle_age_ms: u64,
    /// Volume-based fee tier table. Empty (default for legacy
    /// MarketConfig records) falls back to flat `taker_fee_bps` /
    /// `maker_fee_bps`. Non-empty tables are evaluated at fill time
    /// against each account's rolling 30-day taker volume and apply
    /// tenth-bps fees. Negative maker values are rebates paid from
    /// the FeePool.
    ///
    /// Added after `mark_price_max_oracle_age_ms` so already-merged
    /// BE-33 records keep their positional wire/state layout.
    #[serde(default)]
    pub fee_tiers: Vec<FeeTier>,
    /// Tick size in micro-USDC. Order prices must be exact multiples
    /// of `tick_size`. Zero (default) disables the check, preserving
    /// pre-BE-48 behavior. Recommended: $0.01 = 10_000 µUSDC for
    /// crypto perps; $0.001 = 1_000 µUSDC for high-precision impact
    /// market child books.
    #[serde(default)]
    pub tick_size: u64,
    /// Lot size in contracts. Order quantities must be exact multiples
    /// of `lot_size`. Zero (default) disables the check, preserving
    /// pre-BE-48 behavior. Recommended: 1 for whole-contract markets
    /// (BTC perps), 100 for high-volume markets where round lots
    /// improve readability.
    #[serde(default)]
    pub lot_size: u64,
    /// Primary oracle signer for this market (BE-50). When `Some`,
    /// this signer's `OracleUpdate` is always accepted (subject to
    /// the existing monotonic publish-time check).
    ///
    /// Other authorized signers (the "fallback" oracles) are accepted
    /// only when the market's last oracle update — by *any* signer —
    /// is older than `oracle_staleness_ms`. This means the gate works
    /// recursively: once a fallback takes over, the next fallback is
    /// gated against the active fallback's timestamp, not the primary's.
    /// Net effect: fail-over chains through fallbacks rather than
    /// requiring the primary itself to recover.
    ///
    /// `None` (default) preserves the pre-BE-50 behavior where any
    /// authorized signer can update at any time.
    #[serde(default)]
    pub primary_oracle_signer: Option<[u8; 20]>,
    /// Window (ms) the primary oracle has to publish before fallback
    /// signers can take over. **Zero disables the gate entirely** —
    /// any authorized relayer can post `OracleUpdate` regardless of
    /// how recent the primary's update was. Use a non-zero value
    /// when you want primary-preferred operation with fallback only
    /// on silence; use zero when you want simple
    /// "any-authorized-signer" semantics. Set per-market via
    /// `UpdateMarketFees`.
    #[serde(default)]
    pub oracle_staleness_ms: u64,
    /// Mark-price source mode. See `MarkSourceMode` docstring for
    /// semantics. `serde(default)` -> `OracleOnly` so existing on-chain
    /// `MarketConfig` records and the genesis path are byte-identical
    /// to today. Operators flip individual perp markets to `Median`
    /// via `UpdateMarketFees` once Phase A is live.
    ///
    /// Ignored on impact-family markets (`ConditionalPerp`,
    /// `PredictionBinary`) - those keep marking off the book EWMA per
    /// the no-oracle-MTM redesign (2026-04-26).
    ///
    /// Linear: BE-31 Phase A.
    #[serde(default)]
    pub mark_source_mode: MarkSourceMode,
    /// Top-of-book spread cap (bps) for the thin-book guard on
    /// `MarkSourceMode::Median`. When the top-of-book spread exceeds
    /// this, book-mid is excluded from the median.
    ///
    /// Zero means "use the built-in default `DEFAULT_MAX_MARK_SPREAD_BPS`
    /// (100 bps = 1%)" - which is also the value existing on-chain
    /// `MarketConfig` records (written before this field existed)
    /// decode to thanks to `serde(default)`. Operators tighten or
    /// loosen per-market via `UpdateMarketFees`.
    ///
    /// Linear: BE-31 Phase A.
    #[serde(default)]
    pub max_mark_spread_bps: u32,
    /// BE-31 Phase B: max age (ms) for a composite-CEX price update
    /// before the engine excludes it from the median. Zero means
    /// "use the built-in default `DEFAULT_CEX_COMPOSITE_STALENESS_MS`
    /// (30s)" — also the value existing on-chain `MarketConfig`
    /// records (written before this field existed) decode to thanks
    /// to `serde(default)`. Ignored unless `mark_source_mode` is
    /// `Median`.
    #[serde(default)]
    pub cex_composite_staleness_ms: u64,
    /// BE-26: enable partial liquidation for this market. When true,
    /// the liquidation engine closes positions one at a time and
    /// rechecks maintenance margin after each close. If MM holds after
    /// closing a single market's position, the account is considered
    /// healthy and the remaining positions are not closed.
    ///
    /// When false (default, backward-compatible with existing
    /// `MarketConfig` records), the legacy all-or-nothing liquidation
    /// runs: every position in the owner's portfolio closes when any
    /// market is under MM.
    #[serde(default)]
    pub partial_liquidation_enabled: bool,
    /// Published size scale: `quantity` is in units of `10^-sz_decimals`
    /// base asset. The engine DOES read this for every market kind: each
    /// `price × size` notional/PnL/margin product is divided by `10^sz_decimals`
    /// so e.g. a BTC perp with `sz_decimals = 5` treats `size = 1` as `1e-5`
    /// BTC, and a prediction-binary book at `sz_decimals = 2`
    /// (`PREDICTION_BINARY_SZ_DECIMALS`, hundredths of a contract) treats
    /// `size = 100` as one whole $1 contract (see `engine::size_scale`). Zero
    /// (default) = integer units; old records decode as 0 via serde(default).
    /// Unlike `lot_size` (an increment gate), this is the representation scale:
    /// set-once before trading, since changing it redenominates every stored
    /// quantity.
    #[serde(default)]
    pub sz_decimals: u8,
    /// Human-readable ticker / short symbol (e.g. `BTC`, `BTC-73-CPY`).
    /// Display/metadata only — the engine never reads it, so determinism is
    /// untouched. Empty for legacy records (decodes via `serde(default)`).
    /// Length-bounded by `MAX_TICKER_LEN`, enforced at the create handlers.
    #[serde(default)]
    pub ticker: String,
    /// Maximum aggregate open interest for this market, in raw base-size
    /// units (`10^-sz_decimals` whole units).
    /// Defined as `max(total_long_size, total_short_size)` across open
    /// positions on the market. Enforced before a fill mutates positions.
    /// At schema v9+, if governance tightens the cap below current OI, fills
    /// may preserve or reduce the pre-fill aggregate but may not increase it;
    /// this lets the market unwind until it is back within the configured
    /// limit. Earlier schemas preserve the strict legacy verdict. `0` disables
    /// the cap for backward compatibility.
    ///
    /// This is appended at the tail to preserve positional MessagePack
    /// compatibility for existing on-chain `MarketConfig` records.
    #[serde(default)]
    pub max_open_interest: u64,
}

/// Per-market oracle guards that live OUTSIDE `MarketConfig`, at
/// `keys::market_oracle_guards(market)` (prefix `MarketOracleGuards`).
/// Written only by the governed `AdminAction::SetOracleGuards`
/// arm; absent until then.
///
/// Why a separate record: governance executions absorb their raw writes into
/// the applied-state digest, so appending a field to `MarketConfig` would
/// change the bytes a quorum `CreateMarket` writes and with them the app hash
/// of every already-executed proposal on replay. A positional array whose
/// length depends on a value is ruled out by the `CreateMarket` encoding rule
/// ("one value, one encoding"). A new key that only exists after the gated
/// arm has run leaves every pre-gate byte untouched.
///
/// Read only once the oracle-guard gate is active
/// (`repo::UPGRADE_HEIGHT_ORACLE_GUARDS`): from then on an out-of-band
/// `OracleUpdate` is REJECTED (event `OracleUpdateRejected`) rather than
/// clipped, and an absent record (band zero) fails closed: every update after
/// the first is rejected until a real band is set. Below the gate this record
/// is inert and the schema-v9 clamp on `DEFAULT_MAX_ORACLE_DEVIATION_BPS`
/// keeps applying, so replay is unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOracleGuards {
    /// Maximum per-update move from the stored last-good price, in basis
    /// points. Zero means unset.
    pub max_oracle_deviation_bps: u32,
}

/// Per-tier fee schedule for the volume-based maker-rebate program.
///
/// `*_tenth_bps` lets the engine express sub-bps fees (e.g. 1.5 bps =
/// 15 tenth-bps) and signed maker rebates without losing precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeTier {
    /// Minimum 30-day rolling taker volume (micro-USDC) required to
    /// qualify for this tier. The lowest tier should be 0.
    pub min_30d_volume_micro_usdc: u64,
    /// Maker fee in tenth-basis-points. Negative means maker rebate.
    pub maker_fee_tenth_bps: i16,
    /// Taker fee in tenth-basis-points. The configured alpha tiers keep
    /// this non-negative so the protocol never pays takers.
    pub taker_fee_tenth_bps: i16,
}

/// Configuration for the Hyperliquidity Provider (HLP) — the
/// protocol-owned MM that absorbs bankruptcy losses at Tier 0 of the
/// bad-debt waterfall. See `docs/adl-vs-socialized-loss.md` §3.2.
///
/// Stored under `keys::HLP_CONFIG` (single global record). The HLP's
/// trading account lives at `address`; the engine treats it like any
/// other account for matching purposes but consults `min_balance_floor`
/// when settling deficits — once HLP equity drops below the floor it
/// stops absorbing and further deficits route to the per-pool IF.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HlpConfig {
    /// 20-byte address of the HLP vault account (matches the address
    /// derived from the HLP's signing key).
    #[serde(with = "crate::wire_bytes")]
    pub address: [u8; 20],
    /// Bootstrap equity (microUSDC). Captured at HLP-onboarding time
    /// and never updated; the floor is computed from this baseline so
    /// drawdowns don't move the floor up.
    pub bootstrap_balance: u64,
    /// Minimum balance HLP must retain. Below this, Tier 0 stops
    /// absorbing. Default: 60% of bootstrap (`0.6 × bootstrap_balance`).
    /// Stored absolute so the value at config-write time is durable
    /// across bootstrap_balance migrations.
    pub min_balance_floor: u64,
    /// True iff Tier 0 is enabled. When false (initial state — no HLP
    /// configured) the waterfall starts at Tier 1 (per-pool IF).
    pub enabled: bool,
}

/// Tier-2 haircut already applied for one pool in one consensus block.
///
/// Stored under `keys::socialized_loss_block_accum(pool_id)`. A record from a
/// different height is treated as zero, so the budget resets lazily without an
/// end-of-block sweep. `applied_micro_usdc` tracks cash actually debited, not
/// the requested or theoretical cap amount.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocializedLossBlockAccum {
    pub block_height: u64,
    pub applied_micro_usdc: u64,
}

// ---------------------------------------------------------------------------
// Actions (wire types — field order is the MessagePack wire layout)
// ---------------------------------------------------------------------------

/// The top-level `Action` enum is generated alongside its wire
/// discriminants by `codec::define_actions!`, so the variant set has a
/// single source of truth. Re-exported here to keep the
/// `crate::types::Action` path stable for all callers.
pub use crate::codec::Action;

/// One optional limb in a whole-position protection bracket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerLimb {
    pub trigger_price: u64,
    pub max_slippage_bps: TriggerSlippageBps,
    pub client_trigger_id: Option<ClientTriggerId>,
}

/// Atomically replace the complete stop-loss/take-profit bracket for an
/// existing standalone-perpetual position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetPositionTriggers {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub expected_position_epoch: PositionEpoch,
    pub stop_loss: Option<TriggerLimb>,
    pub take_profit: Option<TriggerLimb>,
    pub client_group_id: Option<ClientTriggerGroupId>,
}

/// Remove the active position-protection bracket for one position epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelPositionTriggers {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub expected_position_epoch: PositionEpoch,
}

/// Test/admin action — force-runs `run_liquidations` immediately.
/// See `Action::RunLiquidationSweep` for rationale.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunLiquidationSweep {
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

/// Maximum owners admitted in one proposer-injected liquidation action.
///
/// This is the canonical decode/preflight bound, not the schema-v9 execution
/// budget: v9 admits at most 32 complete position rows per Tier-2 slot and
/// deterministically defers the rest. A larger owner vector cannot increase
/// useful work (position-less and deferred owners are retried by the daemon),
/// but it can make proposal decoding and owner preflight unbounded. Go
/// proposal admission mirrors this constant and pins the largest canonical
/// MessagePack payload in tests.
pub const MAX_LIQUIDATE_ACCOUNTS_OWNERS: usize = 256;

/// Proposer-injected liquidation candidates (out-of-band liquidations).
///
/// UNSIGNED internal action: the block proposer's liquidation daemon
/// detects undercollateralized accounts off-consensus and injects this
/// tx at the top of its own proposal via PrepareProposal. It carries no
/// authority — per-owner validity is re-derived from state inside the
/// handler (only a definite `InsufficientMargin` liquidates; healthy or
/// indeterminate owners no-op). The envelope's pubkey and signature must
/// be all-zero (canonical form), and Go CheckTx rejects the action type
/// unconditionally so it can never enter the mempool: the only ingress
/// is a block proposal, whose proposer signature already covers it.
///
/// At schema v9 the owner vector is canonical (strictly lexicographically
/// sorted and duplicate-free) and contains at most
/// [`MAX_LIQUIDATE_ACCOUNTS_OWNERS`] entries. Consensus preflights the
/// position count for each owner and executes only the largest
/// complete-owner prefix that fits the per-transaction position budget.
/// Already-saved and position-less owners remain deterministic no-ops.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidateAccounts {
    pub owners: Vec<[u8; 20]>,
}

/// Test/admin action — force-runs a funding tick on one market.
/// See `Action::RunFundingTick` for rationale.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunFundingTick {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

/// Pick a per-account override on the initial-margin ratio for one
/// market. The engine uses `max(market.im_bps, user_im_bps)` on
/// every IM-gated check (place order, withdraw post-trade margin
/// review, scenario IM enumeration), so users can choose a more
/// conservative IM but never circumvent the market's risk floor.
///
/// `user_im_bps == 0` clears the override (equivalent to "use the
/// market default"). The engine validates `user_im_bps == 0 ||
/// user_im_bps >= market.im_bps` at admission; otherwise rejects
/// with `ExecError::UserLeverageBelowMarketIm`.
///
/// This is a user-signed action (not relayer-signed). Each owner
/// can only set their own override — the dispatcher enforces
/// `signer == owner` before the handler runs.
///
/// BE-16, 2026-05-03.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetUserMarketLeverage {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub market: MarketId,
    /// Initial margin ratio in basis points the user wants to use
    /// for this market. `0` clears the override; otherwise must be
    /// `>= market.im_bps`.
    pub user_im_bps: u32,
}

/// Close an entire position on a market by placing an opposite-side
/// immediate-or-cancel order at oracle±spread. Idempotent on
/// already-closed positions: calling on a zero position returns code=0
/// without emitting events. Semantically equivalent to a high-priority
/// market order but replaces the friction of opposite-side placement or
/// order cancellation. User-signed (owner must match position owner).
///
/// S49, Auros documentation plan (2026-05-09) §1 line 11; matches
/// the Hyperliquid pattern.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClosePosition {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
}

/// Immediate-or-cancel order that crosses the book at the best available price.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketOrder {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub side: Side,
    /// Desired fill quantity in base-asset units.
    pub quantity: u64,
    /// Optional client-assigned ID echoed in events for correlation.
    /// Active resting orders are unique per owner/client_order_id; reusing
    /// an ID while an earlier order is still live is rejected.
    pub client_order_id: Option<u64>,
}

/// One leg inside a native all-or-revert basket. The engine executes every leg
/// as a fill-or-kill limit order; if any leg cannot fully fill at `price` or
/// better, the whole transaction rolls back through the normal tx overlay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtomicBasketLeg {
    pub market: MarketId,
    pub side: Side,
    /// Worst acceptable execution price in micro-USDC. Buy legs will not pay
    /// above this price; sell legs will not sell below it.
    pub price: u64,
    pub quantity: u64,
    pub client_order_id: Option<u64>,
    #[serde(default)]
    pub reduce_only: bool,
}

/// Native all-or-revert multi-leg order for impact-market baskets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtomicBasketOrder {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub legs: Vec<AtomicBasketLeg>,
    /// Aggregate slippage budget (bps): enforced per-leg-measured, aggregate-decided across legs; `0` disables.
    #[serde(default)]
    pub max_slippage_bps: u32,
}

/// Place a resting limit order on the book. May partially fill immediately.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaceOrder {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub side: Side,
    /// Limit price in micro-USDC per unit of the base asset.
    pub price: u64,
    /// Order size in base-asset units.
    pub quantity: u64,
    /// Optional client-assigned ID echoed in events for correlation.
    pub client_order_id: Option<u64>,
    /// Post-only flag: if the order would cross the book on placement, the
    /// engine rejects with `PostOnlyWouldCross` instead of taking. Used by
    /// makers who want to guarantee maker-side fills. `serde(default)` so
    /// pre-existing wire records decode with `false`.
    #[serde(default)]
    pub post_only: bool,
    /// Reduce-only flag: order may only reduce an existing position;
    /// rejected if same-side as the current position (would increase) or
    /// no position exists. Clamped to position size if the order would
    /// over-close (flip direction). `serde(default)` so old records
    /// decode with `false`.
    #[serde(default)]
    pub reduce_only: bool,
    /// Time-in-force policy. Defaults to `Gtc` for backward compat with
    /// pre-TIF wire records. `Ioc` drops unfilled quantity after crossing.
    #[serde(default)]
    pub time_in_force: TimeInForce,
}

/// Cancel a resting order. Only the owner (or an authorized agent) may cancel.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelOrder {
    pub order_id: OrderId,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
}

/// Cancel a resting order by client-assigned order id.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelClientOrder {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub client_order_id: u64,
}

/// Cancel all resting orders for an account. If `market` is set, only orders
/// on that market are removed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelAllOrders {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    #[serde(default)]
    pub market: Option<MarketId>,
}

/// Atomically cancel a resting order and place a replacement order.
///
/// Exactly one of `cancel_order_id` or `cancel_client_order_id` must be set.
/// The replacement uses the same semantics as `PlaceOrder`: it may cross,
/// be post-only/reduce-only, and can specify GTC/IOC/FOK.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelReplaceOrder {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    #[serde(default)]
    pub cancel_order_id: Option<OrderId>,
    #[serde(default)]
    pub cancel_client_order_id: Option<u64>,
    pub market: MarketId,
    pub side: Side,
    pub price: u64,
    pub quantity: u64,
    pub client_order_id: Option<u64>,
    #[serde(default)]
    pub post_only: bool,
    #[serde(default)]
    pub reduce_only: bool,
    #[serde(default)]
    pub time_in_force: TimeInForce,
}

/// Amend a resting order without changing its exchange order id.
///
/// `new_quantity` is the new total accepted quantity, not the remaining
/// quantity. It must be greater than the order's cumulative maker fill. Same
/// price size reductions preserve queue priority; price changes and size
/// increases reset priority.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmendOrder {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub order_id: OrderId,
    #[serde(default)]
    pub new_price: Option<u64>,
    #[serde(default)]
    pub new_quantity: Option<u64>,
}

/// Push a new oracle (mark) price. Requires an authorized oracle signer
/// AND a strictly increasing `publish_time_ms` per market.
///
/// **Why the timestamp check** (added 2026-04-23, audit finding B3):
/// before this field existed, `OracleUpdate` relied only on the envelope
/// nonce for transaction replay protection, while the handler read no
/// previous oracle timestamp before overwriting the price. A different
/// previously-signed update carrying stale price data could rewind the
/// mark price to a historical value, triggering mass liquidations or
/// arbitrage at stale prices.
///
/// The field is the Pyth-style publish time of the price signal
/// (ms since Unix epoch). The handler rejects any update whose
/// `publish_time_ms` is ≤ the most recent stored value for the
/// same market. Genesis / first-update carries any timestamp; the
/// check only kicks in from the second update onward.
///
/// `#[serde(default)]` on `publish_time_ms` so on-chain records
/// written before this field existed decode with `publish_time_ms = 0`,
/// which passes the monotonicity check exactly once (and fails
/// all replays of that seed update thereafter).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleUpdate {
    pub market: MarketId,
    /// New mark price in micro-USDC.
    pub price: u64,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// Oracle's own publish timestamp in ms since Unix epoch.
    /// Must be strictly greater than the last accepted update's
    /// `publish_time_ms` for the same market.
    #[serde(default)]
    pub publish_time_ms: u64,
}

/// A source identifier scoped to one policy epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSourceId(pub u32);

/// A monotonically increasing oracle-policy epoch identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OraclePolicyVersion(pub u64);

/// A normalized observation attested by the exact source key in its policy.
/// The envelope signature authenticates the relay, not the external provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitOracleObservation {
    pub market: MarketId,
    pub policy_version: OraclePolicyVersion,
    pub source_id: OracleSourceId,
    pub publish_time_ms: u64,
    pub price_micro: u64,
    pub confidence_micro: Option<u64>,
    #[serde(with = "crate::wire_bytes")]
    pub evidence_digest: [u8; 32],
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

/// Canonical MessagePack policy bytes and the first block that may use them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigureOraclePolicy {
    pub effective_height: u64,
    #[serde(with = "crate::wire_bytes::vec")]
    pub bundle: Vec<u8>,
}

/// Composite-CEX price update — BE-31 Phase B's third source for the
/// multi-source mark-price median. Carries the median (or VWAP) of N
/// off-chain CEX feeds (Binance / OKX / Bybit / Coinbase) computed by
/// a separate off-chain feeder process.
///
/// **Why it's separate from `OracleUpdate`**:
///   1. Different signer set — composite uses a feeder-specific
///      allowlist, not the Pyth oracle relay's. Different trust model.
///   2. Different staleness gate — composite is polled every ~1s vs.
///      Pyth's per-block cadence; rejected from the median when older
///      than `cex_composite_staleness_ms` (default 30s).
///   3. Different aggregation — Pyth oracle is a single value; the
///      composite is explicitly a median across multiple venues, with
///      `n_sources` carried for observability.
///
/// Same monotonicity-of-publish-time replay guard as `OracleUpdate`
/// (audit B3, 2026-04-23).
///
/// Stored under `keys::CexCompositePrice` keyspace, indexed by market.
/// Read by `compute_median_mark_price` as the third source when the
/// market is in `MarkSourceMode::Median`. Has no effect on
/// `OracleOnly` markets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleUpdateComposite {
    pub market: MarketId,
    /// Composite price in micro-USDC (median or VWAP of `n_sources`
    /// off-chain CEX feeds, computed off-chain by the feeder).
    pub price: u64,
    /// Number of CEX feeds that went into the composite. Carried for
    /// observability — a composite from 1 venue is much weaker
    /// signal than one from 4. Field is informational; the engine
    /// doesn't gate on it.
    #[serde(default)]
    pub n_sources: u8,
    /// Authorized feeder signer (20 bytes).
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// Feeder's own publish timestamp in ms since Unix epoch. Must
    /// be strictly greater than the last accepted update's
    /// `publish_time_ms` for the same market — replay guard.
    #[serde(default)]
    pub publish_time_ms: u64,
}

/// Direct deposit — requires **relayer authorization**.
///
/// Previously described as "testing/internal" with no authorization check
/// beyond `signer == owner`, which was an unauthenticated mint primitive:
/// any signed user could credit themselves arbitrary balance. See audit
/// finding B1 (2026-04-23). The handler now requires
/// `is_relayer_authorized(signer)`.
///
/// In production the primary deposit path is [`ConfirmDeposit`] (which
/// additionally dedupes on a Solana tx signature). This direct action
/// remains available to the relayer for test bootstraps and the unusual
/// cases where no Solana sig exists (e.g. migration/genesis-adjacent
/// credits). External callers must use [`ConfirmDeposit`] instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deposit {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    /// Amount in micro-USDC.
    pub amount: u64,
    /// Relayer signer. Must be an authorized relayer; enforced by
    /// `handle_deposit`. Added 2026-04-23 per audit B1.
    #[serde(default)]
    pub signer: [u8; 20],
}

/// Direct withdrawal — requires **relayer authorization**.
///
/// Previously described as "testing/internal" with no authorization check
/// beyond `signer == owner`, which let any user debit their balance with
/// no off-chain counterparty (a silent burn). See audit finding B2
/// (2026-04-23). The handler now requires
/// `is_relayer_authorized(signer)`.
///
/// External users move funds out via the two-phase [`WithdrawRequest`] →
/// relayer [`ConfirmWithdrawal`] path. This direct action remains
/// available to the relayer for administrative adjustments (e.g.
/// refunding a stuck position) and for integration tests.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Withdraw {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    /// Amount in micro-USDC.
    pub amount: u64,
    /// Relayer signer. Must be an authorized relayer; enforced by
    /// `handle_withdraw`. Added 2026-04-23 per audit B2.
    #[serde(default)]
    pub signer: [u8; 20],
}

/// Admin action to register a market with its risk parameters.
/// Requires relayer authorization (signer must be an authorized relayer).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateMarket {
    pub market: MarketId,
    pub im_bps: u32,
    pub mm_bps: u32,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// Funding interval in milliseconds. 0 = funding disabled.
    pub funding_interval_ms: u64,
    /// Maximum absolute funding rate in basis points per interval. Engine
    /// admission rejects values above 3,000 bps (30%).
    pub max_funding_rate_bps: u32,
    /// Bad-debt pool the market belongs to. Markets in different pools
    /// are insulated from each other's liquidation cascades — a residual
    /// in pool 1 cannot ADL profitable counterparties holding only
    /// positions in pool 2 (see `iter_positions(pool_id)` in
    /// `absorb_via_adl`). Defaults to 0; existing markets continue to
    /// share pool 0 unless explicitly placed elsewhere.
    #[serde(default)]
    pub pool_id: u8,
    /// Published size scale: `quantity` is in units of `10^-sz_decimals`
    /// base asset. Execution-relevant — the engine divides notional/PnL/margin
    /// by `10^sz_decimals` (see `MarketConfig::sz_decimals`). Bounded by
    /// `MAX_SZ_DECIMALS` at creation. Mandatory and immutable thereafter: there
    /// is no `UpdateMarketFees` lever to re-denominate a live market, since
    /// changing the scale would redenominate every stored quantity. NO
    /// `serde(default)` — a wire envelope that omits this field is rejected
    /// at decode, which is what makes the scale a required creation input.
    pub sz_decimals: u8,
    /// Human-readable ticker / short symbol (e.g. `BTC`). Display/metadata
    /// only — the engine never reads it. Mandatory like `sz_decimals` (NO
    /// `serde(default)`, so an omitting payload is rejected at decode), but an
    /// empty string is accepted; length is capped at `MAX_TICKER_LEN`.
    pub ticker: String,
    /// Aggregate market open-interest cap in contracts. `0` disables the cap.
    ///
    /// Always serialized, exactly like `pool_id` above: the v2 encoding of
    /// `CreateMarket` is a 12-element array whatever the cap's value. The
    /// `serde(default)` keeps released 11-element payloads decodable as
    /// uncapped, which is what makes this a decode-compatible tail; but a
    /// serializer must never make the array's *length* depend on the field's
    /// *value*. In a positional MessagePack layout that would make the wire
    /// format value-dependent, and any field appended after this one would be
    /// silently shifted into slot 11 whenever the cap happened to be zero.
    /// One value, one encoding.
    #[serde(default)]
    pub max_open_interest: u64,
}

impl Default for CreateMarket {
    /// Conservative defaults that match the private-alpha seed config
    /// (`scripts/seed.ts`): 33.34% IM, 16.67% MM, 5/2 bps fees, 60 s funding
    /// cadence with a 30% per-interval cap, pool 0. Tests that just
    /// need *some* CreateMarket instance can `..Default::default()`
    /// instead of repeating the full struct literal.
    fn default() -> Self {
        Self {
            market: 0,
            im_bps: 3334,
            mm_bps: 1667,
            taker_fee_bps: 5,
            maker_fee_bps: 2,
            signer: [0u8; 20],
            funding_interval_ms: 60_000,
            max_funding_rate_bps: 3000,
            pool_id: 0,
            sz_decimals: 0,
            ticker: String::new(),
            max_open_interest: 0,
        }
    }
}

/// User requests a USDC withdrawal to a Solana address.
/// Debits balance immediately and creates a pending withdrawal record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawRequest {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub amount: u64,
    /// Solana destination public key (Ed25519, 32 bytes).
    #[serde(with = "crate::wire_bytes")]
    pub solana_destination: [u8; 32],
}

/// Position of a single USDC transfer inside its Solana transaction: the
/// top-level instruction index plus, for a transfer nested under a CPI, the
/// inner instruction index. Two transfers in one transaction share a
/// signature and differ only here, so the deposit dedup key carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositLocator {
    /// Index of the top-level instruction within the transaction message.
    pub top_index: u16,
    /// Index within that instruction's inner (CPI) instructions; `None` when
    /// the transfer is itself the top-level instruction.
    pub inner_index: Option<u16>,
}

impl DepositLocator {
    /// Reserved inner-index byte value for `inner_index == None`.
    // Widen both index fields to u32 if a Solana inner-instruction count
    // ever nears 65535.
    const INNER_SENTINEL: u16 = u16::MAX;

    /// Locator for a `ConfirmDeposit`/`FailDeposit` whose wire `locator` is
    /// absent (pre-locator bytes): top-level instruction 0, no inner.
    pub const LEGACY: Self = Self {
        top_index: 0,
        inner_index: None,
    };

    /// Fixed-width big-endian dedup-key suffix `[top(2)][inner-or-sentinel(2)]`.
    pub fn id_suffix(&self) -> [u8; 4] {
        let mut suffix = [0u8; 4];
        suffix[0..2].copy_from_slice(&self.top_index.to_be_bytes());
        suffix[2..4].copy_from_slice(
            &self
                .inner_index
                .unwrap_or(Self::INNER_SENTINEL)
                .to_be_bytes(),
        );
        suffix
    }
}

/// Relayer confirms an on-chain USDC deposit from Solana.
/// Credits the derived internal account.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmDeposit {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub amount: u64,
    /// Solana transaction signature (typically 64 bytes) for idempotency.
    #[serde(with = "crate::wire_bytes::vec")]
    pub solana_tx_sig: Vec<u8>,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// Instruction locator within the signature. Absent (`nil`) on
    /// pre-locator wire bytes, which fall back to [`DepositLocator::LEGACY`].
    #[serde(default)]
    pub locator: Option<DepositLocator>,
}

/// Relayer confirms a USDC withdrawal was sent on Solana.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmWithdrawal {
    pub withdrawal_id: u64,
    /// Solana transaction signature (typically 64 bytes).
    #[serde(with = "crate::wire_bytes::vec")]
    pub solana_tx_sig: Vec<u8>,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

/// Relayer marks a withdrawal as permanently failed (e.g. Solana transfer
/// rejected, destination account closed).  Refunds the debited balance
/// back to the owner.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailWithdrawal {
    pub withdrawal_id: u64,
    /// Human-readable reason for the failure (for event logging).
    pub reason: String,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

// ---------------------------------------------------------------------------
// W28-20 bridge custody: receipt-gated terminal withdrawal actions
// ---------------------------------------------------------------------------
//
// Operator-multisig phase. The legacy relayer `ConfirmWithdrawal` (0x0A) /
// `FailWithdrawal` (0x0B) trust an authorized-relayer assertion; these new
// action_types carry a `bridge_core::BridgeReceiptV1` and its operator
// ed25519 proof, and the engine verifies the quorum in consensus before
// crediting/refunding — no trusted courier assertion. Additive: the legacy
// actions stay decodable for the shadow/canary phase and rollback.
// Design: ProofOfBrain delivery/epics/bridge-contract.md (§Withdrawal).

/// Engine-side mirror of `bridge_core::BridgeReceiptV1` (fixed 327-byte wire
/// form). Carried by the receipt-gated actions; the engine rebuilds the
/// `BridgeReceiptV1`, re-encodes, and verifies the operator quorum signed
/// exactly those bytes. Every field is signed, so tampering fails closed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeWithdrawalReceipt {
    /// `bridge_core::DeploymentId` — pins Proof/Solana genesis, program, mint.
    #[serde(with = "crate::wire_bytes")]
    pub deployment_id: [u8; 32],
    /// `SHA256(Borsh(WithdrawalAuthorizationV1))`.
    #[serde(with = "crate::wire_bytes")]
    pub authorization_digest: [u8; 32],
    pub withdrawal_id: u64,
    /// `1 = Paid`, `2 = Cancelled`.
    pub terminal_state: u8,
    /// `bridge_core::VaultTier` wire byte.
    pub vault_tier: u8,
    #[serde(with = "crate::wire_bytes")]
    pub proof_owner: [u8; 20],
    #[serde(with = "crate::wire_bytes")]
    pub destination_owner: [u8; 32],
    #[serde(with = "crate::wire_bytes")]
    pub destination_token_acct: [u8; 32],
    pub amount_micro_usdc: u64,
    pub fee_micro_usdc: u64,
    pub authorization_signer_epoch: u64,
    /// Solana tx signature (64 bytes). `Vec<u8>` because serde has no `[u8; 64]`.
    #[serde(with = "crate::wire_bytes::vec")]
    pub solana_tx_signature: Vec<u8>,
    pub finalized_slot: u64,
    #[serde(with = "crate::wire_bytes")]
    pub finalized_blockhash: [u8; 32],
    /// `1 = operator m-of-n`, `2 = validator stake`.
    pub receipt_quorum_kind: u8,
    pub receipt_authority_epoch: u64,
}

/// Operator ed25519 proof wrapper — mirrors
/// `bridge_core::ReceiptProofV1::OperatorEd25519`. Bitmap plus one signature
/// per set bit in ascending registry order; structure and every signature
/// are checked by `bridge-core`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorReceiptProof {
    /// `ceil(registry_len / 8)` bytes; unused high bits zero.
    #[serde(with = "crate::wire_bytes::vec")]
    pub signer_bitmap: Vec<u8>,
    /// One 64-byte ed25519 signature per set bit, ascending set-bit order.
    pub signatures: Vec<Vec<u8>>,
}

/// Receipt-gated confirmation: a finalized `Paid` `BridgeReceiptV1`. Replaces
/// the relayer assertion in `ConfirmWithdrawal`. Permissionless to submit —
/// the operator quorum in the receipt is the authority, not the envelope
/// signer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmWithdrawalReceipt {
    pub receipt: BridgeWithdrawalReceipt,
    pub proof: OperatorReceiptProof,
}

/// Receipt-gated failure: a finalized `Cancelled` `BridgeReceiptV1`. Replaces
/// the free-text `FailWithdrawal`; refunds `amount` only against a positive
/// on-chain cancellation proof, never a timeout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailWithdrawalReceipt {
    pub receipt: BridgeWithdrawalReceipt,
    pub proof: OperatorReceiptProof,
}

/// Records the operator-quorum-signed `WithdrawalAuthorizationV1` for a pending
/// withdrawal, binding its digest to the record so a terminal receipt can only
/// settle an authorization the quorum actually issued. Permissionless to submit
/// — the operator quorum in `proof` is the authority, not the envelope signer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorizeWithdrawal {
    /// The canonical `WithdrawalAuthorizationV1` bytes the quorum signed
    /// (`bridge_core::WithdrawalAuthorizationV1::encode`, fixed 221 bytes).
    #[serde(with = "crate::wire_bytes::vec")]
    pub authorization: Vec<u8>,
    pub proof: OperatorReceiptProof,
}

/// Engine operator receipt registry (W28-20 #28): the epoch-pinned named
/// operator set the terminal receipts are verified against. Stored per epoch
/// (W28-20 #316); presence of any registry activates the operator-multisig
/// receipt path, and the highest stored epoch is the current one. `threshold`
/// is *m* (m distinct members), `operator_keys.len()` is *n*. Persistence
/// rejects a duplicate/oversized roster or an out-of-range threshold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorReceiptRegistry {
    /// `bridge_core::DeploymentId` bytes every receipt must match.
    #[serde(with = "crate::wire_bytes")]
    pub deployment_id: [u8; 32],
    /// The registry epoch receipts must be pinned to.
    pub epoch: u64,
    /// *m* — distinct operator members required.
    pub threshold: u16,
    /// The *n* operator ed25519 verification keys, in registry index order.
    pub operator_keys: Vec<[u8; 32]>,
}

/// Why a Solana deposit was rejected by the relayer. Mirrors the small
/// closed set of failure modes the bridge can detect off-chain — anything
/// else falls under [`FailDepositReason::Other`] with a free-text reason
/// in the action's `note` field.
///
/// Wire form: enum discriminant. Adding a variant is a wire-compatible
/// change for old decoders ONLY when appended at the end (msgpack maps
/// missing variants to a decode error). Treat the existing four as
/// stable; bump a new constant if you need to extend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailDepositReason {
    /// Solana transaction couldn't be parsed (invalid instruction layout,
    /// missing token-2022 metadata, etc.).
    MalformedTx,
    /// Deposit was for a token mint we don't accept.
    UnsupportedToken,
    /// Amount under the bridge's dust threshold; processing cost would
    /// exceed the deposit value.
    BelowMinimum,
    /// Catch-all for relayer-side errors not covered above. Keep usage
    /// rare so the breakdown stays meaningful in metrics.
    Other,
}

impl fmt::Display for FailDepositReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FailDepositReason::MalformedTx => f.write_str("malformed_tx"),
            FailDepositReason::UnsupportedToken => f.write_str("unsupported_token"),
            FailDepositReason::BelowMinimum => f.write_str("below_minimum"),
            FailDepositReason::Other => f.write_str("other"),
        }
    }
}

/// Relayer marks a Solana deposit signature as permanently failed.
/// The user is NOT credited — they simply never see the deposit. The
/// signature is recorded under a separate "failed" key so any future
/// ConfirmDeposit OR FailDeposit referencing the same signature is a
/// silent no-op (idempotent).
///
/// `solana_signature` uses `Vec<u8>` (not `[u8; 64]`) so the wire bytes
/// are byte-for-byte identical to the matching `ConfirmDeposit.solana_tx_sig`
/// — the deduplication relies on this identity. Solana sigs are typically
/// 64 bytes; the engine does not currently length-validate but the
/// gateway should reject anything else.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailDeposit {
    /// Solana transaction signature, same bytes that the original
    /// `ConfirmDeposit.solana_tx_sig` would carry. Lookup against the
    /// processed-deposits set is byte-equality.
    #[serde(with = "crate::wire_bytes::vec")]
    pub solana_signature: Vec<u8>,
    /// Structured reason for failure (for event-stream metrics + ops UX).
    pub reason: FailDepositReason,
    /// Authorized relayer signer (mirrors `ConfirmDeposit.signer`). The
    /// envelope signer's derived address must equal this AND must be on
    /// the relayer allowlist; otherwise `UnauthorizedRelayer`.
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// Instruction locator within the signature, matching the failed
    /// transfer's `ConfirmDeposit.locator`. Absent (`nil`) on pre-locator
    /// wire bytes, which fall back to [`DepositLocator::LEGACY`].
    #[serde(default)]
    pub locator: Option<DepositLocator>,
}

/// Approve a delegate keypair ("agent wallet") to trade on the owner's behalf.
/// The agent can place/cancel orders but CANNOT withdraw or move funds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApproveAgent {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    #[serde(with = "crate::wire_bytes")]
    pub agent_pubkey: [u8; 32],
}

/// Revoke a previously approved agent wallet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeAgent {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    #[serde(with = "crate::wire_bytes")]
    pub agent_pubkey: [u8; 32],
}

/// Create a new sub-account under an owner. The derived address is computed
/// via `derive_sub_account(owner, sub_account_id)`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateSubAccount {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub sub_account_id: u32,
    #[serde(with = "crate::wire_bytes")]
    pub name: [u8; 32],
}

/// Transfer balance between two addresses. At least one side must be the
/// master owner (the address that created the sub-accounts). The source
/// must pass the maintenance-margin solvency check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubAccountTransfer {
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    #[serde(with = "crate::wire_bytes")]
    pub from: [u8; 20],
    #[serde(with = "crate::wire_bytes")]
    pub to: [u8; 20],
    pub amount: u64,
}

/// Registry row for a sub-account.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubAccount {
    #[serde(with = "crate::wire_bytes")]
    pub master: [u8; 20],
    pub sub_account_id: u32,
    #[serde(with = "crate::wire_bytes")]
    pub address: [u8; 20],
    #[serde(with = "crate::wire_bytes")]
    pub name: [u8; 32],
    pub created_height: u64,
}

/// Admin action to create a new impact market family. Atomically registers
/// the 4 child markets (CPY / CPN / EBY / EBN) with sequential IDs starting
/// at `child_market_base` and writes the [`ImpactMarketInfo`] record.
/// Requires relayer authorization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateImpactMarket {
    pub impact_market_id: ImpactMarketId,
    /// Underlying perp market (book 1). Must already exist with `kind = Perp`.
    pub underlying_market: MarketId,
    /// Starting ID for the 4 child markets. They are allocated as:
    /// `child_market_base+0` = CPY, `+1` = CPN, `+2` = EBY, `+3` = EBN.
    /// None of these market IDs may already exist.
    pub child_market_base: MarketId,
    pub question: String,
    pub deadline_ms: u64,
    pub resolution_window_ms: u64,
    /// Initial margin ratio for the 2 conditional-perp child books (basis points).
    /// Prediction-binary books don't use bps IM — their IM is computed from payoff.
    pub im_bps: u32,
    /// Maintenance margin ratio for conditional-perp child books.
    pub mm_bps: u32,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    /// Funding interval for the conditional-perp child books (ms). 0 = disabled.
    pub funding_interval_ms: u64,
    pub max_funding_rate_bps: u32,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// BE-54: how this event's YES/NO outcome is determined at deadline.
    /// Optional — `None` (or absent on the wire, via `serde(default)`) means
    /// `RelayerAttested` (the legacy default — the resolver supplies the
    /// outcome). Two auto-resolve modes derive YES/NO from an on-chain
    /// oracle; in those modes `ResolveImpactMarket.outcome` becomes a verifiable
    /// assertion (engine recomputes and rejects on mismatch). Field at the
    /// END of the struct so old SDK clients (12-element arrays) continue
    /// to decode cleanly via the wire's `serde(default)` rule.
    #[serde(default)]
    pub oracle_source: Option<EventOracleSource>,
    /// Optional frontend-facing event body text. Kept on the admin action and
    /// creation event for off-chain indexers, but deliberately not stored in
    /// [`ImpactMarketInfo`] consensus state.
    #[serde(default)]
    pub description: String,
    /// Optional resolution criteria text. Kept on the admin action and creation
    /// event for off-chain indexers, but deliberately not stored in
    /// [`ImpactMarketInfo`] consensus state.
    #[serde(default)]
    pub rules: String,
}

/// Create a standalone event: mints two prediction-binary books (EBY at
/// `child_market_base+0`, EBN at `+1`) under a new [`EventInfo`], with no
/// underlying perp and no conditional legs (G17). Requires relayer
/// authorization; governed like `CreateImpactMarket`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateEvent {
    pub event_id: EventId,
    /// Starting id for the 2 binary child markets: `base+0` = EBY, `+1` = EBN.
    pub child_market_base: MarketId,
    /// Risk/insurance pool the two books belong to.
    pub pool_id: u8,
    pub question: String,
    pub settlement_ms: u64,
    pub resolution_window_ms: u64,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// How the outcome is determined. `None` => `RelayerAttested`.
    #[serde(default)]
    pub oracle_source: Option<EventOracleSource>,
    /// Off-chain event body text (not stored in consensus state).
    #[serde(default)]
    pub description: String,
    /// Off-chain resolution criteria text (not stored in consensus state).
    #[serde(default)]
    pub rules: String,
}

/// Admin action to resolve an impact-market event. Settles the winning
/// conditional-perp book and voids the loser; cash-settles both binary books
/// to $1 (winner) / $0 (loser). Requires relayer authorization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveImpactMarket {
    pub impact_market_id: ImpactMarketId,
    pub outcome: Outcome,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

/// Resolve a standalone event. Settles its two prediction-binary books to
/// Yes/No only (there is no Void), reads no underlying price, and is
/// authorized by the market-parameters key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveEvent {
    pub event_id: EventId,
    pub outcome: Outcome,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
}

/// Update a subset of `MarketConfig` fields on an existing market.
/// Every tunable is `Option<T>` so the caller only supplies the fields
/// they mean to change; `None` leaves the current value untouched.
///
/// This is the admin lever that lets us tighten the funding cap, set
/// position limits, or calibrate fees on a live market — previously
/// the only way to change those was a chain rebase, which surfaced on
/// 2026-04-20 when we saw BTC funding spike to −1608 bps under the
/// seed-time `max_funding_rate_bps = 3000` cap and had no way to
/// dampen it without wiping state.
///
/// Requires relayer authorization. Fields that would violate an
/// invariant of the existing market (e.g. `mm_bps > im_bps`) are
/// rejected with `ExecError::InvalidMarketConfig`.
///
/// Fields left intentionally immutable (not exposed here):
///   * `market` — identity
///   * `kind` — changing a market's kind would break the
///     book's accounting model (perp vs conditional perp vs binary).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMarketFees {
    pub market: MarketId,
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// New taker fee in basis points. `None` = leave unchanged.
    #[serde(default)]
    pub taker_fee_bps: Option<u32>,
    /// New maker fee in basis points. `None` = leave unchanged.
    #[serde(default)]
    pub maker_fee_bps: Option<u32>,
    /// New max funding rate cap in basis points per interval, bounded to
    /// 3,000 bps (30%). `None` = leave unchanged. Setting to 0 forces the
    /// computed funding rate to zero; `funding_interval_ms = 0` disables the
    /// funding schedule.
    #[serde(default)]
    pub max_funding_rate_bps: Option<u32>,
    /// New funding interval in ms. `None` = leave unchanged.
    #[serde(default)]
    pub funding_interval_ms: Option<u64>,
    /// New per-account position cap in contracts. `None` = leave
    /// unchanged. Setting to 0 disables the cap.
    #[serde(default)]
    pub max_position_size: Option<u64>,
    /// New default order TTL in milliseconds. `None` = leave
    /// unchanged. Setting to 0 disables auto-cancel sweeps for this
    /// market. Operators tune this per-market via the relayer-signed
    /// admin action; recommended: 60_000 (1 minute) for perps, longer
    /// for impact markets whose MMs re-quote less often. Motivated
    /// by the 2026-04-23 incident where alice's orphaned-order
    /// backlog locked $6M IM against $2.9M equity and made the BTC
    /// book permanently one-sided — see `run_order_expiry` docstring.
    #[serde(default)]
    pub default_ttl_ms: Option<u64>,
    /// Flip the net-delta portfolio margin flag on this market.
    /// `None` = leave unchanged. `Some(true)` enables net-delta
    /// grouping for firing legs on this market's underlying; `Some(false)`
    /// falls back to per-leg scenario margin.
    ///
    /// See `MarketConfig::net_delta_margin` for the semantics. Flipping
    /// this on a live market changes MM/IM for existing positions
    /// on the next check — operators should model the impact before
    /// flipping to `false` (could push accounts under MM) but
    /// flipping to `true` is always safe (can only relieve MM).
    #[serde(default)]
    pub net_delta_margin: Option<bool>,
    /// Tick size in micro-USDC. `None` = leave unchanged. Setting to 0
    /// disables the tick check (any price accepted). BE-48.
    #[serde(default)]
    pub tick_size: Option<u64>,
    /// Lot size in contracts. `None` = leave unchanged. Setting to 0
    /// disables the lot check (any quantity accepted). BE-48.
    #[serde(default)]
    pub lot_size: Option<u64>,
    /// Primary oracle signer for this market. `None` = leave unchanged.
    /// `Some(addr)` sets the primary to `addr`. BE-50.
    ///
    /// Wire-format note: msgpack via rmp-serde collapses `Option<Option<T>>`
    /// in positional arrays — both `None` and `Some(None)` encode as `nil`,
    /// so we can't distinguish "leave alone" from "clear" with bare option
    /// nesting. To clear the primary without re-creating the market, send
    /// `Some([0u8; 20])` — the engine treats the all-zero address as a
    /// "clear primary" sentinel (mirrors the `FEE_OVERRIDE_REVERT_SENTINEL`
    /// pattern from BE-46.1). Real signer addresses are derived from
    /// keccak256 of an Ed25519 public key, which collides with the all-zero
    /// address only with negligible probability — safe to use as a sentinel.
    ///
    /// Alternative: setting `oracle_staleness_ms = 0` disables the gate
    /// entirely (any authorized relayer accepted) without disturbing the
    /// primary slot — useful when you want to suspend the gate temporarily.
    #[serde(default)]
    pub primary_oracle_signer: Option<[u8; 20]>,
    /// Oracle staleness threshold in ms. `None` = leave unchanged.
    /// Only consulted when `primary_oracle_signer` is set; see
    /// `MarketConfig::oracle_staleness_ms`. BE-50.
    #[serde(default)]
    pub oracle_staleness_ms: Option<u64>,
    /// New mark-source mode (BE-31 Phase A). `None` = leave unchanged.
    /// `Some(MarkSourceMode::Median)` opts the market into the
    /// multi-source median path. Has no effect on impact-family
    /// markets - those always read EWMA per the no-oracle-MTM redesign.
    ///
    /// Operational note: flipping `OracleOnly -> Median` on a live
    /// market that has a thin or absent book returns oracle-only at
    /// the floor - same value as the old path until the book is
    /// liquid enough to pass the spread guard. Safe to roll out
    /// per-market; no chain wipe.
    #[serde(default)]
    pub mark_source_mode: Option<MarkSourceMode>,
    /// New thin-book spread cap in bps for the median guard
    /// (BE-31 Phase A). `None` = leave unchanged. `Some(0)` resets
    /// to the built-in default `DEFAULT_MAX_MARK_SPREAD_BPS` (100 bps).
    /// Ignored unless `mark_source_mode` is `Median`.
    #[serde(default)]
    pub max_mark_spread_bps: Option<u32>,
    /// BE-31 Phase B: max age (ms) for a composite-CEX price update
    /// before it's excluded from the median. `None` = leave unchanged.
    /// `Some(0)` resets to the built-in default
    /// `DEFAULT_CEX_COMPOSITE_STALENESS_MS` (30s). Ignored unless
    /// `mark_source_mode` is `Median` and the market has at least
    /// one composite update.
    #[serde(default)]
    pub cex_composite_staleness_ms: Option<u64>,
    /// BE-26: enable partial liquidation for this market. `None` =
    /// leave unchanged. See `MarketConfig::partial_liquidation_enabled`
    /// for semantics. Safe to flip on at any time.
    #[serde(default)]
    pub partial_liquidation_enabled: Option<bool>,
    /// Replace the rolling-volume fee-tier table. `None` = leave
    /// unchanged. `Some([])` clears the table and returns the market to
    /// flat `taker_fee_bps` / `maker_fee_bps` pricing. Non-empty tables
    /// must start at volume 0 and have strictly increasing thresholds.
    #[serde(default)]
    pub fee_tiers: Option<Vec<FeeTier>>,
    /// New initial margin ratio in basis points. `None` = leave unchanged.
    /// Operators use this to tighten live markets after a risk-policy cutover;
    /// it must be >= the current value and is enforced on the next IM/MM check.
    #[serde(default)]
    pub im_bps: Option<u32>,
    /// New maintenance margin ratio in basis points. `None` = leave unchanged.
    /// Must be >= the current value and satisfy `0 < mm_bps <= im_bps` after
    /// applying any paired IM update.
    #[serde(default)]
    pub mm_bps: Option<u32>,
    /// New aggregate open-interest cap in contracts. `None` = leave
    /// unchanged. Setting to 0 disables the cap.
    ///
    /// Appended at the tail to preserve positional wire compatibility.
    #[serde(default)]
    pub max_open_interest: Option<u64>,
}

/// Per-account fee override (BE-46). Stored at
/// `keys::account_fee_override(addr)` whenever an account has been
/// granted a non-default fee schedule. Replaces the market's base
/// `taker_fee_bps` / `maker_fee_bps` on fills where this account is
/// the taker / maker respectively.
///
/// Override semantics intentionally apply globally across all
/// markets — a single VIP-tier flag per account is the MVP shape;
/// per-market overrides can be added later as a follow-up if a
/// real use-case shows up.
///
/// Wire-format note: this struct is stored on-chain (not sent over
/// the wire as an Action), so field order is fixed by the storage
/// layout. Do not reorder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountFeeOverride {
    /// Taker fee rate in basis points (0..10_000). Replaces the
    /// market's `taker_fee_bps` on fills taken by this account.
    pub taker_fee_bps: u32,
    /// Maker fee rate in basis points (0..10_000). Replaces the
    /// market's `maker_fee_bps` on fills made by this account.
    pub maker_fee_bps: u32,
}

/// Set (or overwrite) an account's per-account fee override.
/// Relayer-signed admin action — `signer` must be on the relayer
/// allowlist or the engine returns `UnauthorizedRelayer`.
///
/// Both `taker_fee_bps` and `maker_fee_bps` must be in
/// `[0, 10_000]`, except for `FEE_OVERRIDE_REVERT_SENTINEL`
/// (`u32::MAX`), which reverts that side to the market's base fee at
/// fill time. Other out-of-range values are rejected with
/// `FeeBpsOutOfRange`.
///
/// To "clear" an existing override, set both fee fields to
/// `FEE_OVERRIDE_REVERT_SENTINEL`; partial reverts set only the side
/// that should fall back to the market base.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetAccountFeeOverride {
    /// Account to override fees for. 20-byte address.
    #[serde(with = "crate::wire_bytes")]
    pub account: [u8; 20],
    /// New taker fee in basis points (0..10_000), or
    /// `FEE_OVERRIDE_REVERT_SENTINEL` to revert taker fills to market base.
    pub taker_fee_bps: u32,
    /// New maker fee in basis points (0..10_000), or
    /// `FEE_OVERRIDE_REVERT_SENTINEL` to revert maker fills to market base.
    pub maker_fee_bps: u32,
    /// Authorized relayer signer. Must equal the envelope's derived
    /// owner and be on the relayer allowlist.
    #[serde(with = "crate::wire_bytes")]
    pub signer: [u8; 20],
    /// Replay-guard sequence (BE-46.2). The engine tracks the highest
    /// accepted `seq` per `account`; the next call must satisfy
    /// `cmd.seq > stored_seq` or it is rejected with
    /// `FeeOverrideStaleSeq`. The first call against a fresh account
    /// (stored seq = 0) accepts any `seq >= 1`. The seq advances on
    /// the no-op path too (identical override) so stale replays stay
    /// rejected even when the value didn't change.
    ///
    /// Appended at the end of the struct so absent-on-the-wire decodes
    /// as `0` (rmp-serde default for `u64`); the handler then rejects
    /// `seq == 0` against any stored seq, surfacing legacy callers
    /// loudly rather than silently accepting them. This breaks the
    /// unreleased-branch wire format intentionally — landing the
    /// guard before mainnet is the whole point.
    pub seq: u64,
}

/// Status of a pending withdrawal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalStatus {
    Pending,
    Completed,
    Failed,
}

/// On-chain record of a withdrawal request. Byte-frozen: the pre-receipt
/// binary decodes this exact 6-field rmp array on rollback, so receipt-phase
/// fields live in [`WithdrawalReceiptSidecar`], never here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawalRecord {
    pub id: u64,
    #[serde(with = "crate::wire_bytes")]
    pub owner: [u8; 20],
    pub amount: u64,
    #[serde(with = "crate::wire_bytes")]
    pub solana_destination: [u8; 32],
    pub status: WithdrawalStatus,
    pub request_height: u64,
}

/// W28-20 receipt-phase sidecar to a [`WithdrawalRecord`], stored under its
/// own key (`keys::withdrawal_receipt_sidecar`). Absent decodes as default:
/// unauthorized, zero fee.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawalReceiptSidecar {
    /// `SHA256(Borsh(WithdrawalAuthorizationV1))` of the operator-quorum
    /// authorization, recorded via `AuthorizeWithdrawal`. `None` until
    /// authorized; a receipt-gated terminal transition requires it to equal
    /// the receipt's `authorization_digest`.
    pub authorized_digest: Option<[u8; 32]>,
    /// The registry epoch this withdrawal was authorized under. Terminal
    /// verification resolves the registry for THIS epoch, not only the
    /// current one, so a rotation does not strand an already-authorized
    /// withdrawal. Set iff `authorized_digest` is set.
    pub authorization_epoch: Option<u64>,
    /// Flat protocol fee in micro-USDC, frozen from the withdrawal policy at
    /// request time. Debited together with `amount` up front; retained as
    /// protocol equity on settle, refunded together with `amount` on cancel.
    pub fee: u64,
}

/// Protocol withdrawal policy: the flat fee and the minimum net withdrawal.
/// Singleton at `keys::WITHDRAWAL_POLICY`; absent decodes as all-zero (no fee,
/// no minimum), so the legacy path is unaffected. Adjustable only through the
/// same quorum + timelock as the launch limits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawalPolicy {
    /// Flat protocol fee in micro-USDC, committed in the signed
    /// `WithdrawalAuthorizationV1`. Applied as an integer floor.
    #[serde(default)]
    pub fee_micro_usdc: u64,
    /// Minimum net `amount` a request may withdraw, in micro-USDC. Held
    /// strictly above `fee_micro_usdc`; the request gate floors the effective
    /// minimum at the fee, so a misconfiguration cannot open a sub-fee dust
    /// hole.
    #[serde(default)]
    pub min_withdrawal_micro_usdc: u64,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Why an order was cancelled. Serialised as a string in ABCI events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CancelReason {
    UserRequested,
    Expired,
    AdminForce,
    Liquidation,
}

impl fmt::Display for CancelReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancelReason::UserRequested => f.write_str("user_requested"),
            CancelReason::Expired => f.write_str("expired"),
            CancelReason::AdminForce => f.write_str("admin_force"),
            CancelReason::Liquidation => f.write_str("liquidation"),
        }
    }
}

/// Engine output events, emitted during transaction execution and end-of-block processing.
/// Encoded as CometBFT ABCI events for indexing and WebSocket streaming.
#[derive(Clone, Debug, Serialize, Deserialize, AbciEvent)]
pub enum Event {
    OrderPlaced {
        order_id: OrderId,
        market: MarketId,
        owner: [u8; 20],
        side: Side,
        price: u64,
        quantity: u64,
        /// Client-assigned order id. `0` means absent.
        client_order_id: u64,
        /// Time-in-force the order rested with (always `gtc` today — IOC/FOK
        /// never rest and so never emit `OrderPlaced`). Emitted so history
        /// can record it without joining to the placement tx.
        time_in_force: TimeInForce,
        post_only: bool,
        reduce_only: bool,
    },
    OrderCancelled {
        order_id: OrderId,
        market: MarketId,
        owner: [u8; 20],
        reason: CancelReason,
        /// Client-assigned order id. `0` means absent.
        client_order_id: u64,
        /// Quantity originally accepted onto the book.
        original_quantity: u64,
        /// Quantity still resting when the cancel occurred.
        remaining_quantity: u64,
        /// Cumulative maker quantity filled before cancellation.
        filled_quantity: u64,
    },
    OrderAmended {
        order_id: OrderId,
        market: MarketId,
        owner: [u8; 20],
        side: Side,
        old_price: u64,
        new_price: u64,
        old_quantity: u64,
        new_quantity: u64,
        remaining_quantity: u64,
        filled_quantity: u64,
        queue_priority_reset: bool,
        /// Client-assigned order id. `0` means absent.
        client_order_id: u64,
    },
    AtomicBasketExecuted {
        owner: [u8; 20],
        leg_count: u32,
        max_slippage_bps: u32,
    },
    PriceUpdated {
        market: MarketId,
        price: u64,
        signer: Option<[u8; 20]>,
    },
    TradeExecuted {
        fill_id: FillId,
        market: MarketId,
        /// Execution price in micro-USDC.
        price: u64,
        /// Filled quantity in base-asset units.
        quantity: u64,
        maker_order_id: OrderId,
        /// Maker's client-assigned order id. `0` means absent.
        maker_client_order_id: u64,
        maker_owner: [u8; 20],
        maker_side: Side,
        /// Engine-assigned id of the taker (aggressor) order. Set even for
        /// market and fully-crossing orders that never rest, so a fill
        /// correlates to both sides' orders.
        taker_order_id: OrderId,
        taker_owner: [u8; 20],
        /// Taker's client-assigned order id. `0` means absent.
        taker_client_order_id: u64,
        /// Taker fee in micro-USDC (positive = charged, negative = rebate).
        taker_fee: i64,
        /// Maker fee in micro-USDC (positive = charged, negative = rebate).
        maker_fee: i64,
    },
    FeesCollected {
        market: MarketId,
        taker_owner: [u8; 20],
        taker_fee: i64,
        maker_owner: [u8; 20],
        maker_fee: i64,
    },
    Deposited {
        owner: [u8; 20],
        amount: u64,
        new_balance: u64,
        signer: Option<[u8; 20]>,
    },
    Withdrawn {
        owner: [u8; 20],
        amount: u64,
        new_balance: u64,
        signer: Option<[u8; 20]>,
    },
    PositionUpdated {
        owner: [u8; 20],
        market: MarketId,
        side: Side,
        entry_price: u64,
        size: u64,
    },
    PositionClosed {
        owner: [u8; 20],
        market: MarketId,
        realized_pnl: i64,
    },
    /// Per-trade MTM event for an impact-family ConditionalPerp close
    /// (no-oracle MTM redesign, 2026-04-26). Issues `signed_delta` units of
    /// the corresponding prediction binary (B+ for CPY closes, B- for CPN
    /// closes) at entry $0 to the closing party, decomposing their
    /// conditional PnL into a fungible binary token.
    ///
    /// `signed_delta` is in conditional dollars (the size of the issued
    /// binary). Positive = long binary issued (closing party gained on the
    /// CP close); negative = short binary issued (loss).
    ///
    /// `cash_pnl_realized` is the cash-settlement byproduct of the v1
    /// limitation where MTM that creates an opposite-side delta against an
    /// existing direct-traded binary position nets out, realizing PnL on
    /// the absorbed portion at the MTM-issued $0 entry. Zero in the common
    /// case (no existing opposite-side binary). Tracked separately for
    /// observability so off-chain monitoring can quantify the cash
    /// imbalance v2 multi-position support will eliminate.
    ///
    /// `final_size` and `final_entry` are the resulting state of the
    /// owner's binary position after the MTM is applied (could be 0 if
    /// the MTM netted out an equal-size opposite position).
    BinaryIssuedFromMTM {
        owner: [u8; 20],
        /// CP market that triggered the MTM (closed via fill).
        cp_market: MarketId,
        /// Target binary market (B+ for CPY closes, B- for CPN closes).
        binary_market: MarketId,
        /// Signed quantity of binary issued. + = long, - = short.
        signed_delta: i64,
        /// Resulting binary position size after MTM applied.
        final_size: u64,
        /// Resulting binary position entry after MTM applied (weighted
        /// average for same-side blends, 0 for fresh issuance).
        final_entry: u64,
        /// Cash debit/credit from netting against an existing opposite-side
        /// binary position. Zero in the common case.
        cash_pnl_realized: i64,
    },
    MarketCreated {
        market: MarketId,
        im_bps: u32,
        mm_bps: u32,
        taker_fee_bps: u32,
        maker_fee_bps: u32,
        funding_interval_ms: u64,
        max_funding_rate_bps: u32,
        /// Address that signed the action. `None` when the market was
        /// created by governance execution, which has no single signer —
        /// the approving roster is carried by the surrounding proposal
        /// events.
        signer: Option<[u8; 20]>,
    },
    /// Admin updated one or more tunable fields on an existing market.
    /// Event carries the FULL post-update set of mutable fields so
    /// consumers see the live config after the tx; no need to diff
    /// against prior state. `kind` remains immutable and is intentionally
    /// omitted because UpdateMarketFees cannot change the accounting model.
    ///
    /// ABCI event derivation requires Display on every attribute, so
    /// Option<T> would break the codegen — we emit the whole config
    /// snapshot instead. Callers that only care about the delta can
    /// compare to the previous MarketConfigUpdated for the same market.
    MarketConfigUpdated {
        market: MarketId,
        im_bps: u32,
        mm_bps: u32,
        taker_fee_bps: u32,
        maker_fee_bps: u32,
        max_funding_rate_bps: u32,
        funding_interval_ms: u64,
        max_position_size: u64,
        default_ttl_ms: u64,
        net_delta_margin: bool,
        // BE-48 + BE-50 fields included in the event payload so off-chain
        // consumers can mirror the live config without re-reading state.
        // `primary_oracle_signer` is flattened to `[u8; 20]`: all-zero
        // bytes mean "no primary signer set". The derive now carries
        // `Option<[u8; N]>` (absence writes an empty attribute), so this
        // field could follow `signer` off the sentinel — a wire change for
        // an existing attribute, so not folded into this one.
        tick_size: u64,
        lot_size: u64,
        primary_oracle_signer: [u8; 20],
        oracle_staleness_ms: u64,
        max_open_interest: u64,
        signer: Option<[u8; 20]>,
    },
    /// An account's fee override was set (BE-46). Emitted on success
    /// of `SetAccountFeeOverride`. Carries the post-update values so
    /// off-chain consumers can mirror the override without re-reading
    /// state.
    AccountFeeOverrideSet {
        account: [u8; 20],
        taker_fee_bps: u32,
        maker_fee_bps: u32,
        signer: Option<[u8; 20]>,
    },
    /// Impact-market family was registered. Emitted once; the 5 underlying
    /// `MarketCreated` events follow (1 reused existing perp + 4 new children).
    ImpactMarketCreated {
        impact_market_id: ImpactMarketId,
        underlying_market: MarketId,
        cpy_market: MarketId,
        cpn_market: MarketId,
        eby_market: MarketId,
        ebn_market: MarketId,
        question: String,
        deadline_ms: u64,
        resolution_window_ms: u64,
        description: String,
        rules: String,
    },
    /// A standalone event was created: two prediction-binary books under a
    /// new [`EventInfo`], no underlying perp (G17).
    EventCreated {
        event_id: EventId,
        eby_market: MarketId,
        ebn_market: MarketId,
        question: String,
        settlement_ms: u64,
        resolution_window_ms: u64,
        description: String,
        rules: String,
    },
    /// Impact-market family crossed its deadline and is frozen while
    /// awaiting resolver signatures. New orders on the child books are
    /// rejected; existing resting child orders are cancelled by the sweep.
    ImpactMarketPreResolution {
        impact_market_id: ImpactMarketId,
        timestamp_ms: u64,
    },
    /// Event was resolved with a definitive outcome. Emitted once per family.
    ImpactMarketResolved {
        impact_market_id: ImpactMarketId,
        outcome: Outcome,
        /// Oracle price of the underlying at resolution time (micro-USDC).
        /// Used as the settlement mark for the winning conditional perp.
        settlement_price: u64,
        timestamp_ms: u64,
        signer: Option<[u8; 20]>,
    },
    /// A standalone event resolved to Yes or No. No settlement price (no
    /// underlying) and no Void.
    EventResolved {
        event_id: EventId,
        outcome: Outcome,
        timestamp_ms: u64,
        signer: Option<[u8; 20]>,
    },
    /// A conditional-perp position was cash-settled to an owner's balance
    /// because its branch won the resolution.
    ConditionalSettled {
        impact_market_id: ImpactMarketId,
        market: MarketId,
        owner: [u8; 20],
        side: Side,
        size: u64,
        entry_price: u64,
        settlement_price: u64,
        realized_pnl: i64,
    },
    /// A conditional-perp position was voided because its branch lost. The
    /// position holder's reserved IM is released (effectively: position deleted,
    /// no balance change, as IM is equity-based not locked collateral).
    ConditionalVoided {
        impact_market_id: ImpactMarketId,
        market: MarketId,
        owner: [u8; 20],
        side: Side,
        size: u64,
    },
    /// A prediction-binary position was cash-settled. Longs receive
    /// `payoff_per_share * size` credited to their balance; shorts have
    /// `(1.0 - payoff_per_share) * size` debited. Payoff is $1 for the winner
    /// and $0 for the loser.
    PredictionSettled {
        impact_market_id: ImpactMarketId,
        market: MarketId,
        owner: [u8; 20],
        side: Side,
        size: u64,
        /// Payoff per share in micro-USDC ($1.00 = 1_000_000, $0.00 = 0).
        payoff_per_share: u64,
        /// Signed cash delta applied to owner balance (micro-USDC).
        cash_delta: i64,
    },
    /// Position forcibly closed by the end-of-block liquidation sweep.
    AccountLiquidated {
        owner: [u8; 20],
        market: MarketId,
        side: Side,
        size: u64,
        /// Oracle mark price at which the position was liquidated (micro-USDC).
        mark_price: u64,
        /// Realized PnL in micro-USDC (negative means a loss).
        realized_pnl: i64,
    },
    /// Insurance fund balance changed (e.g., from liquidation surplus/deficit).
    /// Per-pool variant — the legacy event without `pool_id` continues to
    /// be emitted for pool 0 to preserve consumer compatibility.
    InsuranceFundUpdated {
        /// Pool the change applied to. 0 for legacy/majors pool (the
        /// bare `InsuranceFund` state key); 1+ for newer pools keyed
        /// under `InsuranceFundByPool`. Added 2026-04-25 with the
        /// four-tier waterfall.
        #[serde(default)]
        pool_id: u8,
        /// New total balance in micro-USDC (can be negative if fund is depleted).
        balance: i64,
        /// Change amount in micro-USDC (positive = inflow, negative = outflow).
        delta: i64,
    },
    /// Tier 0 of the bad-debt waterfall. HLP absorbed `amount` of
    /// liquidation deficit, leaving HLP balance at `hlp_balance_after`.
    /// Emitted only when HLP is enabled AND its balance was above the
    /// floor at draw time. Once HLP hits the floor, further deficits
    /// route to Tier 1 (per-pool IF) and this event stops firing.
    HlpAbsorbed {
        /// Pool the liquidation came from (informational — HLP is
        /// pool-agnostic at Tier 0).
        pool_id: u8,
        /// Microusdc absorbed by HLP this draw.
        amount: u64,
        /// HLP balance after the draw.
        hlp_balance_after: i64,
    },
    /// Tier 2 of the bad-debt waterfall. The pool's IF was insufficient
    /// to fully absorb a liquidation deficit, so the residual was
    /// distributed pro-rata across eligible open positions in the pool. At
    /// schema v9 and later, eligibility is Perp-only and all Tier-2 draws for
    /// the same pool and block share one `socialized_cap_bps × pool_notional /
    /// 10_000` budget; legacy schemas retain the historical per-event cap.
    /// Each affected account is debited proportionally.
    SocializedLossApplied {
        pool_id: u8,
        /// Microusdc actually debited from counterparties by this Tier-2
        /// event. Each debit is floored by the counterparty's balance.
        total_amount: u64,
        /// Microusdc the cap-respecting model says we *should* have
        /// absorbed: `min(requested, socialized_cap_bps × pool_notional /
        /// 10_000)`. A `total_amount < cap_target` gap reflects balance or
        /// integer pro-rata floors and rolls through to Tier 3 (ADL).
        /// Off-chain consumers can compare these fields to observe it.
        cap_target: u64,
        /// Cap in bps applied to the pool's two-sided notional. Echoed
        /// here for auditability; matches the
        /// `InsuranceFundConfig.socialized_cap_bps` of the time.
        cap_bps: u32,
        /// Number of open positions debited inline.
        affected_count: u32,
    },
    /// Tier 3 of the bad-debt waterfall. A profitable counterparty was
    /// auto-deleveraged (force-closed at the bankruptcy price of the
    /// counterparty being liquidated) because all prior tiers were
    /// exhausted. The ADL queue ranks positions by
    /// `unrealized_pnl × leverage_used` descending; this event is
    /// emitted once per ADL'd position.
    PositionAutoDeleveraged {
        /// Owner whose position was force-closed.
        owner: [u8; 20],
        market: MarketId,
        side: Side,
        /// Number of contracts force-closed in this leg. May be less
        /// than the ADL'd account's full position when the deficit is
        /// covered by a partial close (per audit 2026-04-25 P0 #2 fix);
        /// the remainder of the position keeps its original entry.
        size: u64,
        /// Price the position was actually closed at — the **liquidated
        /// trader's bankruptcy price** `bp = entry − σ × balance / size`
        /// (one `bp` per liquidation event, threaded through to every
        /// ADL leg in the same waterfall call). Equals `close_price_spec`
        /// after the audit P0 #2 fix landed; the two fields are retained
        /// separately for forward compatibility with future settlement-
        /// price experimentation.
        close_price: u64,
        /// Bankruptcy price the spec (`docs/adl-vs-socialized-loss.md`
        /// §3.5) says the position should be closed at. Currently equal
        /// to `close_price`; tracked separately so any future deviation
        /// (e.g., adding a "max haircut per ADL leg" cap that would
        /// re-introduce a spec gap) can be audited via this field.
        close_price_spec: u64,
        /// Realized PnL credited to the ADL'd counterparty at
        /// `close_price` for the closed `size`. Includes any rounding
        /// surplus credited back when ceil-div over-extracted (so the
        /// counterparty's net surrender equals exactly the residual
        /// covered by this leg, not residual + rounding overshoot).
        /// May be negative if `bp` would imply a realized loss for the
        /// counterparty — note that for the alpha-deferred deviation,
        /// we cap credit at 0 rather than driving the counterparty into
        /// negative balance.
        realized_pnl: i64,
    },
    WithdrawRequested {
        withdrawal_id: u64,
        owner: [u8; 20],
        amount: u64,
        solana_destination: [u8; 32],
    },
    DepositConfirmed {
        owner: [u8; 20],
        amount: u64,
        new_balance: u64,
        solana_tx_sig: Vec<u8>,
        signer: Option<[u8; 20]>,
    },
    /// Relayer rejected a Solana deposit transfer. The user was NOT
    /// credited; the engine records a `(signature, locator)` dedup marker
    /// in state, so any subsequent `ConfirmDeposit`/`FailDeposit` for that
    /// same transfer is a silent no-op. The event itself carries only the
    /// signature — it does not name which transfer inside the transaction
    /// failed. `solana_signature` is the raw on-chain sig bytes (typically
    /// 64 bytes — same encoding as `DepositConfirmed.solana_tx_sig`).
    DepositFailed {
        solana_signature: Vec<u8>,
        reason: FailDepositReason,
        signer: Option<[u8; 20]>,
    },
    WithdrawalConfirmed {
        withdrawal_id: u64,
        solana_tx_sig: Vec<u8>,
        /// Address that signed the action. `None` when confirmed via an
        /// operator-quorum receipt, which has no single signer.
        signer: Option<[u8; 20]>,
    },
    WithdrawalFailed {
        withdrawal_id: u64,
        owner: [u8; 20],
        amount: u64,
        new_balance: u64,
        reason: String,
        /// Address that signed the action. `None` when failed via an
        /// operator-quorum receipt, which has no single signer.
        signer: Option<[u8; 20]>,
    },
    WithdrawalAuthorized {
        withdrawal_id: u64,
        authorization_digest: [u8; 32],
    },
    AgentApproved {
        owner: [u8; 20],
        agent: [u8; 20],
        agent_pubkey: [u8; 32],
    },
    AgentRevoked {
        owner: [u8; 20],
        agent: [u8; 20],
        agent_pubkey: [u8; 32],
    },
    /// A new funding rate was computed and applied to the market.
    FundingApplied {
        market: MarketId,
        /// Signed funding rate in basis points for this interval.
        funding_rate_bps: i64,
        /// New cumulative funding index after applying this rate.
        cumulative_funding: i64,
        timestamp_ms: u64,
    },
    /// Funding payment settled for a single position.
    FundingSettled {
        owner: [u8; 20],
        market: MarketId,
        /// Payment in micro-USDC (positive = received, negative = paid).
        payment: i64,
    },
    /// Account lacked sufficient balance to pay full funding obligation.
    FundingShortfall {
        owner: [u8; 20],
        market: MarketId,
        /// Full amount owed in micro-USDC.
        owed: i64,
        /// Amount actually collected in micro-USDC.
        actual: i64,
        /// Unfunded gap absorbed by the insurance fund (micro-USDC).
        shortfall: u64,
    },
    /// Emitted when a fill causes an account to drop below maintenance margin.
    /// The fill is NOT blocked — the end-of-block liquidation sweep will handle it.
    /// This event provides immediate observability for off-chain monitoring.
    MarginWarning {
        owner: [u8; 20],
        equity: i64,
        maintenance_margin: u64,
    },
    /// Emitted exactly once at the end of every market-order tx that passes
    /// envelope checks, regardless of how many fills happened.
    ///
    /// Market orders use IOC semantics — any unfilled remainder is silently
    /// dropped. Without this event, callers would have to count downstream
    /// `TradeExecuted` events to learn whether a market order actually moved
    /// any quantity, and could not distinguish "no counterparty" from "fully
    /// filled and the trade events arrived in a different stream view".
    ///
    /// Off-chain monitors and SDKs should treat this as the authoritative
    /// "did my market order do anything?" signal:
    ///   - `filled_quantity == requested_quantity` → fully filled
    ///   - `0 < filled_quantity < requested_quantity` → partial fill, rest dropped
    ///   - `filled_quantity == 0` → no counterparty (or all counterparties were
    ///     the taker themselves and got rejected by self-match prevention)
    MarketOrderProcessed {
        /// Engine-assigned id of this market order, referenceable in fills
        /// and order history.
        order_id: OrderId,
        market: MarketId,
        owner: [u8; 20],
        side: Side,
        requested_quantity: u64,
        filled_quantity: u64,
        /// Client-assigned order id. `0` means absent.
        client_order_id: u64,
    },
    /// User picked a per-market IM override (BE-16). `user_im_bps == 0`
    /// means the override was cleared (engine reverts to market
    /// default).
    UserMarketLeverageSet {
        owner: [u8; 20],
        market: MarketId,
        user_im_bps: u32,
    },
    /// Compatibility settlement for a position liability left by the retired
    /// schema-v9 socialized-loss index. No new Tier-2 event increments that
    /// index. `collected < owed` means the account's balance floored the debit.
    SocializedLossSettled {
        owner: [u8; 20],
        market: MarketId,
        /// Share owed for the index movement since last settle (µUSDC).
        owed: u64,
        /// Amount actually debited, floored at the account balance (µUSDC).
        collected: u64,
    },
    /// An admin proposal was accepted. Carries the engine-assigned id —
    /// the only channel that returns it to the proposer. Full
    /// `action_bytes` deliberately stay out of events; state plus the
    /// proposals query is the reconstruction source of record.
    ProposalCreated {
        proposal_id: u64,
        proposer: [u8; 20],
        registry_version: u64,
        threshold: u32,
        action_tag: u8,
        content_hash: [u8; 32],
        created_ms: u64,
        expiry_ms: u64,
    },
    /// An approval vote was recorded (including the proposer's implicit
    /// approval #1 — not evented separately; it rides `ProposalCreated`).
    ProposalApproved {
        proposal_id: u64,
        approver: [u8; 20],
        approvals_count: u32,
        threshold: u32,
    },
    /// Threshold reached and the inner action applied. Followed in the
    /// same transaction result by the inner action's own events.
    ProposalExecuted { proposal_id: u64 },
    /// Threshold reached but the inner apply failed deterministically.
    /// The child overlay was dropped; `error_code` is the full
    /// `ExecError::code()` value, unnarrowed.
    ProposalFailed { proposal_id: u64, error_code: u32 },
    /// A rejection was recorded. Terminal when `by_proposer` (cancel) or
    /// when `rejections_count` reached `members - threshold + 1`.
    ProposalRejected {
        proposal_id: u64,
        rejecter: [u8; 20],
        by_proposer: bool,
        rejections_count: u32,
    },
    /// A `Pending` proposal was flipped terminal by the lazy expiry
    /// sweep (`ttl`) or a registry rotation (`registry_changed`).
    ProposalExpired {
        proposal_id: u64,
        reason: ExpiryReason,
    },
    /// A single-signer emergency action executed. `market_id` is zero
    /// for market-less arms (`HaltTrading`).
    EmergencyActionExecuted {
        signer: [u8; 20],
        action_tag: u8,
        market_id: MarketId,
    },
    /// The signer roster was replaced (a rotation executed, or genesis /
    /// seed wrote version 1). `members` is the full replacement roster,
    /// concatenated 40-hex-char addresses in canonical order.
    AdminSignerRegistryUpdated {
        version: u64,
        threshold: u32,
        members: String,
    },
    /// The admin multisig reached quorum on a bridge un-halt. The engine
    /// holds no halt flag; this is the authoritative decision record the
    /// Squads operator quorum acts on to unfreeze the vault on Solana.
    BridgeUnpauseAuthorized { proposal_id: u64 },
    /// An operator-authority set was rotated by governance (#422). `domain`
    /// is the `AuthorityDomain` discriminant; `added`/`removed` are the
    /// affected addresses as concatenated 40-hex-char strings. The signer is
    /// carried by the accompanying `ProposalExecuted`/approval events.
    AuthoritySetUpdated {
        domain: u8,
        added: String,
        removed: String,
        proposal_id: u64,
    },
    /// The admin quorum wrote the per-market oracle guards. Carries
    /// the FULL post-update guard set so consumers need not diff; zero means
    /// the field is still unset.
    OracleGuardsUpdated {
        market: MarketId,
        mark_price_max_oracle_age_ms: u64,
        max_oracle_deviation_bps: u32,
        proposal_id: u64,
    },
    /// An authorized `OracleUpdate` was refused by the per-market deviation
    /// guard once the oracle-guard gate is active. The
    /// transaction itself is accepted (nonce burned, no state written): the
    /// submitted price is NOT stored, the publish time does NOT advance, and
    /// this event records why. `max_oracle_deviation_bps == 0` with reason
    /// `deviation_band_unset` is the fail-closed default before governance
    /// sets a band.
    OracleUpdateRejected {
        market: MarketId,
        submitted_price: u64,
        last_good: u64,
        max_oracle_deviation_bps: u32,
        signer: Option<[u8; 20]>,
        reason: OracleRejectReason,
    },
    /// A due funding interval on `market` was skipped because its mark was
    /// stale or unguarded while the oracle-guard gate is active (pause,
    /// no retroactive catch-up). The interval clock restarts at
    /// `timestamp_ms`, so the missed interval is lost, not replayed.
    FundingSkipped {
        market: MarketId,
        timestamp_ms: u64,
        reason: FundingSkipReason,
    },
    /// A multisig-approved trigger-market policy was stored for automatic
    /// application at `effective_height`. The complete replacement is evented
    /// so operators can audit the scheduled transition without interpreting
    /// proposal bytes. This event does not mean the policy is effective yet.
    TriggerMarketConfigScheduled {
        market: MarketId,
        version: u64,
        accepted_height: u64,
        effective_height: u64,
        enabled: bool,
        max_trigger_slippage_bps: u32,
        max_mark_age_ms: u64,
        max_future_publish_skew_ms: u64,
        max_active_brackets: u64,
    },
    /// A previously scheduled trigger-market policy became effective at this
    /// block's automatic pre-transaction configuration phase.
    TriggerMarketConfigActivated {
        market: MarketId,
        version: u64,
        effective_height: u64,
        enabled: bool,
        max_trigger_slippage_bps: u32,
        max_mark_age_ms: u64,
        max_future_publish_skew_ms: u64,
        max_active_brackets: u64,
    },
    /// A whole-position protection bracket was installed or atomically
    /// replaced. Zero client/limb ids mean the optional field was absent.
    PositionTriggersSet {
        owner: [u8; 20],
        market: MarketId,
        position_epoch: u64,
        group_id: u64,
        client_group_id: u64,
        stop_limb_id: u64,
        stop_client_trigger_id: u64,
        take_profit_limb_id: u64,
        take_profit_client_trigger_id: u64,
        accepted_height: u64,
        active_from_height: u64,
        replaced_group_id: u64,
    },
    /// A user explicitly removed a stored position-protection bracket.
    PositionTriggersCancelled {
        owner: [u8; 20],
        market: MarketId,
        position_epoch: u64,
        group_id: u64,
    },
    /// A position close or side flip invalidated both linked limbs.
    PositionTriggersInvalidated {
        owner: [u8; 20],
        market: MarketId,
        position_epoch: u64,
        group_id: u64,
    },
    /// A crossed limb passed authoritative snapshot/account validation and
    /// began its one bounded reduce-only IOC-limit attempt.
    PositionTriggerActivated {
        owner: [u8; 20],
        market: MarketId,
        position_epoch: u64,
        group_id: u64,
        limb_id: u64,
        /// Zero means no client id was supplied.
        client_group_id: u64,
        /// Zero means no client id was supplied.
        client_trigger_id: u64,
        limb_kind: String,
        trigger_price: u64,
        frozen_mark: u64,
        limit_price: u64,
        requested_quantity: u64,
        execution_order_id: u64,
    },
    /// Terminal result of the activated limb's single bounded attempt.
    PositionTriggerExecuted {
        owner: [u8; 20],
        market: MarketId,
        position_epoch: u64,
        group_id: u64,
        limb_id: u64,
        client_group_id: u64,
        client_trigger_id: u64,
        limb_kind: String,
        trigger_price: u64,
        frozen_mark: u64,
        limit_price: u64,
        requested_quantity: u64,
        filled_quantity: u64,
        residual_quantity: u64,
        execution_order_id: u64,
        /// Aggregate signed fee charged by the attempt's committed fills.
        total_fee: i64,
        result: String,
        /// Empty when the terminal result has no additional reason.
        reason: String,
    },
    /// Account-level evaluation deferred this crossed limb. Repeated equal
    /// reasons are suppressed by the lifecycle repository.
    PositionTriggerDeferred {
        owner: [u8; 20],
        market: MarketId,
        position_epoch: u64,
        group_id: u64,
        limb_id: u64,
        client_group_id: u64,
        client_trigger_id: u64,
        limb_kind: String,
        trigger_price: u64,
        frozen_mark: u64,
        requested_quantity: u64,
        reason: String,
    },
    /// Shared market health changed from available to deferred. This is a
    /// market transition, never one event per stored bracket.
    TriggerMarketDeferred { market: MarketId, reason: String },
    /// Shared market health recovered from the previously deferred reason.
    TriggerMarketResumed {
        market: MarketId,
        previous_reason: String,
    },
    SubAccountCreated {
        owner: [u8; 20],
        sub_account_id: u32,
        address: [u8; 20],
        name: [u8; 32],
    },
    SubAccountTransferCompleted {
        owner: [u8; 20],
        from: [u8; 20],
        to: [u8; 20],
        amount: u64,
    },
}

impl Event {
    /// Whether the active trigger contract requires canonical chain
    /// coordinates on this event. Trigger-generated fills reuse the ordinary
    /// `TradeExecuted` event, so every active-era trade also carries its
    /// execution coordinate; this lets consumers correlate fills without
    /// guessing provenance from envelope-group position. Configuration
    /// scheduling/activation is a governance audit surface and deliberately
    /// retains its existing shape.
    pub const fn requires_trigger_coordinates(&self) -> bool {
        matches!(
            self,
            Self::TradeExecuted { .. }
                | Self::PositionTriggersSet { .. }
                | Self::PositionTriggersCancelled { .. }
                | Self::PositionTriggersInvalidated { .. }
                | Self::PositionTriggerActivated { .. }
                | Self::PositionTriggerExecuted { .. }
                | Self::PositionTriggerDeferred { .. }
                | Self::TriggerMarketDeferred { .. }
                | Self::TriggerMarketResumed { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Transaction execution error. Each variant maps to a stable non-zero ABCI
/// result code via [`ExecError::code()`] (see the `impl` block below). Codes
/// are unique except for the deliberate code-21 nonce-error family. The
/// historical code-50 collision between [`ExecError::SlippageExceeded`] and
/// [`ExecError::OpenInterestLimitExceeded`] was removed before live OI-cap
/// activation: basket slippage keeps its original code 50 and OI-cap rejection
/// uses code 51.
#[derive(Debug)]
pub enum ExecError {
    DecodeError(String),
    OrderNotFound(OrderId),
    NotOwner(OrderId),
    UnauthorizedOracle,
    Overflow,
    InvalidPrice,
    InvalidQuantity,
    InvalidSide,
    UnknownMarket(MarketId),
    InsufficientBalance,
    /// Post-trade equity would fall below initial margin requirement.
    InsufficientMargin,
    /// Store read/write returned unexpected data; indicates a bug or data corruption.
    StateCorruption(String),
    /// An admin action was submitted before the signer registry was initialized.
    AdminGovernanceInactive,
    /// The declared admin signer is unauthorized or differs from the envelope signer.
    NotAdminSigner,
    /// No proposal record exists under the given id.
    ProposalNotFound(ProposalId),
    /// The proposal exists but is already terminal.
    ProposalNotPending {
        id: ProposalId,
        status: ProposalStatus,
    },
    /// The proposal's TTL has passed (stored `Expired`, or `Pending`
    /// past `expiry_ms` — the lazy-expiry equivalent).
    ProposalExpired(ProposalId),
    /// The signer already approved this proposal (includes the
    /// proposer's implicit approval #1).
    DuplicateApproval,
    /// The signer already rejected this proposal.
    DuplicateRejection,
    /// The submitted or stored registry version differs from the
    /// current registry version.
    ProposalRegistryVersionMismatch {
        expected: RegistryVersion,
        got: RegistryVersion,
    },
    /// The approval's full immutable context does not byte-match the
    /// stored proposal. Deliberately field-free: no oracle for which
    /// byte differed.
    ProposalContentMismatch,
    /// The inner admin or emergency action is structurally invalid or
    /// its arm is not admitted (unknown/disallowed arm, non-zero inner
    /// signer, bounds).
    InvalidAdminAction(String),
    /// Canonical inner action bytes exceed `MAX_ADMIN_ACTION_BYTES`.
    AdminActionTooLarge {
        max: u32,
        got: u32,
    },
    /// `MAX_PENDING_PROPOSALS` simultaneously pending proposals already
    /// exist.
    TooManyPendingProposals,
    /// A governance id counter is exhausted; ids never wrap.
    ProposalIdExhausted,
    /// The signer's opposite vote already exists on this proposal;
    /// votes are immutable and disjoint.
    ConflictingVote,
    /// A caller-supplied market id exceeds the `i32::MAX` identity
    /// bound every signed-32-bit downstream consumer relies on.
    MarketIdOutOfRange {
        max: u32,
        got: u64,
    },
    /// The signer exceeded the per-signer emergency rate bound within
    /// the rolling window.
    EmergencyRateLimited {
        max: u32,
        window_ms: u64,
    },
    /// The chain-wide emergency rate bound was reached within the
    /// rolling window.
    EmergencyGlobalRateLimited {
        max: u32,
        window_ms: u64,
    },
    /// The emergency arm was retired: historical executions replay
    /// unchanged, current-height submissions fail closed.
    EmergencyActionRetired,
    /// A registry-gated admin action was submitted directly while the
    /// signer registry exists: for those actions the legacy path is
    /// closed and the proposal path is the only way — even for an
    /// authorized relayer. Raised by `CreateMarket` whenever a registry
    /// exists, and by `CreateImpactMarket` only once its lineage's
    /// admin-actions-v2 height has also passed. Other relayer-gated
    /// actions do not consult the registry and never raise this.
    AdminActionRequiresProposal,
    /// A proposed or applied signer roster violates the registry
    /// invariants (threshold bounds, sorted duplicate-free members,
    /// roster size, version headroom). Fails before any mutation.
    InvalidAdminRegistry(String),
    /// W28-20: a receipt-gated withdrawal action was submitted while no
    /// operator receipt registry exists on this chain — the operator
    /// custody phase is inactive and the path fails closed.
    BridgeReceiptRegistryInactive,
    /// W28-20: the operator quorum proof failed `bridge-core` verification
    /// (bad structure, below *m* distinct members, or an invalid
    /// signature). Wraps the `bridge_core::VerifyError` cause as text.
    BridgeReceiptInvalid(String),
    /// W28-20: the signed receipt does not bind to this withdrawal or
    /// deployment (id/owner/amount/destination/epoch/terminal-state
    /// mismatch). Names the field that diverged.
    BridgeReceiptMismatch(String),
    /// The net withdrawal `amount` is below the effective minimum (the
    /// configured `min_withdrawal`, floored at the flat fee), so the payout
    /// would be worth less than it costs to settle. Dust-griefing gate.
    WithdrawalBelowMinimum {
        amount: u64,
        min: u64,
    },
    /// W28-20 (DEC-48): a retired legacy relayer terminal
    /// (`ConfirmWithdrawal` / `FailWithdrawal`) was submitted at or above the
    /// receipt cutover. Rejected as a normal failed action (bytes absorbed,
    /// nonce burned). The reverse case — an operator-receipt terminal below
    /// the cutover — is NOT this error: it is skipped byte-identically to the
    /// pre-upgrade binary's decode failure (`DecodeError`, code 1), because
    /// any observable difference from that binary would fork a mixed fleet.
    WithdrawalTerminalGated(String),
    /// Catch-all for unexpected runtime failures (code 255).
    InternalError(String),
    UnauthorizedRelayer,
    WithdrawalNotFound(u64),
    WithdrawalAlreadyProcessed(u64),
    /// Solana tx signature has already been used (idempotency guard).
    DuplicateDeposit,
    InvalidSignature,
    SignatureRequired,
    /// Tx signer is neither the account owner nor an approved agent.
    AgentNotAuthorized,
    /// Agent wallets may trade but are forbidden from withdrawing funds.
    AgentCannotWithdraw,
    NonceTooOld {
        min_accepted: u64,
        got: u64,
    },
    NonceTooFarFuture {
        max_accepted: u64,
        got: u64,
    },
    NonceReplay {
        nonce: u64,
    },
    NonceBelowOldest {
        oldest: u64,
        got: u64,
    },
    MarketAlreadyExists(MarketId),
    InvalidMarketConfig(String),
    ImpactMarketAlreadyExists(ImpactMarketId),
    ImpactMarketNotFound(ImpactMarketId),
    /// Attempted to place an order on a conditional/binary book whose parent
    /// impact market is already resolved or voided.
    MarketClosedForTrading(MarketId),
    /// Binary-book order outside the [0, BINARY_PRICE_MAX] range.
    BinaryPriceOutOfRange,
    /// ResolveImpactMarket called with an invalid outcome for the current state.
    InvalidResolution(String),
    /// A fill would push the taker's absolute net position past
    /// `MarketConfig.max_position_size`. Engine-level cap enforced at
    /// placement time so a single whale can't accumulate unbounded
    /// exposure past protocol limits, regardless of margin.
    PositionLimitExceeded {
        market: MarketId,
        limit: u64,
        would_be: u64,
    },
    /// Fill would violate the configured market open-interest cap. At schema
    /// v9+, an already-over-cap market may preserve or reduce its pre-fill OI,
    /// but may not increase it.
    OpenInterestLimitExceeded {
        market: MarketId,
        limit: u64,
        would_be: u64,
    },
    /// Rejected `OracleUpdate` whose `publish_time_ms` is not strictly
    /// greater than the last accepted update for this market. Replay
    /// protection per audit B3 (2026-04-23).
    OracleTimestampNotMonotonic {
        market: MarketId,
        stored: u64,
        submitted: u64,
    },
    /// Account would touch more impact markets than the scenario margin
    /// engine can enumerate (`MAX_IMPACT_MARKETS_PER_ACCOUNT`). Returned
    /// instead of `InsufficientMargin` so clients can distinguish
    /// "basket exceeds enumeration cap" from "collateral shortfall."
    /// Audit 2026-04-25 P3.
    TooManyActiveImpactMarkets {
        current: u32,
        max: u32,
    },
    /// Net-delta margin grouping found legs of the same group with
    /// disagreeing settle prices — upstream data corruption (different
    /// markets in the same `underlying_market_id` group should resolve
    /// to identical settles per scenario). Distinct from `Overflow`,
    /// which the same path used to return as a placeholder. Audit
    /// 2026-04-25 P2 #10.
    SettlementPriceMismatch {
        market: MarketId,
        expected: u64,
        got: u64,
    },
    /// Rejected `OracleUpdate` for an impact-family market (CPY/CPN/EBY/EBN).
    /// Per the no-oracle MTM redesign (2026-04-26), these markets mark
    /// off the book directly and have no oracle layer. The only oracle
    /// reading happens at resolution, against the underlying perp.
    /// Returned defensively so a misbehaving feeder or replayed update
    /// can't corrupt the impact market's state.
    ///
    /// Variant defined in Phase A but not yet wired in `handle_oracle_update`
    /// (see comment there): the engine-side reject lands in Phase D
    /// alongside the `get_mark_price` book-mid fallback and the rewrite
    /// of impact-market test setup helpers, so the rollout is atomic.
    OracleNotApplicable {
        market: MarketId,
    },
    /// `PlaceOrder` with `post_only=true` would have crossed the book on
    /// placement. Rejected without taking, so makers can guarantee
    /// maker-side fills.
    PostOnlyWouldCross,
    /// `PlaceOrder` or `MarketOrder` with `reduce_only=true` was same-side
    /// as the existing position (would increase exposure) or no position
    /// existed at all. Reduce-only orders are required to actually reduce
    /// or close a position.
    ReduceOnlyWouldIncrease,
    /// A test/admin action (`RunLiquidationSweep`, `RunFundingTick`,
    /// `ForceLiquidate`) was rejected because the engine isn't configured
    /// to accept them in this deployment, or the position the action
    /// referenced doesn't exist.
    TestActionRejected(String),
    /// `SetAccountFeeOverride` rejected because a fee value is outside
    /// the legal `[0, 10_000]` basis-point range. BE-46.
    FeeBpsOutOfRange {
        bps: u32,
    },
    /// `SetAccountFeeOverride` rejected because `cmd.seq` is not
    /// strictly greater than the seq stored on this account — i.e. it
    /// is a replay or an out-of-order tx. BE-46.2 replay guard
    /// (Ramon's 2026-05-03 review on #39). The seq advances on the
    /// no-op path too, so even an identical-payload replay against a
    /// stale seq is rejected here.
    FeeOverrideStaleSeq {
        cmd_seq: u64,
        stored_seq: u64,
    },
    /// `PlaceOrder` price is not an exact multiple of the market's
    /// `tick_size`. BE-48: makes the orderbook coarser at high precision
    /// to keep MMs from quoting through fractional ticks.
    TickSizeViolation {
        market: MarketId,
        tick_size: u64,
        price: u64,
    },
    /// `PlaceOrder` quantity is not an exact multiple of the market's
    /// `lot_size`. BE-48 sibling of `TickSizeViolation`.
    LotSizeViolation {
        market: MarketId,
        lot_size: u64,
        quantity: u64,
    },
    /// `OracleUpdate` from a fallback (non-primary) signer was rejected
    /// because the market's last oracle update — by any signer — is still
    /// within the staleness window. Caller must wait until
    /// `block_time - last_publish_ms >= oracle_staleness_ms`. BE-50.
    OracleStaleNotElapsed {
        market: MarketId,
        last_publish_ms: u64,
        block_time_ms: u64,
        staleness_ms: u64,
    },
    /// `get_mark_price` rejected because the oracle for `market` is
    /// older than `MarketConfig::mark_price_max_oracle_age_ms`. Order
    /// placement, margin checks, and liquidation refuse to use a
    /// stale oracle so a node with a stuck feeder can't silently
    /// misprice the book. BE-33, 2026-05-03.
    StaleOracle {
        market: MarketId,
        /// Stored `publish_time_ms` of the most recent oracle update.
        publish_time_ms: u64,
        /// Block time at which the read was attempted.
        block_time_ms: u64,
        /// Configured staleness cap from `MarketConfig`.
        max_staleness_ms: u64,
    },
    /// A mark-dependent read on `market` was refused because the oracle-guard
    /// gate (`repo::UPGRADE_HEIGHT_ORACLE_GUARDS`) is active and the market's
    /// `mark_price_max_oracle_age_ms` is still the unset zero. Zero no longer
    /// means "disabled": the default fails closed until the admin quorum sets
    /// a real value through `AdminAction::SetOracleGuards`.
    OracleGuardUnset {
        market: MarketId,
    },
    /// `SetUserMarketLeverage` rejected because the user attempted to
    /// pick an IM ratio LOWER than the market's risk floor. The
    /// engine only allows users to deleverage (more margin, less
    /// leverage), never the other direction. BE-16, 2026-05-03.
    UserLeverageBelowMarketIm {
        market: MarketId,
        user_im_bps: u32,
        market_im_bps: u32,
    },
    /// Cancel-by-client-order-id could not find an active resting order for
    /// this owner/id pair. The order may never have rested, may have already
    /// filled, or may already have been cancelled.
    ClientOrderIdNotFound {
        client_order_id: u64,
    },
    /// A new resting order attempted to reuse an active owner/client_order_id
    /// pair. Client order IDs are how external MMs reconcile cancels, so the
    /// engine keeps the active namespace one-to-one instead of letting a later
    /// order shadow the earlier index entry.
    DuplicateClientOrderId {
        client_order_id: u64,
    },
    /// Client order ID zero is reserved as the "absent" sentinel in ABCI
    /// events. External clients must use a positive 64-bit value.
    InvalidClientOrderId {
        client_order_id: u64,
    },
    /// `PlaceOrder` with `time_in_force=Fok` could not fully fill against
    /// currently visible crossing liquidity at the submitted limit price.
    FillOrKillWouldNotFill {
        requested: u64,
        available: u64,
    },
    /// `CancelReplaceOrder` must identify exactly one active order, either by
    /// engine order id or by owner-scoped client order id.
    InvalidCancelReplaceTarget,
    /// `AmendOrder.new_quantity` is a total quantity below the order's already
    /// filled maker quantity. The engine rejects instead of creating a
    /// negative or zero resting remainder.
    AmendBelowFilled {
        order_id: OrderId,
        filled_quantity: u64,
        requested_quantity: u64,
    },
    /// An `AtomicBasketOrder` with a non-zero `max_slippage_bps` filled with
    /// aggregate adverse slippage exceeding that budget. Slippage is measured
    /// per leg (each fill price against that market's mark price) and the
    /// decision is taken on the notional-weighted aggregate across all legs —
    /// per-leg worst-case is separately bounded by each leg's FOK limit price.
    /// `max_slippage_bps == 0` disables the check (opt-in enforcement).
    SlippageExceeded {
        aggregate_bps: u32,
        max_slippage_bps: u32,
    },
    /// Registry lookup miss: no sub-account exists for the given master/id.
    SubAccountNotFound,
    /// Duplicate create: a sub-account with this master/id already exists.
    SubAccountAlreadyExists,
    /// Transfer from == to (no-op rejected).
    SubAccountTransferSameAccount,
    /// Neither side of a transfer is the master owner; both are derived children.
    SubAccountTransferBothChildren,
    /// Source balance is below the transfer amount.
    SubAccountTransferInsufficientBalance,
    /// Create rejected: `sub_account_id` is zero, which is not a valid id.
    SubAccountIdZero,
}

impl ExecError {
    pub fn code(&self) -> u32 {
        // Reserved by the SDK gateway submit path for transport-level
        // failures before the engine sees a transaction: 401, 413, 429, 500.
        // Keep consensus ExecError codes out of that range so callers can
        // distinguish engine rejects from gateway rejects.
        match self {
            ExecError::DecodeError(_) => 1,
            ExecError::OrderNotFound(_) => 2,
            ExecError::NotOwner(_) => 3,
            ExecError::UnauthorizedOracle => 4,
            ExecError::Overflow => 5,
            ExecError::InvalidPrice => 6,
            ExecError::InvalidQuantity => 7,
            ExecError::InvalidSide => 8,
            ExecError::UnknownMarket(_) => 9,
            ExecError::InsufficientBalance => 11,
            ExecError::InsufficientMargin => 12,
            ExecError::StateCorruption(_) => 10,
            ExecError::UnauthorizedRelayer => 13,
            ExecError::WithdrawalNotFound(_) => 14,
            ExecError::WithdrawalAlreadyProcessed(_) => 15,
            ExecError::DuplicateDeposit => 16,
            ExecError::InvalidSignature => 17,
            ExecError::SignatureRequired => 18,
            ExecError::AgentNotAuthorized => 19,
            ExecError::AgentCannotWithdraw => 20,
            ExecError::NonceTooOld { .. }
            | ExecError::NonceTooFarFuture { .. }
            | ExecError::NonceReplay { .. }
            | ExecError::NonceBelowOldest { .. } => 21,
            ExecError::MarketAlreadyExists(_) => 22,
            ExecError::InvalidMarketConfig(_) => 23,
            ExecError::ImpactMarketAlreadyExists(_) => 24,
            ExecError::ImpactMarketNotFound(_) => 25,
            ExecError::MarketClosedForTrading(_) => 26,
            ExecError::BinaryPriceOutOfRange => 27,
            ExecError::InvalidResolution(_) => 28,
            ExecError::PositionLimitExceeded { .. } => 29,
            // Code 50 was already assigned to SlippageExceeded when the OI-cap
            // variant was added. Preserve that older integration contract and
            // use the next contiguous code for the OI-cap rejection (#250).
            ExecError::OpenInterestLimitExceeded { .. } => 51,
            ExecError::OracleTimestampNotMonotonic { .. } => 30,
            ExecError::TooManyActiveImpactMarkets { .. } => 31,
            ExecError::SettlementPriceMismatch { .. } => 32,
            ExecError::OracleNotApplicable { .. } => 33,
            ExecError::PostOnlyWouldCross => 34,
            ExecError::ReduceOnlyWouldIncrease => 35,
            ExecError::TestActionRejected(_) => 36,
            ExecError::StaleOracle { .. } => 37,
            ExecError::UserLeverageBelowMarketIm { .. } => 38,
            ExecError::TickSizeViolation { .. } => 39,
            ExecError::LotSizeViolation { .. } => 40,
            ExecError::OracleStaleNotElapsed { .. } => 41,
            ExecError::FeeBpsOutOfRange { .. } => 42,
            ExecError::FeeOverrideStaleSeq { .. } => 43,
            ExecError::ClientOrderIdNotFound { .. } => 44,
            ExecError::DuplicateClientOrderId { .. } => 45,
            ExecError::InvalidClientOrderId { .. } => 46,
            ExecError::FillOrKillWouldNotFill { .. } => 47,
            ExecError::InvalidCancelReplaceTarget => 48,
            ExecError::AmendBelowFilled { .. } => 49,
            ExecError::SlippageExceeded { .. } => 50,
            ExecError::AdminGovernanceInactive => 52,
            ExecError::NotAdminSigner => 53,
            ExecError::ProposalNotFound(_) => 54,
            ExecError::ProposalNotPending { .. } => 55,
            ExecError::ProposalExpired(_) => 56,
            ExecError::DuplicateApproval => 57,
            ExecError::DuplicateRejection => 58,
            ExecError::ProposalRegistryVersionMismatch { .. } => 59,
            ExecError::ProposalContentMismatch => 60,
            ExecError::InvalidAdminAction(_) => 61,
            ExecError::AdminActionTooLarge { .. } => 62,
            ExecError::TooManyPendingProposals => 63,
            ExecError::ProposalIdExhausted => 64,
            ExecError::ConflictingVote => 65,
            ExecError::MarketIdOutOfRange { .. } => 66,
            ExecError::EmergencyRateLimited { .. } => 67,
            ExecError::EmergencyGlobalRateLimited { .. } => 68,
            ExecError::EmergencyActionRetired => 69,
            ExecError::AdminActionRequiresProposal => 70,
            ExecError::InvalidAdminRegistry(_) => 71,
            ExecError::BridgeReceiptRegistryInactive => 72,
            ExecError::BridgeReceiptInvalid(_) => 73,
            ExecError::BridgeReceiptMismatch(_) => 74,
            ExecError::WithdrawalBelowMinimum { .. } => 75,
            ExecError::WithdrawalTerminalGated(_) => 76,
            ExecError::OracleGuardUnset { .. } => 82,
            ExecError::SubAccountNotFound => 83,
            ExecError::SubAccountAlreadyExists => 84,
            ExecError::SubAccountTransferSameAccount => 85,
            ExecError::SubAccountTransferBothChildren => 86,
            ExecError::SubAccountTransferInsufficientBalance => 87,
            ExecError::SubAccountIdZero => 88,
            ExecError::InternalError(_) => 255,
        }
    }

    /// Stable, one-line human-readable meaning per variant. Intended for
    /// documentation and integration-guide tables (e.g. the openapi.yaml
    /// `ExecErrorCode` table for Auros and other MMs). The string is the
    /// **integration contract**: don't reword these without bumping a
    /// minor doc version, since downstream tooling may key off them.
    ///
    /// Wording rules: present-tense, action-oriented, names the cause not
    /// the symptom. "Order ID does not exist on the requested market"
    /// beats "the order was not found" — clients want to know what to
    /// fix, not just that something failed.
    pub fn meaning(&self) -> &'static str {
        match self {
            ExecError::DecodeError(_) => {
                "Tx envelope or payload could not be decoded as MessagePack at the expected version. \
                 Indicates a malformed wire frame or a client/server version mismatch."
            }
            ExecError::OrderNotFound(_) => {
                "Order ID does not exist on the requested market, or has already been filled or cancelled."
            }
            ExecError::NotOwner(_) => {
                "Tx signer is not the owner of the referenced order; only the owner (or an approved agent) \
                 may cancel or amend it."
            }
            ExecError::UnauthorizedOracle => {
                "Oracle update was signed by a key that is not registered as an oracle signer for the market."
            }
            ExecError::AdminGovernanceInactive => {
                "Admin governance action (propose/approve/reject/emergency) was submitted while no \
                 admin signer registry exists on this chain; multisig administration is inactive and \
                 every governance path fails closed."
            }
            ExecError::NotAdminSigner => {
                "Tx signer does not match the action's declared proposer/approver/rejecter/signer \
                 field, or is not a member of the current admin signer registry."
            }
            ExecError::ProposalNotFound(_) => {
                "No admin proposal exists under the given id. Either it was never created or \
                 terminal-retention pruning removed it; query /v1/proposals for the live set."
            }
            ExecError::ProposalNotPending { .. } => {
                "The admin proposal is already terminal (executed, failed, rejected, or expired); \
                 votes are only accepted while it is pending."
            }
            ExecError::ProposalExpired(_) => {
                "The admin proposal passed its TTL. Approvals are never durable credentials — \
                 re-propose and collect fresh signatures."
            }
            ExecError::DuplicateApproval => {
                "This signer already approved the proposal (the proposer's own approval is \
                 recorded at creation). Each member votes at most once."
            }
            ExecError::DuplicateRejection => {
                "This signer already rejected the proposal. Each member votes at most once."
            }
            ExecError::ProposalRegistryVersionMismatch { .. } => {
                "The submitted registry version does not match the current signer registry. A \
                 rotation happened since the payload was built — re-read the registry and re-sign."
            }
            ExecError::ProposalContentMismatch => {
                "The approval's immutable context does not exactly match the stored proposal \
                 (field-by-field plus canonical action bytes plus recomputed content hash). \
                 Rebuild the approval from the proposals query; never sign a summary."
            }
            ExecError::InvalidAdminAction(_) => {
                "The inner admin or emergency action failed validation: unknown or not-admitted \
                 arm, non-zero inner signer on a governance-authorized action, or an out-of-bounds \
                 field."
            }
            ExecError::AdminActionTooLarge { .. } => {
                "The canonical inner action encoding exceeds MAX_ADMIN_ACTION_BYTES; the proposal \
                 is rejected before any state is written."
            }
            ExecError::TooManyPendingProposals => {
                "MAX_PENDING_PROPOSALS admin proposals are already pending. Approve, reject, or \
                 let one expire before proposing again."
            }
            ExecError::ProposalIdExhausted => {
                "A governance id counter reached its maximum; ids never wrap. This is effectively \
                 unreachable and indicates a defect if observed."
            }
            ExecError::ConflictingVote => {
                "This signer already cast the opposite vote on the proposal. Votes are immutable: \
                 approvals and rejections are disjoint and cannot be switched."
            }
            ExecError::MarketIdOutOfRange { .. } => {
                "Market id exceeds the signed-32-bit identity bound (i32::MAX) that downstream \
                 columns, parsers, and APIs rely on. Pick a smaller id."
            }
            ExecError::EmergencyRateLimited { .. } => {
                "This signer exceeded the per-signer emergency action bound within the rolling \
                 window; the action was not executed."
            }
            ExecError::EmergencyGlobalRateLimited { .. } => {
                "The chain-wide emergency action bound was reached within the rolling window; the \
                 action was not executed."
            }
            ExecError::EmergencyActionRetired => {
                "This emergency arm was retired at a protocol boundary. Historical executions \
                 replay unchanged; new submissions of the arm fail closed."
            }
            ExecError::AdminActionRequiresProposal => {
                "The admin signer registry exists, so this registry-gated admin action is \
                 proposal-only: its direct path is closed even for an authorized relayer. \
                 Submit it as a governance proposal instead."
            }
            ExecError::InvalidAdminRegistry(_) => {
                "The proposed signer roster violates the registry invariants: threshold in \
                 [2, members], canonically sorted duplicate-free members, roster within \
                 MAX_ADMIN_SIGNERS, version headroom. Nothing was mutated."
            }
            ExecError::Overflow => {
                "An arithmetic operation (price * quantity, fee accrual, position size) overflowed a u64. \
                 Almost always indicates a malformed input rather than legitimate volume."
            }
            ExecError::InvalidPrice => {
                "Price is zero, exceeds the per-market max, or violates tick-size quantization."
            }
            ExecError::InvalidQuantity => {
                "Quantity is zero, exceeds the per-market max, or violates lot-size quantization."
            }
            ExecError::InvalidSide => "Side byte is neither Buy (0) nor Sell (1).",
            ExecError::UnknownMarket(_) => {
                "Market ID is not registered. Either the market does not exist or it has been removed."
            }
            ExecError::InsufficientBalance => {
                "Account's USDC balance cannot cover the requested debit (deposit, withdrawal, or fee)."
            }
            ExecError::InsufficientMargin => {
                "Post-trade equity would fall below the initial-margin requirement for the resulting \
                 portfolio. Reduce order size, add collateral, or close offsetting positions."
            }
            ExecError::StateCorruption(_) => {
                "Engine read state in an unexpected shape (e.g. missing required key, malformed value). \
                 Indicates a bug or data corruption — file an issue with the surrounding context."
            }
            ExecError::BridgeReceiptRegistryInactive => {
                "A receipt-gated withdrawal action (ConfirmWithdrawalReceipt / \
                 FailWithdrawalReceipt) was submitted while no operator receipt registry \
                 exists on this chain; the operator custody phase is inactive and the path \
                 fails closed."
            }
            ExecError::BridgeReceiptInvalid(_) => {
                "The operator quorum proof failed bridge-core verification: malformed proof \
                 structure, fewer than m distinct operator members, or an invalid ed25519 \
                 signature over the receipt bytes."
            }
            ExecError::BridgeReceiptMismatch(_) => {
                "The signed bridge receipt does not bind to this withdrawal or deployment \
                 (deployment id, withdrawal id, owner, amount, destination, authority epoch, \
                 quorum kind, or terminal state diverged)."
            }
            ExecError::WithdrawalBelowMinimum { .. } => {
                "Net withdrawal amount is below the configured minimum (set above the flat protocol \
                 fee), so the payout would be worth less than it costs to settle."
            }
            ExecError::WithdrawalTerminalGated(_) => {
                "A retired legacy relayer withdrawal terminal (ConfirmWithdrawal / FailWithdrawal) \
                 was submitted at or after the bridge receipt cutover. (An operator-receipt \
                 terminal submitted before the cutover instead fails as a decode error, code 1, \
                 byte-identically to the pre-upgrade binary.)"
            }
            ExecError::SubAccountNotFound => {
                "No sub-account exists in the registry for the given master address and sub-account id."
            }
            ExecError::SubAccountAlreadyExists => {
                "A sub-account with this master address and sub-account id already exists in the registry."
            }
            ExecError::SubAccountTransferSameAccount => {
                "Transfer from and to addresses are identical; no-op transfers are rejected."
            }
            ExecError::SubAccountTransferBothChildren => {
                "Neither side of a transfer is the master owner address; at least one side must be the master."
            }
            ExecError::SubAccountTransferInsufficientBalance => {
                "Source account has insufficient balance to complete the transfer."
            }
            ExecError::SubAccountIdZero => {
                "Sub-account id must be non-zero; id 0 is not a valid sub-account id."
            }
            ExecError::InternalError(_) => {
                "Catch-all for unexpected runtime failures (panics caught by the FFI boundary, etc.). \
                 Treat as a server bug."
            }
            ExecError::UnauthorizedRelayer => {
                "Tx is a relayer-only action (oracle update, funding tick, deposit confirmation, etc.) \
                 but the signer is not registered as an authorized relayer."
            }
            ExecError::WithdrawalNotFound(_) => {
                "Withdrawal ID does not exist or has already been claimed/refunded."
            }
            ExecError::WithdrawalAlreadyProcessed(_) => {
                "Withdrawal was already settled (claim or refund); duplicate finalize call rejected."
            }
            ExecError::DuplicateDeposit => {
                "On-chain deposit signature has already been credited; idempotency guard rejected the replay."
            }
            ExecError::InvalidSignature => {
                "Ed25519 verification of the V2 envelope failed. Signature, pubkey, or signed bytes are wrong."
            }
            ExecError::SignatureRequired => {
                "A signed (V2) transaction envelope is required. Unsigned (V1) envelopes are not accepted."
            }
            ExecError::AgentNotAuthorized => {
                "Tx signer is neither the account owner nor on the owner's approved-agent list."
            }
            ExecError::AgentCannotWithdraw => {
                "Agent wallets may place/cancel/market orders but are forbidden from initiating withdrawals."
            }
            ExecError::NonceTooOld { .. }
            | ExecError::NonceTooFarFuture { .. }
            | ExecError::NonceReplay { .. }
            | ExecError::NonceBelowOldest { .. } => {
                "Timestamp nonce failed replay-window validation. Use a unique millisecond Unix timestamp within \
                 [block_time-2d, block_time+1d]; included failures burn their nonce."
            }
            ExecError::MarketAlreadyExists(_) => {
                "Attempted CreateMarket for a market ID already in the registry."
            }
            ExecError::InvalidMarketConfig(_) => {
                "MarketConfig fields fail validation (e.g. fee bps out of range, lot/tick zero, IM/MM ratio \
                 inverted)."
            }
            ExecError::ImpactMarketAlreadyExists(_) => {
                "Attempted CreateImpactMarket for an impact market ID already in the registry."
            }
            ExecError::ImpactMarketNotFound(_) => {
                "Impact market ID does not exist; cannot resolve, cash-out, or query."
            }
            ExecError::MarketClosedForTrading(_) => {
                "Order placement attempted on a conditional/binary book whose parent impact market is \
                 already resolved or voided."
            }
            ExecError::BinaryPriceOutOfRange => {
                "Binary-book order price is outside the [0, BINARY_PRICE_MAX] range."
            }
            ExecError::InvalidResolution(_) => {
                "ResolveImpactMarket called with an outcome incompatible with the current state (already resolved, \
                 outcome not in the configured set, etc.)."
            }
            ExecError::PositionLimitExceeded { .. } => {
                "Fill would push absolute net position past MarketConfig.max_position_size. Engine cap \
                 enforced at placement time independent of margin."
            }
            ExecError::OpenInterestLimitExceeded { .. } => {
                "Fill would violate MarketConfig.max_open_interest. At schema v9+, an already-over-cap market \
                 may preserve or reduce its pre-fill aggregate OI but may not increase it; enforcement occurs \
                 before either side's position is mutated."
            }
            ExecError::OracleTimestampNotMonotonic { .. } => {
                "OracleUpdate publish_time_ms is not strictly greater than the last accepted update for \
                 this market — replay protection per audit B3 (2026-04-23)."
            }
            ExecError::TooManyActiveImpactMarkets { .. } => {
                "Account would touch more impact markets than the scenario margin engine can enumerate \
                 (MAX_IMPACT_MARKETS_PER_ACCOUNT). Close a leg before opening another."
            }
            ExecError::SettlementPriceMismatch { .. } => {
                "Net-delta margin grouping found legs with disagreeing settle prices (data corruption \
                 across same `underlying_market_id`)."
            }
            ExecError::OracleNotApplicable { .. } => {
                "OracleUpdate targets an impact-family market (CPY/CPN/EBY/EBN), which marks off the book \
                 and has no oracle layer."
            }
            ExecError::PostOnlyWouldCross => {
                "PlaceOrder with post_only=true would have crossed the book. Rejected so makers retain \
                 maker-side fills."
            }
            ExecError::ReduceOnlyWouldIncrease => {
                "PlaceOrder/MarketOrder with reduce_only=true was same-side as the existing position (would \
                 increase exposure) or no position existed."
            }
            ExecError::TestActionRejected(_) => {
                "Test/admin action (RunLiquidationSweep, RunFundingTick, ForceLiquidate) rejected because \
                 the engine isn't configured to accept them, or the position the action referenced does \
                 not exist."
            }
            ExecError::StaleOracle { .. } => {
                "Oracle price is stale for this market; refresh the oracle before placing orders, reading margin, or liquidating."
            }
            ExecError::OracleGuardUnset { .. } => {
                "The oracle-guard gate is active and this market has no mark-price max oracle age set; every mark-dependent action is refused until governance sets one."
            }
            ExecError::UserLeverageBelowMarketIm { .. } => {
                "User-selected initial margin is below the market risk floor; only deleveraging above the market floor is allowed."
            }
            ExecError::TickSizeViolation { .. } => {
                "Order price is not an exact multiple of the market tick size."
            }
            ExecError::LotSizeViolation { .. } => {
                "Order quantity is not an exact multiple of the market lot size."
            }
            ExecError::OracleStaleNotElapsed { .. } => {
                "Fallback oracle signer published before the market staleness window elapsed."
            }
            ExecError::FeeBpsOutOfRange { .. } => {
                "Per-account fee override has a fee outside the legal basis-point range."
            }
            ExecError::FeeOverrideStaleSeq { .. } => {
                "Per-account fee override sequence is stale or out of order."
            }
            ExecError::ClientOrderIdNotFound { .. } => {
                "No active resting order exists for the requested client order id."
            }
            ExecError::DuplicateClientOrderId { .. } => {
                "An active resting order already uses the requested client order id."
            }
            ExecError::InvalidClientOrderId { .. } => {
                "Client order id zero is reserved and cannot be submitted."
            }
            ExecError::FillOrKillWouldNotFill { .. } => {
                "Fill-or-kill order cannot be fully filled immediately at the submitted limit price."
            }
            ExecError::InvalidCancelReplaceTarget => {
                "Cancel-replace must specify exactly one active order target: either orderId or clientOrderId."
            }
            ExecError::AmendBelowFilled { .. } => {
                "AmendOrder new quantity is below the quantity already filled while the order rested."
            }
            ExecError::SlippageExceeded { .. } => {
                "Atomic basket aggregate slippage exceeded the requested maxSlippageBps budget."
            }
        }
    }

    /// This market cannot settle in this block, but can in the next.
    pub fn is_settlement_retryable(&self) -> bool {
        matches!(
            self,
            ExecError::InvalidResolution(_)
                | ExecError::StaleOracle { .. }
                | ExecError::OracleGuardUnset { .. }
                | ExecError::UnknownMarket(_)
        )
    }
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecError::DecodeError(msg) => write!(f, "decode error: {msg}"),
            ExecError::OrderNotFound(id) => write!(f, "order not found: {id}"),
            ExecError::NotOwner(id) => write!(f, "not owner of order: {id}"),
            ExecError::UnauthorizedOracle => write!(f, "unauthorized oracle signer"),
            ExecError::Overflow => write!(f, "arithmetic overflow"),
            ExecError::InvalidPrice => write!(f, "invalid price"),
            ExecError::InvalidQuantity => write!(f, "invalid quantity"),
            ExecError::InvalidSide => write!(f, "invalid side"),
            ExecError::UnknownMarket(id) => write!(f, "unknown market: {id}"),
            ExecError::InsufficientBalance => write!(f, "insufficient balance"),
            ExecError::InsufficientMargin => write!(f, "insufficient margin"),
            ExecError::StateCorruption(msg) => write!(f, "state corruption: {msg}"),
            ExecError::UnauthorizedRelayer => write!(f, "unauthorized relayer signer"),
            ExecError::AdminGovernanceInactive => {
                write!(f, "admin governance inactive: no signer registry exists")
            }
            ExecError::NotAdminSigner => write!(f, "not an authorized admin signer"),
            ExecError::ProposalNotFound(id) => write!(f, "proposal not found: {}", id.0),
            ExecError::ProposalNotPending { id, status } => {
                write!(f, "proposal {} is not pending: {}", id.0, status.label())
            }
            ExecError::ProposalExpired(id) => write!(f, "proposal expired: {}", id.0),
            ExecError::DuplicateApproval => write!(f, "signer already approved this proposal"),
            ExecError::DuplicateRejection => write!(f, "signer already rejected this proposal"),
            ExecError::ProposalRegistryVersionMismatch { expected, got } => write!(
                f,
                "registry version mismatch: expected {}, got {}",
                expected.0, got.0
            ),
            ExecError::ProposalContentMismatch => {
                write!(f, "approval context does not match the stored proposal")
            }
            ExecError::InvalidAdminAction(reason) => {
                write!(f, "invalid admin action: {reason}")
            }
            ExecError::AdminActionTooLarge { max, got } => {
                write!(f, "admin action too large: {got} bytes exceeds max {max}")
            }
            ExecError::TooManyPendingProposals => {
                write!(f, "too many pending proposals")
            }
            ExecError::ProposalIdExhausted => write!(f, "governance id counter exhausted"),
            ExecError::ConflictingVote => {
                write!(f, "opposite vote already recorded; votes are immutable")
            }
            ExecError::MarketIdOutOfRange { max, got } => {
                write!(f, "market id out of range: {got} exceeds max {max}")
            }
            ExecError::EmergencyRateLimited { max, window_ms } => write!(
                f,
                "emergency rate limited: max {max} per signer per {window_ms} ms"
            ),
            ExecError::EmergencyGlobalRateLimited { max, window_ms } => write!(
                f,
                "emergency rate limited: max {max} chain-wide per {window_ms} ms"
            ),
            ExecError::EmergencyActionRetired => write!(f, "emergency arm retired"),
            ExecError::AdminActionRequiresProposal => {
                write!(f, "admin action requires a governance proposal")
            }
            ExecError::InvalidAdminRegistry(reason) => {
                write!(f, "invalid admin registry: {reason}")
            }
            ExecError::WithdrawalNotFound(id) => write!(f, "withdrawal not found: {id}"),
            ExecError::WithdrawalAlreadyProcessed(id) => {
                write!(f, "withdrawal already processed: {id}")
            }
            ExecError::WithdrawalBelowMinimum { amount, min } => {
                write!(f, "withdrawal amount {amount} below minimum {min}")
            }
            ExecError::DuplicateDeposit => write!(f, "duplicate deposit signature"),
            ExecError::InvalidSignature => write!(f, "invalid Ed25519 signature"),
            ExecError::SignatureRequired => write!(f, "signed transaction required"),
            ExecError::AgentNotAuthorized => {
                write!(f, "signer is not owner or authorized agent")
            }
            ExecError::AgentCannotWithdraw => {
                write!(f, "agent wallets cannot perform withdrawals")
            }
            ExecError::NonceTooOld { min_accepted, got } => {
                write!(
                    f,
                    "nonce too old: minimum accepted {min_accepted}, got {got}"
                )
            }
            ExecError::NonceTooFarFuture { max_accepted, got } => {
                write!(
                    f,
                    "nonce too far in future: maximum accepted {max_accepted}, got {got}"
                )
            }
            ExecError::NonceReplay { nonce } => write!(f, "nonce replay: {nonce}"),
            ExecError::NonceBelowOldest { oldest, got } => {
                write!(f, "nonce below retained oldest: oldest {oldest}, got {got}")
            }
            ExecError::MarketAlreadyExists(id) => write!(f, "market already exists: {id}"),
            ExecError::InvalidMarketConfig(msg) => write!(f, "invalid market config: {msg}"),
            ExecError::ImpactMarketAlreadyExists(id) => {
                write!(f, "impact market already exists: {id}")
            }
            ExecError::ImpactMarketNotFound(id) => write!(f, "impact market not found: {id}"),
            ExecError::MarketClosedForTrading(id) => {
                write!(f, "market closed for trading: {id}")
            }
            ExecError::BinaryPriceOutOfRange => write!(
                f,
                "prediction-binary price must be in [0, {}]",
                BINARY_PRICE_MAX
            ),
            ExecError::InvalidResolution(msg) => write!(f, "invalid resolution: {msg}"),
            ExecError::PositionLimitExceeded {
                market,
                limit,
                would_be,
            } => write!(
                f,
                "position limit exceeded on market {market}: would be {would_be}, cap {limit}"
            ),
            ExecError::OpenInterestLimitExceeded {
                market,
                limit,
                would_be,
            } => write!(
                f,
                "open interest limit exceeded on market {market}: would be {would_be}, cap {limit}"
            ),
            ExecError::OracleTimestampNotMonotonic {
                market,
                stored,
                submitted,
            } => write!(
                f,
                "oracle publish_time_ms not strictly monotonic on market {market}: \
                stored {stored}, submitted {submitted} (submitted must be > stored)"
            ),
            ExecError::TooManyActiveImpactMarkets { current, max } => write!(
                f,
                "too many active impact markets in basket: current {current}, cap {max}"
            ),
            ExecError::SettlementPriceMismatch {
                market,
                expected,
                got,
            } => write!(
                f,
                "settle price disagreement on market {market}: expected {expected} (group key), got {got}"
            ),
            ExecError::OracleNotApplicable { market } => write!(
                f,
                "oracle update rejected for market {market}: impact-family markets mark off the book directly and have no oracle layer (post 2026-04-26 redesign)"
            ),
            ExecError::PostOnlyWouldCross => write!(
                f,
                "post-only order would cross the book on placement; rejected"
            ),
            ExecError::ReduceOnlyWouldIncrease => write!(
                f,
                "reduce-only order rejected: would increase exposure (same-side as position) or no position to reduce"
            ),
            ExecError::TestActionRejected(msg) => write!(f, "test action rejected: {msg}"),
            ExecError::FeeBpsOutOfRange { bps } => {
                write!(f, "fee out of range: {bps} bps (must be in [0, 10_000])")
            }
            ExecError::FeeOverrideStaleSeq {
                cmd_seq,
                stored_seq,
            } => write!(
                f,
                "stale SetAccountFeeOverride seq: cmd seq {cmd_seq} <= stored seq {stored_seq} (replay or out-of-order tx)"
            ),
            ExecError::TickSizeViolation {
                market,
                tick_size,
                price,
            } => write!(
                f,
                "tick size violation on market {market}: price {price} not a multiple of tick_size {tick_size}"
            ),
            ExecError::LotSizeViolation {
                market,
                lot_size,
                quantity,
            } => write!(
                f,
                "lot size violation on market {market}: quantity {quantity} not a multiple of lot_size {lot_size}"
            ),
            ExecError::OracleStaleNotElapsed {
                market,
                last_publish_ms,
                block_time_ms,
                staleness_ms,
            } => write!(
                f,
                "oracle update from fallback signer rejected on market {market}: \
                primary's last_publish_ms {last_publish_ms} is too recent \
                (block_time {block_time_ms} - last_publish < staleness_ms {staleness_ms})"
            ),
            ExecError::StaleOracle {
                market,
                publish_time_ms,
                block_time_ms,
                max_staleness_ms,
            } => write!(
                f,
                "oracle stale on market {market}: last publish_time {publish_time_ms}ms, \
                block time {block_time_ms}ms (age {age}ms > cap {max_staleness_ms}ms)",
                age = block_time_ms.saturating_sub(*publish_time_ms),
            ),
            ExecError::OracleGuardUnset { market } => write!(
                f,
                "oracle guard unset on market {market}: mark_price_max_oracle_age_ms is 0 while the \
                oracle-guard gate is active; mark-dependent actions are refused until governance sets it"
            ),
            ExecError::UserLeverageBelowMarketIm {
                market,
                user_im_bps,
                market_im_bps,
            } => write!(
                f,
                "user_im_bps {user_im_bps} below market {market}'s im_bps {market_im_bps}; \
                only deleveraging (user_im >= market_im) is allowed"
            ),
            ExecError::ClientOrderIdNotFound { client_order_id } => {
                write!(f, "client order id not found: {client_order_id}")
            }
            ExecError::DuplicateClientOrderId { client_order_id } => {
                write!(f, "duplicate active client order id: {client_order_id}")
            }
            ExecError::InvalidClientOrderId { client_order_id } => {
                write!(f, "invalid client order id: {client_order_id}")
            }
            ExecError::FillOrKillWouldNotFill {
                requested,
                available,
            } => write!(
                f,
                "fill-or-kill would not fully fill: requested {requested}, available {available}"
            ),
            ExecError::InvalidCancelReplaceTarget => {
                write!(f, "cancel-replace requires exactly one cancel target")
            }
            ExecError::AmendBelowFilled {
                order_id,
                filled_quantity,
                requested_quantity,
            } => write!(
                f,
                "amend quantity below filled for order {order_id}: requested total {requested_quantity}, filled {filled_quantity}"
            ),
            ExecError::SlippageExceeded {
                aggregate_bps,
                max_slippage_bps,
            } => write!(
                f,
                "atomic basket aggregate slippage {aggregate_bps} bps exceeds budget {max_slippage_bps} bps"
            ),
            ExecError::BridgeReceiptRegistryInactive => {
                write!(f, "bridge receipt registry inactive: operator custody phase not configured")
            }
            ExecError::BridgeReceiptInvalid(msg) => write!(f, "bridge receipt invalid: {msg}"),
            ExecError::BridgeReceiptMismatch(msg) => write!(f, "bridge receipt mismatch: {msg}"),
            ExecError::WithdrawalTerminalGated(msg) => {
                write!(f, "withdrawal terminal gated by receipt cutover: {msg}")
            }
            ExecError::SubAccountNotFound => {
                write!(f, "sub-account not found in registry")
            }
            ExecError::SubAccountAlreadyExists => {
                write!(f, "sub-account already exists in registry")
            }
            ExecError::SubAccountTransferSameAccount => {
                write!(f, "transfer from and to are the same address")
            }
            ExecError::SubAccountTransferBothChildren => {
                write!(f, "transfer requires at least one side to be the master owner")
            }
            ExecError::SubAccountTransferInsufficientBalance => {
                write!(f, "insufficient balance for sub-account transfer")
            }
            ExecError::SubAccountIdZero => {
                write!(f, "sub-account id must be non-zero")
            }
            ExecError::InternalError(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Prelude — commonly used types for internal imports
// ---------------------------------------------------------------------------

pub mod prelude {
    pub use crate::types::{
        AccountFeeOverride, Action, AmendOrder, ApproveAgent, AtomicBasketLeg, AtomicBasketOrder,
        AuthorizeWithdrawal, Branch, BridgeWithdrawalReceipt, CancelAllOrders, CancelClientOrder,
        CancelOrder, CancelReason, CancelReplaceOrder, ClosePosition, ConfirmDeposit,
        ConfirmWithdrawal, ConfirmWithdrawalReceipt, CreateImpactMarket, CreateMarket,
        CreateSubAccount, Deposit, DepositLocator, Event, EventOracleSource, ExecError,
        FailDeposit, FailWithdrawal, FailWithdrawalReceipt, FillId, FundingSkipReason,
        ImpactMarketId, ImpactMarketInfo, ImpactMarketStatus, LiquidateAccounts, MarkSourceMode,
        MarketConfig, MarketId, MarketKind, MarketOracleGuards, MarketOrder, OpenInterest,
        OperatorReceiptProof, OperatorReceiptRegistry, OracleRejectReason, OracleUpdate,
        OracleUpdateComposite, Order, OrderId, Outcome, PlaceOrder, Position, ResolveEvent,
        ResolveImpactMarket, RevokeAgent, RunFundingTick, RunLiquidationSweep,
        SetAccountFeeOverride, SetUserMarketLeverage, Side, SubAccount, SubAccountTransfer,
        TimeInForce, TxContext, UpdateMarketFees, Withdraw, WithdrawRequest,
        WithdrawalReceiptSidecar, WithdrawalRecord, WithdrawalStatus, BINARY_PRICE_MAX,
        DEFAULT_CEX_COMPOSITE_STALENESS_MS, DEFAULT_MAX_MARK_SPREAD_BPS,
        DEFAULT_MAX_ORACLE_DEVIATION_BPS, DEFAULT_STALE_LAST_GOOD_HARD_CAP_FACTOR,
        MARK_MIN_BOOK_NOTIONAL_UUSDC, PREDICTION_BINARY_LOT_SIZE, PREDICTION_BINARY_SZ_DECIMALS,
        PREDICTION_BINARY_TICK_SIZE,
    };
}
