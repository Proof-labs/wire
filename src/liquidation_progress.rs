//! What one liquidation attempt on an owner did in a block, as carried by
//! `Event::LiquidationProgressReported`.

use core::fmt;

use serde::{Deserialize, Serialize};

/// How a nominated owner's liquidation ended in one block. MessagePack
/// encodes a variant by its name, so a variant is never renamed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiquidationProgress {
    /// Maintenance holds, or the owner holds no position.
    NotEligible,
    /// Margin could not be evaluated from current references.
    Indeterminate,
    /// Cancelling the owner's exposure-adding orders restored maintenance.
    RestoredByCancellation,
    /// No safe partial fill was found within the work limits.
    NoFill,
    /// Safe partial fills were made; the owner is still below maintenance.
    SafeProgress,
    /// Partial fills restored maintenance without reaching the target ratio.
    MaintenanceRestoredBelowTarget,
    /// Partial fills brought every outcome to the target ratio.
    TargetReached,
    /// Below the full-close threshold, cancelling the owner's
    /// exposure-adding orders restored maintenance.
    FullCloseRestoredByCancellation,
    /// Below the full-close threshold, no fill was made.
    FullCloseNoFill,
    /// Below the full-close threshold, fills were made and some perpetual
    /// exposure is still open.
    FullClosePartlyClosed,
    /// Below the full-close threshold, every perpetual leg was closed.
    FullCloseClosed,
    /// Negative equity in some outcome: closed at mark through the
    /// insurance-fund and auto-deleveraging waterfall.
    Insolvent,
    /// Not served: the batch's position budget was spent on owners ahead
    /// of it in the serving order.
    Deferred,
    /// The attempt failed and none of its writes were kept.
    Failed,
}

impl LiquidationProgress {
    /// Every variant, in encoding order.
    pub const ALL: [Self; 14] = [
        Self::NotEligible,
        Self::Indeterminate,
        Self::RestoredByCancellation,
        Self::NoFill,
        Self::SafeProgress,
        Self::MaintenanceRestoredBelowTarget,
        Self::TargetReached,
        Self::FullCloseRestoredByCancellation,
        Self::FullCloseNoFill,
        Self::FullClosePartlyClosed,
        Self::FullCloseClosed,
        Self::Insolvent,
        Self::Deferred,
        Self::Failed,
    ];

    /// The ABCI attribute value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotEligible => "not_eligible",
            Self::Indeterminate => "indeterminate",
            Self::RestoredByCancellation => "restored_by_cancellation",
            Self::NoFill => "no_fill",
            Self::SafeProgress => "safe_progress",
            Self::MaintenanceRestoredBelowTarget => "maintenance_restored_below_target",
            Self::TargetReached => "target_reached",
            Self::FullCloseRestoredByCancellation => "full_close_restored_by_cancellation",
            Self::FullCloseNoFill => "full_close_no_fill",
            Self::FullClosePartlyClosed => "full_close_partly_closed",
            Self::FullCloseClosed => "full_close_closed",
            Self::Insolvent => "insolvent",
            Self::Deferred => "deferred",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for LiquidationProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
