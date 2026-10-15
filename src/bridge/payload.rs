//! The signed payloads clients build: [`WithdrawalAuthorizationV1`] and
//! [`BridgeReceiptV1`], with the field types they carry.
//!
//! Rules: domain tags emitted, never supplied; fixed-width hand-written
//! encodings, integers LE; one atomic [`DeploymentId`] per payload. Editing a
//! `V1` is never compatible — new shape = new `V2` + new domain.

use crate::bridge::epoch::RegistryEpoch;
use crate::bridge::hash::Hasher;
use crate::bridge::ids::{ProofOwner, SolanaPubkey, TxSignature, WithdrawalId};
use crate::bridge::tier::VaultTier;
use crate::bridge::units::MicroUsdc;
use borsh::{BorshDeserialize, BorshSerialize};

/// Why a payload could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PayloadError {
    /// The `terminal_state` byte matches no terminal state. `0` is included:
    /// an uninitialised byte is not "not yet terminal", it is invalid.
    UnknownTerminalState,
    /// The `receipt_quorum_kind` byte matches no quorum kind.
    UnknownQuorumKind,
    /// The decoded domain tag is not this payload's.
    WrongDomain,
    /// The `vault_tier` byte matches no tier.
    UnknownVaultTier,
}

/// A Solana slot number. Distinct type from [`UnixSeconds`] — one clock's
/// value must not satisfy the other's check.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Slot(u64);

impl Slot {
    /// Wrap a raw slot number.
    #[must_use]
    pub const fn new(slot: u64) -> Self {
        Self(slot)
    }

    /// The raw slot number, for wire encoding.
    #[must_use]
    pub const fn to_u64(self) -> u64 {
        self.0
    }
}

/// A Solana `Clock.unix_timestamp`: seconds, `i64` as the runtime reports it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct UnixSeconds(i64);

impl UnixSeconds {
    /// Wrap a raw timestamp.
    #[must_use]
    pub const fn new(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The raw timestamp, for wire encoding.
    #[must_use]
    pub const fn to_i64(self) -> i64 {
        self.0
    }

    /// This timestamp advanced by `seconds`, refusing to wrap.
    pub fn checked_add_seconds(self, seconds: i64) -> Option<Self> {
        self.0.checked_add(seconds).map(Self)
    }
}

/// Terminal state a withdrawal reached on Solana. Exactly two, mutually
/// exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum TerminalState {
    /// `execute` moved USDC to the destination.
    Paid,
    /// `cancel_expired` returned the request unpaid after expiry.
    Cancelled,
}

impl TerminalState {
    /// The wire discriminant, per the spec: `1 = Paid`, `2 = Cancelled`.
    #[must_use]
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Paid => 1,
            Self::Cancelled => 2,
        }
    }

    /// Decode a wire discriminant, failing closed on anything else.
    pub fn from_wire(discriminant: u8) -> Result<Self, PayloadError> {
        match discriminant {
            1 => Ok(Self::Paid),
            2 => Ok(Self::Cancelled),
            _ => Err(PayloadError::UnknownTerminalState),
        }
    }
}

/// Which quorum attested a receipt. The configured phase fixes which kind is
/// legal; mismatch fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum ReceiptQuorumKind {
    /// *m* distinct members of the epoch-pinned operator receipt registry.
    OperatorMOfN,
    /// More than two thirds of the epoch's Proof validator stake.
    ValidatorStake,
}

impl ReceiptQuorumKind {
    /// The wire discriminant, per the spec: `1 = operator`, `2 = validator`.
    #[must_use]
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::OperatorMOfN => 1,
            Self::ValidatorStake => 2,
        }
    }

    /// Decode a wire discriminant, failing closed on anything else.
    pub fn from_wire(discriminant: u8) -> Result<Self, PayloadError> {
        match discriminant {
            1 => Ok(Self::OperatorMOfN),
            2 => Ok(Self::ValidatorStake),
            _ => Err(PayloadError::UnknownQuorumKind),
        }
    }
}

/// 32-byte atomic deployment identity. Decoded values are claims: recompute
/// from out-of-band identifiers and compare.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct DeploymentId([u8; 32]);

impl DeploymentId {
    /// Wrap raw bytes decoded off the wire.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw bytes, for hashing and wire encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// What a quorum signs to release one withdrawal. Operator phase: Squads
/// proposal/CPI payload; validator phase: quorum-signed bytes. No reduced
/// forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WithdrawalAuthorizationV1 {
    /// The deployment this authorization is pinned to.
    pub deployment_id: DeploymentId,
    /// Which vault pays.
    pub vault_tier: VaultTier,
    /// The engine's sequential withdrawal id.
    pub withdrawal_id: WithdrawalId,
    /// The debited Proof account.
    pub proof_owner: ProofOwner,
    /// The Solana wallet that owns the destination token account.
    pub destination_owner: SolanaPubkey,
    /// The destination USDC token account, derived from `destination_owner`.
    pub destination_token_acct: SolanaPubkey,
    /// What the destination receives.
    pub amount_micro_usdc: MicroUsdc,
    /// The flat protocol fee, retained in the tier vault.
    pub fee_micro_usdc: MicroUsdc,
    /// The Proof block height at which the debit was recorded.
    pub engine_height: u64,
    /// The signer-registry epoch that authorized this request.
    pub signer_epoch: RegistryEpoch,
    /// Execution may not happen before this slot.
    pub not_before_slot: Slot,
    /// Execution may not happen after this slot.
    pub expires_at_slot: Slot,
    /// Execution may not happen before this wall-clock time.
    pub not_before_unix_seconds: UnixSeconds,
    /// Execution may not happen after this wall-clock time.
    pub expires_at_unix_seconds: UnixSeconds,
}

impl WithdrawalAuthorizationV1 {
    /// `ascii32("bridge.withdrawal.v1")`.
    pub const DOMAIN: [u8; 32] = domains::WITHDRAWAL;

    /// The exact encoded length. Fixed by construction.
    pub const ENCODED_LEN: usize = 221;

    /// The canonical signed bytes. Straight-line writes at const offsets into
    /// a fixed array — no allocation, no cursor, no silent-pad path.
    #[must_use]
    // Const offsets into a const-sized array, `END` compile-checked against
    // `ENCODED_LEN`; a width mismatch panics in every test run.
    #[allow(clippy::indexing_slicing)]
    pub fn encode(&self) -> [u8; Self::ENCODED_LEN] {
        use crate::bridge::payload::layout::withdrawal as L;
        let mut out = [0u8; Self::ENCODED_LEN];
        out[L::DOMAIN..L::DEPLOYMENT_ID].copy_from_slice(&Self::DOMAIN);
        out[L::DEPLOYMENT_ID..L::VAULT_TIER].copy_from_slice(self.deployment_id.as_bytes());
        out[L::VAULT_TIER] = self.vault_tier.to_wire();
        out[L::WITHDRAWAL_ID..L::PROOF_OWNER]
            .copy_from_slice(&self.withdrawal_id.to_u64().to_le_bytes());
        out[L::PROOF_OWNER..L::DESTINATION_OWNER].copy_from_slice(self.proof_owner.as_bytes());
        out[L::DESTINATION_OWNER..L::DESTINATION_TOKEN_ACCT]
            .copy_from_slice(self.destination_owner.as_bytes());
        out[L::DESTINATION_TOKEN_ACCT..L::AMOUNT]
            .copy_from_slice(self.destination_token_acct.as_bytes());
        out[L::AMOUNT..L::FEE].copy_from_slice(&self.amount_micro_usdc.to_micro().to_le_bytes());
        out[L::FEE..L::ENGINE_HEIGHT]
            .copy_from_slice(&self.fee_micro_usdc.to_micro().to_le_bytes());
        out[L::ENGINE_HEIGHT..L::SIGNER_EPOCH].copy_from_slice(&self.engine_height.to_le_bytes());
        out[L::SIGNER_EPOCH..L::NOT_BEFORE_SLOT]
            .copy_from_slice(&self.signer_epoch.to_u64().to_le_bytes());
        out[L::NOT_BEFORE_SLOT..L::EXPIRES_AT_SLOT]
            .copy_from_slice(&self.not_before_slot.to_u64().to_le_bytes());
        out[L::EXPIRES_AT_SLOT..L::NOT_BEFORE_UNIX]
            .copy_from_slice(&self.expires_at_slot.to_u64().to_le_bytes());
        out[L::NOT_BEFORE_UNIX..L::EXPIRES_AT_UNIX]
            .copy_from_slice(&self.not_before_unix_seconds.to_i64().to_le_bytes());
        out[L::EXPIRES_AT_UNIX..L::END]
            .copy_from_slice(&self.expires_at_unix_seconds.to_i64().to_le_bytes());
        out
    }

    /// Decode the exact signed bytes — [`Self::encode`]'s inverse.
    // Const offsets, same layout proof as `encode`.
    #[allow(clippy::indexing_slicing)]
    pub fn decode(bytes: &[u8; Self::ENCODED_LEN]) -> Result<Self, PayloadError> {
        use crate::bridge::payload::layout::withdrawal as L;
        if bytes[L::DOMAIN..L::DEPLOYMENT_ID] != Self::DOMAIN {
            return Err(PayloadError::WrongDomain);
        }
        let vault_tier = VaultTier::from_wire(bytes[L::VAULT_TIER])
            .map_err(|_| PayloadError::UnknownVaultTier)?;
        Ok(Self {
            deployment_id: DeploymentId::new(read_array(bytes, L::DEPLOYMENT_ID)),
            vault_tier,
            withdrawal_id: WithdrawalId::new(read_u64(bytes, L::WITHDRAWAL_ID)),
            proof_owner: ProofOwner::new(read_array(bytes, L::PROOF_OWNER)),
            destination_owner: SolanaPubkey::new(read_array(bytes, L::DESTINATION_OWNER)),
            destination_token_acct: SolanaPubkey::new(read_array(bytes, L::DESTINATION_TOKEN_ACCT)),
            amount_micro_usdc: MicroUsdc::new(read_u64(bytes, L::AMOUNT)),
            fee_micro_usdc: MicroUsdc::new(read_u64(bytes, L::FEE)),
            engine_height: read_u64(bytes, L::ENGINE_HEIGHT),
            signer_epoch: RegistryEpoch::new(read_u64(bytes, L::SIGNER_EPOCH)),
            not_before_slot: Slot::new(read_u64(bytes, L::NOT_BEFORE_SLOT)),
            expires_at_slot: Slot::new(read_u64(bytes, L::EXPIRES_AT_SLOT)),
            not_before_unix_seconds: UnixSeconds::new(read_i64(bytes, L::NOT_BEFORE_UNIX)),
            expires_at_unix_seconds: UnixSeconds::new(read_i64(bytes, L::EXPIRES_AT_UNIX)),
        })
    }

    /// `SHA256(encode())` — the identity a terminal receipt binds to.
    #[must_use]
    pub fn digest<H: Hasher>(&self) -> [u8; 32] {
        H::sha256(&[&self.encode()])
    }
}

/// What a quorum signs to prove a withdrawal reached a terminal state. The
/// positive proof gating the engine's terminal transition — a timeout or RPC
/// error is never failure evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeReceiptV1 {
    /// The deployment this receipt is pinned to.
    pub deployment_id: DeploymentId,
    /// `SHA256(Borsh(WithdrawalAuthorizationV1))` — the complete authorization.
    pub authorization_digest: [u8; 32],
    /// The engine's sequential withdrawal id.
    pub withdrawal_id: WithdrawalId,
    /// Which terminal state was reached.
    pub terminal_state: TerminalState,
    /// The vault that paid, or would have.
    pub vault_tier: VaultTier,
    /// The debited Proof account.
    pub proof_owner: ProofOwner,
    /// The destination wallet.
    pub destination_owner: SolanaPubkey,
    /// The destination token account.
    pub destination_token_acct: SolanaPubkey,
    /// The authorized amount.
    pub amount_micro_usdc: MicroUsdc,
    /// The authorized fee.
    pub fee_micro_usdc: MicroUsdc,
    /// The epoch that authorized the withdrawal.
    pub authorization_signer_epoch: RegistryEpoch,
    /// The finalized transaction that settled it.
    pub solana_tx_signature: TxSignature,
    /// That transaction's containing slot.
    pub finalized_slot: Slot,
    /// That slot's blockhash.
    pub finalized_blockhash: [u8; 32],
    /// Which quorum kind attested this receipt.
    pub receipt_quorum_kind: ReceiptQuorumKind,
    /// The attesting authority's registry epoch.
    pub receipt_authority_epoch: RegistryEpoch,
}

impl BridgeReceiptV1 {
    /// `ascii32("bridge.withdrawal-receipt.v1")`.
    pub const DOMAIN: [u8; 32] = domains::WITHDRAWAL_RECEIPT;

    /// The exact encoded length. Fixed by construction.
    pub const ENCODED_LEN: usize = 327;

    /// The canonical signed bytes — const-offset writes, no allocation.
    #[must_use]
    // Const offsets, `END` compile-checked; width mismatch panics in tests.
    #[allow(clippy::indexing_slicing)]
    pub fn encode(&self) -> [u8; Self::ENCODED_LEN] {
        use crate::bridge::payload::layout::bridge_receipt as L;
        let mut out = [0u8; Self::ENCODED_LEN];
        out[L::DOMAIN..L::DEPLOYMENT_ID].copy_from_slice(&Self::DOMAIN);
        out[L::DEPLOYMENT_ID..L::AUTHORIZATION_DIGEST]
            .copy_from_slice(self.deployment_id.as_bytes());
        out[L::AUTHORIZATION_DIGEST..L::WITHDRAWAL_ID].copy_from_slice(&self.authorization_digest);
        out[L::WITHDRAWAL_ID..L::TERMINAL_STATE]
            .copy_from_slice(&self.withdrawal_id.to_u64().to_le_bytes());
        out[L::TERMINAL_STATE] = self.terminal_state.to_wire();
        out[L::VAULT_TIER] = self.vault_tier.to_wire();
        out[L::PROOF_OWNER..L::DESTINATION_OWNER].copy_from_slice(self.proof_owner.as_bytes());
        out[L::DESTINATION_OWNER..L::DESTINATION_TOKEN_ACCT]
            .copy_from_slice(self.destination_owner.as_bytes());
        out[L::DESTINATION_TOKEN_ACCT..L::AMOUNT]
            .copy_from_slice(self.destination_token_acct.as_bytes());
        out[L::AMOUNT..L::FEE].copy_from_slice(&self.amount_micro_usdc.to_micro().to_le_bytes());
        out[L::FEE..L::AUTHORIZATION_SIGNER_EPOCH]
            .copy_from_slice(&self.fee_micro_usdc.to_micro().to_le_bytes());
        out[L::AUTHORIZATION_SIGNER_EPOCH..L::TX_SIGNATURE]
            .copy_from_slice(&self.authorization_signer_epoch.to_u64().to_le_bytes());
        out[L::TX_SIGNATURE..L::FINALIZED_SLOT]
            .copy_from_slice(self.solana_tx_signature.as_bytes());
        out[L::FINALIZED_SLOT..L::FINALIZED_BLOCKHASH]
            .copy_from_slice(&self.finalized_slot.to_u64().to_le_bytes());
        out[L::FINALIZED_BLOCKHASH..L::QUORUM_KIND].copy_from_slice(&self.finalized_blockhash);
        out[L::QUORUM_KIND] = self.receipt_quorum_kind.to_wire();
        out[L::AUTHORITY_EPOCH..L::END]
            .copy_from_slice(&self.receipt_authority_epoch.to_u64().to_le_bytes());
        out
    }

    /// `SHA256(encode())`.
    #[must_use]
    pub fn digest<H: Hasher>(&self) -> [u8; 32] {
        H::sha256(&[&self.encode()])
    }
}

/// Byte offsets of every payload field: each is the previous offset plus the
/// previous field's width, and each `END` is compile-checked against
/// `ENCODED_LEN` — a drifted offset is a build error or a test-run panic,
/// never a silently misaligned encoding.
// Fixed-offset reads for decoders. Bounds hold by the `layout` end asserts;
// the double slice avoids offset arithmetic entirely.
#[allow(clippy::indexing_slicing)]
fn read_array<const N: usize>(bytes: &[u8], at: usize) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[at..][..N]);
    out
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(read_array(bytes, at))
}

fn read_i64(bytes: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(read_array(bytes, at))
}

mod layout {
    /// `WithdrawalAuthorizationV1`.
    pub mod withdrawal {
        pub const DOMAIN: usize = 0;
        pub const DEPLOYMENT_ID: usize = DOMAIN + 32;
        pub const VAULT_TIER: usize = DEPLOYMENT_ID + 32;
        pub const WITHDRAWAL_ID: usize = VAULT_TIER + 1;
        pub const PROOF_OWNER: usize = WITHDRAWAL_ID + 8;
        pub const DESTINATION_OWNER: usize = PROOF_OWNER + 20;
        pub const DESTINATION_TOKEN_ACCT: usize = DESTINATION_OWNER + 32;
        pub const AMOUNT: usize = DESTINATION_TOKEN_ACCT + 32;
        pub const FEE: usize = AMOUNT + 8;
        pub const ENGINE_HEIGHT: usize = FEE + 8;
        pub const SIGNER_EPOCH: usize = ENGINE_HEIGHT + 8;
        pub const NOT_BEFORE_SLOT: usize = SIGNER_EPOCH + 8;
        pub const EXPIRES_AT_SLOT: usize = NOT_BEFORE_SLOT + 8;
        pub const NOT_BEFORE_UNIX: usize = EXPIRES_AT_SLOT + 8;
        pub const EXPIRES_AT_UNIX: usize = NOT_BEFORE_UNIX + 8;
        pub const END: usize = EXPIRES_AT_UNIX + 8;
    }

    /// `BridgeReceiptV1`.
    pub mod bridge_receipt {
        pub const DOMAIN: usize = 0;
        pub const DEPLOYMENT_ID: usize = DOMAIN + 32;
        pub const AUTHORIZATION_DIGEST: usize = DEPLOYMENT_ID + 32;
        pub const WITHDRAWAL_ID: usize = AUTHORIZATION_DIGEST + 32;
        pub const TERMINAL_STATE: usize = WITHDRAWAL_ID + 8;
        pub const VAULT_TIER: usize = TERMINAL_STATE + 1;
        pub const PROOF_OWNER: usize = VAULT_TIER + 1;
        pub const DESTINATION_OWNER: usize = PROOF_OWNER + 20;
        pub const DESTINATION_TOKEN_ACCT: usize = DESTINATION_OWNER + 32;
        pub const AMOUNT: usize = DESTINATION_TOKEN_ACCT + 32;
        pub const FEE: usize = AMOUNT + 8;
        pub const AUTHORIZATION_SIGNER_EPOCH: usize = FEE + 8;
        pub const TX_SIGNATURE: usize = AUTHORIZATION_SIGNER_EPOCH + 8;
        pub const FINALIZED_SLOT: usize = TX_SIGNATURE + 64;
        pub const FINALIZED_BLOCKHASH: usize = FINALIZED_SLOT + 8;
        pub const QUORUM_KIND: usize = FINALIZED_BLOCKHASH + 32;
        pub const AUTHORITY_EPOCH: usize = QUORUM_KIND + 1;
        pub const END: usize = AUTHORITY_EPOCH + 8;
    }
}

const _: () = assert!(layout::withdrawal::END == WithdrawalAuthorizationV1::ENCODED_LEN);
const _: () = assert!(layout::bridge_receipt::END == BridgeReceiptV1::ENCODED_LEN);

/// Domain tags: literal ASCII names zero-padded to 32 bytes, not
/// `SHA256(name)` — legible on the wire, and the source string *is* the
/// constant.
pub mod domains {
    use super::ascii_domain;

    /// `"bridge.withdrawal.v1"`, zero-padded.
    pub const WITHDRAWAL: [u8; 32] = ascii_domain(b"bridge.withdrawal.v1");

    /// `"bridge.withdrawal-receipt.v1"`, zero-padded — the longest tag, with
    /// four bytes spare.
    pub const WITHDRAWAL_RECEIPT: [u8; 32] = ascii_domain(b"bridge.withdrawal-receipt.v1");
}

/// Longest tag = full field width — the only mechanically meaningful bound
/// (past it, truncation collides prefixed tags). Headroom is bought with
/// short names, not a narrower limit.
const MAX_TAG_LEN: usize = 32;

/// ASCII name zero-padded to 32 bytes. Const-evaluated — an over-long tag is
/// a compile error, never a truncation.
//
// Indexing/increment are const-evaluated with the bound asserted above;
// narrow allow, not crate-wide.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn ascii_domain(tag: &[u8]) -> [u8; 32] {
    assert!(
        tag.len() <= MAX_TAG_LEN,
        "domain tag exceeds the 32-byte field and would be truncated into a \
         collision with any tag sharing its prefix"
    );

    let mut padded = [0u8; 32];
    let mut index = 0;
    while index < tag.len() {
        padded[index] = tag[index];
        index += 1;
    }
    padded
}

#[cfg(all(test, feature = "rust-crypto"))]
mod tests {
    use super::*;
    use crate::bridge::hash::RustCrypto;

    /// The expected domain encoding, written independently of the
    /// implementation so the test does not just restate it.
    fn zero_padded(tag: &[u8]) -> [u8; 32] {
        let mut expected = [0u8; 32];
        expected
            .get_mut(..tag.len())
            .expect("a domain tag fits its 32-byte field")
            .copy_from_slice(tag);
        expected
    }

    fn deployment_id() -> DeploymentId {
        DeploymentId::new([0xD1; 32])
    }

    fn authorization() -> WithdrawalAuthorizationV1 {
        WithdrawalAuthorizationV1 {
            deployment_id: deployment_id(),
            vault_tier: VaultTier::Hot,
            withdrawal_id: WithdrawalId::new(7),
            proof_owner: ProofOwner::new([5u8; 20]),
            destination_owner: SolanaPubkey::new([6u8; 32]),
            destination_token_acct: SolanaPubkey::new([7u8; 32]),
            amount_micro_usdc: MicroUsdc::new(1_000_000),
            fee_micro_usdc: MicroUsdc::new(1_000_000),
            engine_height: 42,
            signer_epoch: RegistryEpoch::new(3),
            not_before_slot: Slot::new(100),
            expires_at_slot: Slot::new(200),
            not_before_unix_seconds: UnixSeconds::new(1_000),
            expires_at_unix_seconds: UnixSeconds::new(2_000),
        }
    }

    fn bridge_receipt() -> BridgeReceiptV1 {
        BridgeReceiptV1 {
            deployment_id: deployment_id(),
            authorization_digest: authorization().digest::<RustCrypto>(),
            withdrawal_id: WithdrawalId::new(7),
            terminal_state: TerminalState::Paid,
            vault_tier: VaultTier::Hot,
            proof_owner: ProofOwner::new([5u8; 20]),
            destination_owner: SolanaPubkey::new([6u8; 32]),
            destination_token_acct: SolanaPubkey::new([7u8; 32]),
            amount_micro_usdc: MicroUsdc::new(1_000_000),
            fee_micro_usdc: MicroUsdc::new(1_000_000),
            authorization_signer_epoch: RegistryEpoch::new(3),
            solana_tx_signature: TxSignature::new([10u8; 64]),
            finalized_slot: Slot::new(900),
            finalized_blockhash: [11u8; 32],
            receipt_quorum_kind: ReceiptQuorumKind::OperatorMOfN,
            receipt_authority_epoch: RegistryEpoch::new(3),
        }
    }

    // ---- decode ------------------------------------------------------------

    /// The program receives the exact signed bytes off the wire; decode must
    /// invert encode bit-for-bit.
    #[test]
    fn withdrawal_decode_inverts_encode() {
        let payload = authorization();
        assert_eq!(
            WithdrawalAuthorizationV1::decode(&payload.encode()),
            Ok(payload)
        );
    }

    #[test]
    fn withdrawal_decode_refuses_a_wrong_domain() {
        let mut bytes = authorization().encode();
        bytes[0] ^= 1;
        assert_eq!(
            WithdrawalAuthorizationV1::decode(&bytes),
            Err(PayloadError::WrongDomain)
        );
    }

    #[test]
    fn withdrawal_decode_refuses_an_unknown_tier_byte() {
        let mut bytes = authorization().encode();
        bytes[layout::withdrawal::VAULT_TIER] = 9;
        assert!(WithdrawalAuthorizationV1::decode(&bytes).is_err());
    }

    #[test]
    fn each_payload_leads_with_its_own_readable_domain_tag() {
        assert_eq!(
            authorization()
                .encode()
                .get(..32)
                .expect("an encoding holds its 32-byte domain"),
            &zero_padded(b"bridge.withdrawal.v1")
        );
        assert_eq!(
            bridge_receipt()
                .encode()
                .get(..32)
                .expect("an encoding holds its 32-byte domain"),
            &zero_padded(b"bridge.withdrawal-receipt.v1")
        );
    }

    #[test]
    fn payload_encodings_are_fixed_width() {
        assert_eq!(authorization().encode().len(), 221);
        assert_eq!(bridge_receipt().encode().len(), 327);
    }

    // ---- Digest binding ----------------------------------------------------

    /// Spec: `authorization_digest = SHA256(Borsh(WithdrawalAuthorizationV1))`.
    #[test]
    fn the_authorization_digest_is_sha256_of_its_encoding() {
        let auth = authorization();
        assert_eq!(
            auth.digest::<RustCrypto>(),
            RustCrypto::sha256(&[&auth.encode()])
        );
    }

    /// Every field is signed. Spec §Domain separation and expiry: "the same
    /// signature fails on a different Proof genesis, Solana cluster, program
    /// id, mint, destination, amount, signer epoch or after `expires_at_slot`."
    /// This is the test that keeps a compromised or confused signer from
    /// replaying one approval into a different withdrawal.
    #[test]
    fn every_field_is_covered_by_the_digest() {
        let base = authorization().digest::<RustCrypto>();

        let mutations: Vec<(&str, WithdrawalAuthorizationV1)> = vec![
            // The four deployment identifiers are covered transitively:
            // `every_identifier_changes_the_deployment_id` proves each changes
            // the id, and this mutation proves the id changes the digest.
            ("deployment id", {
                let mut a = authorization();
                a.deployment_id = DeploymentId::new([0xAA; 32]);
                a
            }),
            ("tier", {
                let mut a = authorization();
                a.vault_tier = VaultTier::Cold;
                a
            }),
            ("withdrawal id", {
                let mut a = authorization();
                a.withdrawal_id = WithdrawalId::new(8);
                a
            }),
            ("owner", {
                let mut a = authorization();
                a.proof_owner = ProofOwner::new([0xAA; 20]);
                a
            }),
            ("destination owner", {
                let mut a = authorization();
                a.destination_owner = SolanaPubkey::new([0xAA; 32]);
                a
            }),
            ("destination token account", {
                let mut a = authorization();
                a.destination_token_acct = SolanaPubkey::new([0xAA; 32]);
                a
            }),
            ("amount", {
                let mut a = authorization();
                a.amount_micro_usdc = MicroUsdc::new(2_000_000);
                a
            }),
            ("fee", {
                let mut a = authorization();
                a.fee_micro_usdc = MicroUsdc::new(2_000_000);
                a
            }),
            ("engine height", {
                let mut a = authorization();
                a.engine_height = 43;
                a
            }),
            ("signer epoch", {
                let mut a = authorization();
                a.signer_epoch = RegistryEpoch::new(4);
                a
            }),
            ("not before slot", {
                let mut a = authorization();
                a.not_before_slot = Slot::new(101);
                a
            }),
            ("expires at slot", {
                let mut a = authorization();
                a.expires_at_slot = Slot::new(201);
                a
            }),
            ("not before unix", {
                let mut a = authorization();
                a.not_before_unix_seconds = UnixSeconds::new(1_001);
                a
            }),
            ("expires at unix", {
                let mut a = authorization();
                a.expires_at_unix_seconds = UnixSeconds::new(2_001);
                a
            }),
        ];

        for (field, mutated) in mutations {
            assert_ne!(
                mutated.digest::<RustCrypto>(),
                base,
                "changing the {field} left the digest unchanged, so a signature \
                 over one value would authorize another"
            );
        }
    }

    /// The terminal receipt binds to the *complete* authorization by digest,
    /// not merely to the withdrawal id — so a `Paid` receipt cannot be
    /// harvested from one authorization and presented against another that
    /// happens to share an id across a rotation.
    #[test]
    fn a_bridge_receipt_binds_the_whole_authorization_not_just_its_id() {
        let mut other_auth = authorization();
        other_auth.destination_owner = SolanaPubkey::new([0xAA; 32]);

        let mut receipt = bridge_receipt();
        assert_eq!(receipt.withdrawal_id, other_auth.withdrawal_id);

        receipt.authorization_digest = other_auth.digest::<RustCrypto>();
        assert_ne!(receipt.encode(), bridge_receipt().encode());
    }

    // ---- Terminal state ----------------------------------------------------

    /// Spec: `terminal_state` is `1 = Paid`, `2 = Cancelled`. Zero is not a
    /// terminal state — an uninitialised byte must not decode as one.
    #[test]
    fn terminal_state_wire_values_follow_the_spec() {
        assert_eq!(TerminalState::Paid.to_wire(), 1);
        assert_eq!(TerminalState::Cancelled.to_wire(), 2);
        assert_eq!(
            TerminalState::from_wire(0),
            Err(PayloadError::UnknownTerminalState)
        );
        assert_eq!(
            TerminalState::from_wire(3),
            Err(PayloadError::UnknownTerminalState)
        );
    }

    /// Frozen V1 vectors: a V1 payload never changes in place, so these bytes
    /// never change. Not in `vectors/`, which holds signed transactions only.
    /// Regenerate only for a new V2, by printing `hex::encode(payload.encode())`
    /// for the fixture.
    #[test]
    fn bridge_payload_wire_vectors() {
        let authorization_hex = [
            "6272696467652e7769746864726177616c2e7631000000000000000000000000",
            "d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1",
            "0107000000000000000505050505050505050505050505050505050505060606",
            "0606060606060606060606060606060606060606060606060606060606070707",
            "070707070707070707070707070707070707070707070707070707070740420f",
            "000000000040420f00000000002a000000000000000300000000000000640000",
            "0000000000c800000000000000e803000000000000d007000000000000",
        ]
        .concat();
        let receipt_hex = [
            "6272696467652e7769746864726177616c2d726563656970742e763100000000",
            "d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1",
            "58421cdfd5675375bfeaad67d7c02e32672202c28bb85897ae90ab96cffa2014",
            "0700000000000000010105050505050505050505050505050505050505050606",
            "0606060606060606060606060606060606060606060606060606060606060707",
            "0707070707070707070707070707070707070707070707070707070707074042",
            "0f000000000040420f000000000003000000000000000a0a0a0a0a0a0a0a0a0a",
            "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
            "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a84030000000000000b0b",
            "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0103",
            "00000000000000",
        ]
        .concat();

        assert_eq!(hex::encode(authorization().encode()), authorization_hex);
        assert_eq!(hex::encode(bridge_receipt().encode()), receipt_hex);
    }

    #[test]
    fn terminal_state_round_trips() {
        for state in [TerminalState::Paid, TerminalState::Cancelled] {
            assert_eq!(
                TerminalState::from_wire(state.to_wire())
                    .expect("every state decodes its own wire byte"),
                state
            );
        }
    }

    /// Spec: `receipt_quorum_kind` is `1 = operator m-of-n`, `2 = validator
    /// stake`. The selected bridge mode fixes which is legal; a zero byte is
    /// neither.
    #[test]
    fn receipt_quorum_kind_wire_values_follow_the_spec() {
        assert_eq!(ReceiptQuorumKind::OperatorMOfN.to_wire(), 1);
        assert_eq!(ReceiptQuorumKind::ValidatorStake.to_wire(), 2);
        assert_eq!(
            ReceiptQuorumKind::from_wire(0),
            Err(PayloadError::UnknownQuorumKind)
        );
    }
}
