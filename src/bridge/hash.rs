//! Hashing seam. On-chain:
//! [`hashv`](https://docs.rs/solana-program/latest/solana_program/hash/fn.hashv.html)
//! / [`keccak::hashv`](https://docs.rs/solana-program/latest/solana_program/keccak/fn.hashv.html)
//! syscalls. Off-chain: [`RustCrypto`]. Chunked signature = the syscalls'
//! shape, so on-chain hashes fields in place with no allocation.

/// SHA-256 + Keccak-256 over the concatenation of `chunks`. Chunked and flat
/// calls over the same bytes must agree.
pub trait Hasher {
    /// SHA-256. Payload digests, `deposit_id`, `deployment_id`.
    fn sha256(chunks: &[&[u8]]) -> [u8; 32];

    /// Keccak-256 (pre-NIST padding, **not**
    /// [SHA3-256](https://csrc.nist.gov/pubs/fips/202/final)). Owner
    /// derivation only.
    fn keccak256(chunks: &[&[u8]]) -> [u8; 32];
}

/// Off-chain backend (`sha2` / `sha3`). Never link into the on-chain program —
/// syscall there.
#[cfg(feature = "rust-crypto")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustCrypto;

#[cfg(feature = "rust-crypto")]
impl Hasher for RustCrypto {
    fn sha256(chunks: &[&[u8]]) -> [u8; 32] {
        use sha2::Digest as _;

        let mut hasher = sha2::Sha256::new();
        for chunk in chunks {
            hasher.update(chunk);
        }
        hasher.finalize().into()
    }

    fn keccak256(chunks: &[&[u8]]) -> [u8; 32] {
        use sha3::Digest as _;

        let mut hasher = sha3::Keccak256::new();
        for chunk in chunks {
            hasher.update(chunk);
        }
        hasher.finalize().into()
    }
}

#[cfg(all(test, feature = "rust-crypto"))]
mod tests {
    use super::*;

    /// NIST vector: `SHA256("abc")`.
    const SHA256_ABC: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];

    /// `KECCAK256("abc")` — the pre-NIST-padding Keccak used by Solana's
    /// `keccak` syscall and by the engine's owner derivation, *not* SHA3-256.
    const KECCAK256_ABC: [u8; 32] = [
        0x4e, 0x03, 0x65, 0x7a, 0xea, 0x45, 0xa9, 0x4f, 0xc7, 0xd4, 0x7b, 0xa8, 0x26, 0xc8, 0xd6,
        0x67, 0xc0, 0xd1, 0xe6, 0xe3, 0x3a, 0x64, 0xa0, 0x36, 0xec, 0x44, 0xf5, 0x8f, 0xa1, 0x2d,
        0x6c, 0x45,
    ];

    #[test]
    fn rust_crypto_sha256_matches_the_nist_vector() {
        assert_eq!(RustCrypto::sha256(&[b"abc"]), SHA256_ABC);
    }

    #[test]
    fn rust_crypto_keccak256_matches_the_reference_vector() {
        assert_eq!(RustCrypto::keccak256(&[b"abc"]), KECCAK256_ABC);
    }

    /// The multi-chunk shape exists so an on-chain implementation can call
    /// `sol_sha256` directly on borrowed slices instead of allocating a
    /// concatenated buffer. It must hash the concatenation, exactly — if a
    /// chunked call differed from the flat one, the program and the engine
    /// would compute different digests for the same payload.
    #[test]
    fn chunked_hashing_equals_hashing_the_concatenation() {
        assert_eq!(
            RustCrypto::sha256(&[b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]),
            SHA256_ABC
        );
        assert_eq!(
            RustCrypto::sha256(&[b"ab".as_slice(), b"c".as_slice()]),
            SHA256_ABC
        );
        assert_eq!(
            RustCrypto::keccak256(&[b"a".as_slice(), b"bc".as_slice()]),
            KECCAK256_ABC
        );
    }

    #[test]
    fn empty_chunks_do_not_change_the_digest() {
        assert_eq!(
            RustCrypto::sha256(&[b"".as_slice(), b"abc".as_slice(), b"".as_slice()]),
            SHA256_ABC
        );
    }

    #[test]
    fn no_chunks_hashes_the_empty_input() {
        let expected = RustCrypto::sha256(&[b"".as_slice()]);
        assert_eq!(RustCrypto::sha256(&[]), expected);
    }

    /// The two families are distinct algorithms, not interchangeable backends.
    /// Owner derivation is keccak (an Ethereum-shaped 20-byte address);
    /// payload digests are SHA-256 (Solana's native grain). A backend that
    /// wired one to the other would still pass a round-trip test, so pin it.
    #[test]
    fn sha256_and_keccak256_are_not_the_same_function() {
        assert_ne!(
            RustCrypto::sha256(&[b"abc"]),
            RustCrypto::keccak256(&[b"abc"])
        );
    }
}
