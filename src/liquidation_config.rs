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

/// A bounded count of liquidation work items: book orders visited, candidate
/// prefixes inspected, fills committed or state reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LiquidationWorkCount(pub u32);

/// A bound on the bytes of state one liquidation attempt may read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LiquidationReadBytes(pub u32);

/// The liquidation settings shared by every market.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidationConfig {
    pub target_ratio_bps: TargetRatioBps,
    pub full_close_threshold: FullCloseThreshold,
    pub partial_penalty_bps: LiquidationPenaltyBps,
    pub max_book_orders: LiquidationWorkCount,
    pub max_candidate_prefixes: LiquidationWorkCount,
    pub max_committed_fills: LiquidationWorkCount,
    pub max_state_reads: LiquidationWorkCount,
    pub max_state_read_bytes: LiquidationReadBytes,
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
    /// A work limit is zero.
    ZeroWorkLimit,
    /// More fills could commit than candidate prefixes may be inspected.
    FillsExceedCandidatePrefixes {
        max_committed_fills: u32,
        max_candidate_prefixes: u32,
    },
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
            Self::ZeroWorkLimit => f.write_str("every work limit must be non-zero"),
            Self::FillsExceedCandidatePrefixes {
                max_committed_fills,
                max_candidate_prefixes,
            } => write!(
                f,
                "max committed fills {max_committed_fills} exceeds max candidate prefixes {max_candidate_prefixes}"
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
        if [
            self.max_book_orders.0,
            self.max_candidate_prefixes.0,
            self.max_committed_fills.0,
            self.max_state_reads.0,
            self.max_state_read_bytes.0,
        ]
        .contains(&0)
        {
            return Err(LiquidationConfigError::ZeroWorkLimit);
        }
        if self.max_committed_fills > self.max_candidate_prefixes {
            return Err(LiquidationConfigError::FillsExceedCandidatePrefixes {
                max_committed_fills: self.max_committed_fills.0,
                max_candidate_prefixes: self.max_candidate_prefixes.0,
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
