//! `proof-wire`: the single definition of the Proof exchange wire contract.
//!
//! MessagePack codec, Ed25519 signing preimage, action + wire types, the
//! `ExecError` code table, and the trigger-policy wire payloads. Both
//! `exchange-core` (the engine) and `proof-trading-sdk` (the client SDK)
//! depend on this crate, so the bytes are identical by construction: the
//! contract cannot drift between engine and SDK.
//!
//! Deliberately dep-light and wasm/pyo3-clean: no `getrandom`. Engine-only
//! concerns (state store, matching, liquidation, bridge-receipt verification)
//! stay in `exchange-core`.

#[cfg(feature = "full")]
pub mod abci_event;
#[cfg(feature = "bridge")]
pub mod bridge;
#[cfg(feature = "full")]
pub mod codec;
#[cfg(feature = "full")]
pub mod crypto;
#[cfg(feature = "full")]
pub mod query;
#[cfg(feature = "full")]
pub mod triggers;
#[cfg(feature = "full")]
pub mod types;
#[cfg(feature = "full")]
pub mod wire_bytes;
