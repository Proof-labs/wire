//! The bridge payloads clients sign (withdrawal authorization and receipt) and
//! the field types they carry. `bridge-core` depends on this module, so it is
//! self-contained: it builds with the `bridge` feature alone.

// bridge-core's lint set: these bytes are decoded on chain, where a panic aborts
// the transaction. Tests may expect, panic and index.
#![deny(clippy::indexing_slicing, missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)
)]

pub mod epoch;
pub mod hash;
pub mod ids;
pub mod payload;
pub mod tier;
pub mod units;

pub use epoch::{EpochError, RegistryEpoch};
pub use hash::Hasher;
#[cfg(feature = "rust-crypto")]
pub use hash::RustCrypto;
pub use ids::{ProofOwner, SolanaPubkey, TxSignature, WithdrawalId};
pub use payload::{
    BridgeReceiptV1, DeploymentId, PayloadError, ReceiptQuorumKind, Slot, TerminalState,
    UnixSeconds, WithdrawalAuthorizationV1,
};
pub use tier::{UnknownVaultTier, VaultTier};
pub use units::{MicroUsdc, UnitsError};
