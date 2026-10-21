//! The per-position liquidation record and the failed-attempt report, as
//! carried by `Event::PositionLiquidated` and `Event::LiquidationFailed`.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::query::BindingScenario;

/// Which path liquidated a position. MessagePack encodes a variant by its
/// name, so a variant is never renamed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiquidationMode {
    /// A partial step through the book toward the target ratio.
    Partial,
    /// A full close through the book, below the full-close threshold.
    Full,
    /// Negative equity in some outcome: closed at mark.
    Insolvent,
}

impl LiquidationMode {
    /// Every variant, in encoding order.
    pub const ALL: [Self; 3] = [Self::Partial, Self::Full, Self::Insolvent];

    /// The ABCI attribute value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Partial => "partial",
            Self::Full => "full",
            Self::Insolvent => "insolvent",
        }
    }
}

impl fmt::Display for LiquidationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The outcome with the lowest equity-to-maintenance ratio, one branch per
/// event of the account's domain in ascending event order. Empty when the
/// account holds no event exposure. Renders as `event_id:branch` pairs
/// joined by commas.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BindingOutcome(pub Vec<BindingScenario>);

impl fmt::Display for BindingOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, scenario) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{}:{}", scenario.event_id, scenario.branch)?;
        }
        Ok(())
    }
}

/// Why a liquidation attempt was refused other than by an engine error.
/// MessagePack encodes a variant by its name, so a variant is never renamed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiquidationRefusal {
    /// A position changed under the step that was reducing it.
    PositionMismatch,
    /// A resting order did not match its book.
    InvalidMaker,
    /// A reference price the attempt needs was unavailable.
    ReferenceUnavailable,
}

impl LiquidationRefusal {
    /// Every variant, in encoding order.
    pub const ALL: [Self; 3] = [
        Self::PositionMismatch,
        Self::InvalidMaker,
        Self::ReferenceUnavailable,
    ];

    /// The ABCI attribute value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PositionMismatch => "position_mismatch",
            Self::InvalidMaker => "invalid_maker",
            Self::ReferenceUnavailable => "reference_unavailable",
        }
    }
}

impl fmt::Display for LiquidationRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
