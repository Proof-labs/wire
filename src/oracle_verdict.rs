//! The oracle policy's per-block verdict as clients read it from the
//! `oracle_price_certified` and `oracle_price_uncertified` events.
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::types::OracleReason;

/// Whether a market's verdict certifies a price this block.
///
/// The engine persists this inside its committed verdict records, so the
/// variant order and names are a storage contract: append only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleStatus {
    Fresh,
    Stale,
    Unpriceable,
}

impl fmt::Display for OracleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Unpriceable => "unpriceable",
        })
    }
}

/// Bit `i` is set when current slot `i` is usable at the classified block
/// time: a structurally valid, authenticated, quality-good record inside
/// `max_age` and the open session, from a slot not poisoned this block and,
/// for the primary, not refused for divergence. During a closed or unknown
/// session only the age and future bounds apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMask(u8);

impl SourceMask {
    pub const EMPTY: Self = Self(0);
    pub const BOTH: Self = Self(0b11);

    pub fn insert(&mut self, index: usize) {
        if let Some(bit) = 1_u8.checked_shl(index as u32) {
            self.0 |= bit;
        }
    }

    pub fn contains(self, index: usize) -> bool {
        1_u8.checked_shl(index as u32)
            .is_some_and(|bit| self.0 & bit != 0)
    }

    pub fn bits(self) -> u8 {
        self.0
    }
}

/// Renders the bit set as its integer.
impl fmt::Display for SourceMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What the price prefix did with one admitted source observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleAdmission {
    Current,
    Pending,
    Duplicate,
    Older,
    Conflict,
    InvalidObservation,
    FutureRejected,
    InvalidPolicy,
    ClockRegression,
}

impl fmt::Display for OracleAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Current => "current",
            Self::Pending => "pending",
            Self::Duplicate => "duplicate",
            Self::Older => "older",
            Self::Conflict => "conflict",
            Self::InvalidObservation => "invalid_observation",
            Self::FutureRejected => "future_rejected",
            Self::InvalidPolicy => "invalid_policy",
            Self::ClockRegression => "clock_regression",
        })
    }
}

impl fmt::Display for OracleReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPolicy => "invalid_policy",
            Self::UpdateLimitExceeded => "update_limit_exceeded",
            Self::ClockRegression => "clock_regression",
            Self::ManualHalt => "manual_halt",
            Self::SessionUnknown => "session_unknown",
            Self::SessionClosed => "session_closed",
            Self::BadQuality => "bad_quality",
            Self::Disagreement => "disagreement",
            Self::ReferenceUnavailable => "reference_unavailable",
            Self::MovementBound => "movement_bound",
            Self::MissingSource => "missing_source",
            Self::ExpiredSource => "expired_source",
            Self::OutsideSession => "outside_session",
            Self::PairTimeMismatch => "pair_time_mismatch",
            Self::RecoveryPending => "recovery_pending",
            Self::Fresh => "fresh",
        })
    }
}

/// A list event attribute, rendered comma-separated; empty when the list is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeList<T>(pub Vec<T>);

impl<T: fmt::Display> fmt::Display for AttributeList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, item) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{item}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_render_comma_separated_and_empty() {
        assert_eq!(AttributeList::<OracleAdmission>(Vec::new()).to_string(), "");
        assert_eq!(
            AttributeList(vec![
                OracleAdmission::Current,
                OracleAdmission::FutureRejected
            ])
            .to_string(),
            "current,future_rejected"
        );
        assert_eq!(
            AttributeList(vec![
                OracleReason::ReferenceUnavailable,
                OracleReason::SessionClosed
            ])
            .to_string(),
            "reference_unavailable,session_closed"
        );
    }

    #[test]
    fn spellings_are_snake_case_of_the_variant() {
        for (status, spelled) in [
            (OracleStatus::Fresh, "Fresh"),
            (OracleStatus::Stale, "Stale"),
            (OracleStatus::Unpriceable, "Unpriceable"),
        ] {
            assert_eq!(status.to_string(), spelled.to_lowercase());
            assert_eq!(format!("{status:?}"), spelled);
        }
        let reason = |reason: OracleReason| {
            let debug = format!("{reason:?}");
            let mut snake = String::new();
            for (index, ch) in debug.chars().enumerate() {
                if ch.is_uppercase() && index > 0 {
                    snake.push('_');
                }
                snake.push(ch.to_ascii_lowercase());
            }
            assert_eq!(reason.to_string(), snake, "reason {debug}");
        };
        for r in [
            OracleReason::InvalidPolicy,
            OracleReason::UpdateLimitExceeded,
            OracleReason::ClockRegression,
            OracleReason::ManualHalt,
            OracleReason::SessionUnknown,
            OracleReason::SessionClosed,
            OracleReason::BadQuality,
            OracleReason::Disagreement,
            OracleReason::ReferenceUnavailable,
            OracleReason::MovementBound,
            OracleReason::MissingSource,
            OracleReason::ExpiredSource,
            OracleReason::OutsideSession,
            OracleReason::PairTimeMismatch,
            OracleReason::RecoveryPending,
            OracleReason::Fresh,
        ] {
            reason(r);
        }
    }
}
