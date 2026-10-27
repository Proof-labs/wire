//! The global liquidation configuration and the `SetLiquidationConfig` admin
//! payload that replaces it.
//!
//! Collar width is not a field: it is each market's maintenance margin
//! fraction. Oracle freshness, lot and tick sizes and fees are the markets'
//! existing settings.

use serde::{Deserialize, Serialize};

/// Basis points that equal a ratio of one.
pub const RATIO_ONE_BPS: u32 = 10_000;

/// Largest partial-liquidation penalty a configuration may carry.
pub const MAX_PARTIAL_PENALTY_BPS: u32 = 100;

/// Position of a configuration in its update sequence. Each update names the
/// sequence it replaces and carries a strictly greater one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LiquidationConfigSequence(pub u64);

/// Equity-to-maintenance ratio a partial liquidation sizes toward, in basis
/// points of maintenance margin. Always above [`RATIO_ONE_BPS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TargetRatioBps(pub u32);

/// Exact equity-to-maintenance ratio `numerator / denominator` below which an
/// account is closed in full rather than partially. Strictly between zero
/// and one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullCloseThreshold {
    pub numerator: u32,
    pub denominator: u32,
}

/// Penalty charged on each partial-liquidation fill, in basis points of the
/// fill's notional. At most [`MAX_PARTIAL_PENALTY_BPS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LiquidationPenaltyBps(pub u32);

/// Most of the takeover discount beyond the maintenance rate, in basis
/// points.
pub const MAX_TAKEOVER_DISCOUNT_BPS: u32 = 100;

/// Discount beyond the maintenance rate at which a pool's insurance fund
/// takes over an insolvent position (DEC-264). At most
/// [`MAX_TAKEOVER_DISCOUNT_BPS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TakeoverDiscountBps(pub u32);

impl TakeoverDiscountBps {
    /// The discount a configuration written before the field existed takes.
    pub const INITIAL: Self = Self(50);

    fn initial() -> Self {
        Self::INITIAL
    }
}

/// A retired per-owner, per-block liquidation work count, kept so stored
/// records decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LiquidationWorkCount(pub u32);

/// The liquidation settings shared by every market.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationConfig {
    pub target_ratio_bps: TargetRatioBps,
    pub full_close_threshold: FullCloseThreshold,
    pub partial_penalty_bps: LiquidationPenaltyBps,
    /// Unused: liquidation has no per-block cap. Kept so records decode.
    pub max_book_orders: LiquidationWorkCount,
    /// Unused: liquidation has no per-block cap. Kept so records decode.
    pub max_committed_fills: LiquidationWorkCount,
    #[serde(default = "TakeoverDiscountBps::initial")]
    pub takeover_discount_bps: TakeoverDiscountBps,
}

/// The rule a configuration or an update breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidationConfigError {
    /// The target ratio is at or below one.
    TargetRatioNotAboveOne { target_ratio_bps: u32 },
    /// The full-close threshold is not strictly between zero and one.
    FullCloseThresholdOutOfRange { numerator: u32, denominator: u32 },
    /// The partial penalty exceeds [`MAX_PARTIAL_PENALTY_BPS`].
    PenaltyAboveMaximum { partial_penalty_bps: u32 },
    /// A pacing cap is zero; `field` names the configuration field.
    ZeroWorkLimit { field: &'static str },
    /// The takeover discount exceeds [`MAX_TAKEOVER_DISCOUNT_BPS`].
    TakeoverDiscountAboveMaximum { takeover_discount_bps: u32 },
    /// The update's sequence does not exceed the one it replaces.
    SequenceNotIncreasing {
        predecessor: LiquidationConfigSequence,
        sequence: LiquidationConfigSequence,
    },
}

impl core::fmt::Display for LiquidationConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TargetRatioNotAboveOne { target_ratio_bps } => write!(
                f,
                "target ratio {target_ratio_bps} bps must exceed {RATIO_ONE_BPS} bps"
            ),
            Self::FullCloseThresholdOutOfRange {
                numerator,
                denominator,
            } => write!(
                f,
                "full-close threshold {numerator}/{denominator} must be strictly between 0 and 1"
            ),
            Self::PenaltyAboveMaximum {
                partial_penalty_bps,
            } => write!(
                f,
                "partial penalty {partial_penalty_bps} bps exceeds {MAX_PARTIAL_PENALTY_BPS} bps"
            ),
            Self::ZeroWorkLimit { field } => {
                write!(f, "work limit {field} must be non-zero")
            }
            Self::TakeoverDiscountAboveMaximum {
                takeover_discount_bps,
            } => write!(
                f,
                "takeover discount {takeover_discount_bps} bps exceeds {MAX_TAKEOVER_DISCOUNT_BPS} bps"
            ),
            Self::SequenceNotIncreasing {
                predecessor,
                sequence,
            } => write!(
                f,
                "sequence {} must exceed predecessor {}",
                sequence.0, predecessor.0
            ),
        }
    }
}

impl LiquidationConfig {
    pub fn validate(&self) -> Result<(), LiquidationConfigError> {
        if self.target_ratio_bps.0 <= RATIO_ONE_BPS {
            return Err(LiquidationConfigError::TargetRatioNotAboveOne {
                target_ratio_bps: self.target_ratio_bps.0,
            });
        }
        let FullCloseThreshold {
            numerator,
            denominator,
        } = self.full_close_threshold;
        if numerator == 0 || numerator >= denominator {
            return Err(LiquidationConfigError::FullCloseThresholdOutOfRange {
                numerator,
                denominator,
            });
        }
        if self.partial_penalty_bps.0 > MAX_PARTIAL_PENALTY_BPS {
            return Err(LiquidationConfigError::PenaltyAboveMaximum {
                partial_penalty_bps: self.partial_penalty_bps.0,
            });
        }
        let limits = [
            ("max_book_orders", self.max_book_orders.0),
            ("max_committed_fills", self.max_committed_fills.0),
        ];
        if let Some((field, _)) = limits.iter().find(|(_, limit)| *limit == 0) {
            return Err(LiquidationConfigError::ZeroWorkLimit { field });
        }
        if self.takeover_discount_bps.0 > MAX_TAKEOVER_DISCOUNT_BPS {
            return Err(LiquidationConfigError::TakeoverDiscountAboveMaximum {
                takeover_discount_bps: self.takeover_discount_bps.0,
            });
        }
        Ok(())
    }
}

/// Payload of `AdminAction::SetLiquidationConfig`: replaces the configuration
/// stored at `predecessor` with `config` at `sequence`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetLiquidationConfig {
    /// Sequence of the configuration this update replaces.
    pub predecessor: LiquidationConfigSequence,
    /// Sequence of the new configuration.
    pub sequence: LiquidationConfigSequence,
    pub config: LiquidationConfig,
}

impl SetLiquidationConfig {
    pub fn validate(&self) -> Result<(), LiquidationConfigError> {
        if self.sequence <= self.predecessor {
            return Err(LiquidationConfigError::SequenceNotIncreasing {
                predecessor: self.predecessor,
                sequence: self.sequence,
            });
        }
        self.config.validate()
    }
}
