//! The vault tier a client payload names.

use borsh::{BorshDeserialize, BorshSerialize};

/// A `vault_tier` discriminant outside the defined set, `0` included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownVaultTier {
    /// The byte that did not decode.
    pub discriminant: u8,
}

/// One of the three program-owned custody vaults. Wire starts at `1` — a
/// zeroed account byte is invalid, not `Hot`. `Ord` = release speed, so
/// "promote to slower tier" is well-defined.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub enum VaultTier {
    /// Working float for routine withdrawals. Low quorum, small cap, seconds.
    Hot,
    /// Buffer that refills hot. Larger quorum, medium cap, minutes.
    Warm,
    /// Bulk reserve. Largest quorum, manual release, ≥24 hours.
    Cold,
}

impl VaultTier {
    /// The wire discriminant. Stable: it appears inside signed payloads.
    #[must_use]
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Hot => 1,
            Self::Warm => 2,
            Self::Cold => 3,
        }
    }

    /// Decode a wire discriminant, failing closed on anything unrecognised.
    pub fn from_wire(discriminant: u8) -> Result<Self, UnknownVaultTier> {
        match discriminant {
            1 => Ok(Self::Hot),
            2 => Ok(Self::Warm),
            3 => Ok(Self::Cold),
            _ => Err(UnknownVaultTier { discriminant }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zeroed byte is the shape an uninitialised account has. Making `0` an
    /// invalid tier means a zeroed discriminant cannot silently decode as the
    /// *hot* tier — the smallest quorum and fastest release. Nomad's loss came
    /// from exactly this class: a zero value that decoded as valid.
    #[test]
    fn zero_is_not_a_valid_tier_discriminant() {
        assert_eq!(
            VaultTier::from_wire(0),
            Err(UnknownVaultTier { discriminant: 0 })
        );
    }

    #[test]
    fn tier_wire_encoding_round_trips() {
        for tier in [VaultTier::Hot, VaultTier::Warm, VaultTier::Cold] {
            assert_eq!(
                VaultTier::from_wire(tier.to_wire()).expect("every tier decodes its own wire byte"),
                tier
            );
        }
    }

    #[test]
    fn an_unknown_discriminant_is_rejected() {
        assert_eq!(
            VaultTier::from_wire(4),
            Err(UnknownVaultTier { discriminant: 4 })
        );
        assert_eq!(
            VaultTier::from_wire(u8::MAX),
            Err(UnknownVaultTier {
                discriminant: u8::MAX
            })
        );
    }

    /// Spec §Custody topology: hot releases in seconds, warm in minutes, cold
    /// after at least 24 hours. The ordering is what makes "promote to the
    /// slower tier" well-defined.
    #[test]
    fn tiers_order_from_fastest_to_slowest() {
        assert!(VaultTier::Hot < VaultTier::Warm);
        assert!(VaultTier::Warm < VaultTier::Cold);
    }
}
