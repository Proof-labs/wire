//! Registry epochs — versioning for every signer set. Spec §Signer-registry.

use borsh::{BorshDeserialize, BorshSerialize};

/// Why an epoch transition was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EpochError {
    /// The candidate epoch is not exactly one greater than the current one.
    NotConsecutive,
    /// The epoch space is exhausted; no successor exists.
    Exhausted,
}

/// Monotonic signer-registry version. No decrement exists — a rollback is a
/// new forward epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct RegistryEpoch(u64);

impl RegistryEpoch {
    /// The epoch committed by the deployment manifest at `initialize`.
    pub const GENESIS: Self = Self(0);

    /// Wrap a raw epoch number, e.g. when decoding a signed payload.
    #[must_use]
    pub const fn new(epoch: u64) -> Self {
        Self(epoch)
    }

    /// The raw epoch number, for wire encoding.
    #[must_use]
    pub const fn to_u64(self) -> u64 {
        self.0
    }

    /// The next epoch: exactly one greater.
    pub fn next(self) -> Result<Self, EpochError> {
        self.0.checked_add(1).map(Self).ok_or(EpochError::Exhausted)
    }

    /// Accept `candidate` only if it is exactly `self + 1`.
    pub fn check_successor(self, candidate: Self) -> Result<(), EpochError> {
        match self.next()? {
            successor if successor == candidate => Ok(()),
            _ => Err(EpochError::NotConsecutive),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec §Signer-registry bootstrap and rotation (5): "Epochs increase by
    /// exactly one."
    #[test]
    fn the_successor_of_an_epoch_is_exactly_one_greater() {
        assert_eq!(
            RegistryEpoch::GENESIS
                .next()
                .expect("genesis has a successor"),
            RegistryEpoch::new(1)
        );
        assert_eq!(
            RegistryEpoch::new(41)
                .next()
                .expect("epoch 41 has a successor"),
            RegistryEpoch::new(42)
        );
    }

    #[test]
    fn genesis_is_epoch_zero() {
        assert_eq!(RegistryEpoch::GENESIS, RegistryEpoch::new(0));
    }

    /// Rotation is the only way to move epochs, and it may only move forward
    /// by one. A skip is a rejected transition, not a fast-forward: skipping
    /// would let a compromised set install itself past a pending obligation
    /// still bound to the intervening epoch.
    #[test]
    fn a_skipped_epoch_is_not_a_valid_successor() {
        assert_eq!(
            RegistryEpoch::new(1).check_successor(RegistryEpoch::new(3)),
            Err(EpochError::NotConsecutive)
        );
    }

    #[test]
    fn a_backwards_epoch_is_not_a_valid_successor() {
        assert_eq!(
            RegistryEpoch::new(5).check_successor(RegistryEpoch::new(4)),
            Err(EpochError::NotConsecutive)
        );
        assert_eq!(
            RegistryEpoch::new(5).check_successor(RegistryEpoch::new(5)),
            Err(EpochError::NotConsecutive)
        );
    }

    #[test]
    fn the_consecutive_successor_is_accepted() {
        assert_eq!(
            RegistryEpoch::new(5).check_successor(RegistryEpoch::new(6)),
            Ok(())
        );
    }

    #[test]
    fn the_last_representable_epoch_has_no_successor() {
        assert_eq!(
            RegistryEpoch::new(u64::MAX).next(),
            Err(EpochError::Exhausted)
        );
    }
}
