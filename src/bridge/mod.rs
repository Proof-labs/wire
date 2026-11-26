//! The bridge payloads clients sign (withdrawal authorization and receipt) and
//! the field types they carry. `bridge-core` depends on this module, so it is
//! self-contained: it builds with the `bridge` feature alone.
//!
//! The identity, epoch and unit newtypes also build under `full` (with serde
//! instead of Borsh), so the engine's bridge actions, records and events
//! carry the same types the payloads do; the Borsh payloads need `bridge`.

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
#[cfg(feature = "bridge")]
pub mod payload;
#[cfg(feature = "bridge")]
pub mod tier;
pub mod units;

pub use epoch::{EpochError, RegistryEpoch};
pub use hash::Hasher;
#[cfg(feature = "rust-crypto")]
pub use hash::RustCrypto;
pub use ids::{
    DeploymentId, OperatorKey, OperatorSignature, ProofOwner, SolanaPubkey, TxSignature,
    WithdrawalId,
};
#[cfg(feature = "bridge")]
pub use payload::{
    BridgeReceiptV1, PayloadError, ReceiptQuorumKind, Slot, TerminalState, UnixSeconds,
    WithdrawalAuthorizationV1,
};
#[cfg(feature = "bridge")]
pub use tier::{UnknownVaultTier, VaultTier};
pub use units::{MicroUsdc, UnitsError};
