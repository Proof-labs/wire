//! Identity newtypes the client payloads carry — distinct types per identity
//! so substitution fails to compile.

use crate::bridge::hash::Hasher;
#[cfg(feature = "bridge")]
use borsh::{BorshDeserialize, BorshSerialize};

/// Declare a fixed-width byte-array newtype with the shared accessors.
///
/// Borsh (the `bridge` feature) writes the raw bytes. Serde (the `full`
/// feature) goes through [`crate::wire_bytes`]: a MessagePack `seq` of `u8`,
/// byte-identical to the bare `[u8; N]` it replaces, and a byte string in
/// human-readable formats. `Display` is lowercase hex, as the ABCI event
/// attributes write a bare byte array.
macro_rules! byte_newtype {
    ($(#[$meta:meta])* $name:ident, $len:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "bridge", derive(BorshSerialize, BorshDeserialize))]
        #[cfg_attr(
            feature = "full",
            derive(serde::Serialize, serde::Deserialize),
            serde(transparent)
        )]
        pub struct $name(
            #[cfg_attr(feature = "full", serde(with = "crate::wire_bytes"))] [u8; $len],
        );

        impl $name {
            /// Wrap raw bytes.
            #[must_use]
            pub const fn new(bytes: [u8; $len]) -> Self {
                Self(bytes)
            }

            /// The raw bytes, for hashing and wire encoding.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $len] {
                &self.0
            }
        }

        impl From<[u8; $len]> for $name {
            fn from(bytes: [u8; $len]) -> Self {
                Self(bytes)
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                for byte in self.0 {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

byte_newtype!(
    /// A Solana account address — wallet, mint, program id or PDA. No curve
    /// assertion: deposit authorities are deliberately off-curve.
    SolanaPubkey,
    32
);

byte_newtype!(
    /// A Solana transaction signature. Not a deposit identity — one
    /// transaction may carry several transfers.
    TxSignature,
    64
);

byte_newtype!(
    /// A bridge operator's ed25519 verification key: the operator's only
    /// identity in the operator receipt registry. It signs the operator's
    /// votes, pause votes and payout records.
    OperatorKey,
    32
);

byte_newtype!(
    /// A 64-byte ed25519 signature by an [`OperatorKey`] over one bridge
    /// message (a vote, a pause vote or a payout record).
    OperatorSignature,
    64
);

byte_newtype!(
    /// 32-byte atomic deployment identity. Decoded values are claims:
    /// recompute from out-of-band identifiers and compare.
    DeploymentId,
    32
);

byte_newtype!(
    /// A Proof account identifier: `keccak256(solana_pubkey)[12..]`.
    ProofOwner,
    20
);

impl ProofOwner {
    /// `keccak256(pubkey)[12..]` — the engine's existing mapping, syscall-
    /// recomputable on-chain.
    #[must_use]
    pub fn from_solana_pubkey<H: Hasher>(pubkey: SolanaPubkey) -> Self {
        let digest = H::keccak256(&[pubkey.as_bytes()]);
        debug_assert_eq!(digest.len(), 32, "keccak256 must produce 32 bytes");
        let mut owner = [0u8; 20];
        // Take the low 20 bytes by iteration rather than by slicing: `zip`
        // stops at 20 by construction, so there is no index that could panic
        // if the digest width ever changed under us.
        for (dst, src) in owner.iter_mut().zip(digest.iter().skip(12)) {
            *dst = *src;
        }
        Self(owner)
    }
}

/// Engine's monotonic withdrawal sequence number. Sequence half only — the
/// cross-deployment identity needs `proof_genesis_hash` too. Serde-transparent:
/// it encodes as the bare `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "bridge", derive(BorshSerialize, BorshDeserialize))]
#[cfg_attr(
    feature = "full",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct WithdrawalId(u64);

impl WithdrawalId {
    /// Wrap a raw sequence number.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw sequence number, for wire encoding.
    #[must_use]
    pub const fn to_u64(self) -> u64 {
        self.0
    }
}

impl core::fmt::Display for WithdrawalId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(all(test, feature = "rust-crypto"))]
mod tests {
    use super::*;
    use crate::bridge::hash::RustCrypto;

    /// Spec §Current state: `owner = keccak256(solana_pubkey)[12..]`,
    /// recomputable through a Solana syscall.
    #[test]
    fn proof_owner_is_the_low_twenty_bytes_of_keccak256_of_the_pubkey() {
        let pubkey = SolanaPubkey::new([7u8; 32]);
        let owner = ProofOwner::from_solana_pubkey::<RustCrypto>(pubkey);
        let digest = RustCrypto::keccak256(&[&[7u8; 32]]);

        assert_eq!(owner.as_bytes(), &digest[12..32]);
    }

    #[test]
    fn distinct_pubkeys_give_distinct_owners() {
        let a = ProofOwner::from_solana_pubkey::<RustCrypto>(SolanaPubkey::new([1u8; 32]));
        let b = ProofOwner::from_solana_pubkey::<RustCrypto>(SolanaPubkey::new([2u8; 32]));
        assert_ne!(a, b);
    }

    #[test]
    fn withdrawal_ids_order_by_their_sequence_number() {
        assert!(WithdrawalId::new(1) < WithdrawalId::new(2));
    }
}
