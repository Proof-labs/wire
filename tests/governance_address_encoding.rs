//! `SignerAddress` and `AccountAddress` encode through `wire_bytes`: the
//! MessagePack bytes equal the bare `[u8; 20]` they replaced, and a
//! human-readable reader still accepts the sequence-of-u8 form.

#![allow(clippy::expect_used)]

use proof_wire::types::{AccountAddress, SignerAddress};
use serde_test::{assert_ser_tokens, Configure, Token};

const ADDRESS: [u8; 20] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe,
    0x00, 0x7f, 0x80, 0xff,
];

#[test]
fn messagepack_bytes_equal_the_bare_array() {
    let bare = rmp_serde::to_vec(&ADDRESS).expect("bare array encodes");
    assert_eq!(
        rmp_serde::to_vec(&SignerAddress(ADDRESS)).expect("signer encodes"),
        bare
    );
    assert_eq!(
        rmp_serde::to_vec(&AccountAddress(ADDRESS)).expect("account encodes"),
        bare
    );
}

#[test]
fn messagepack_round_trips() {
    let bytes = rmp_serde::to_vec(&ADDRESS).expect("bare array encodes");
    let signer: SignerAddress = rmp_serde::from_slice(&bytes).expect("signer decodes");
    let account: AccountAddress = rmp_serde::from_slice(&bytes).expect("account decodes");
    assert_eq!(signer, SignerAddress(ADDRESS));
    assert_eq!(account, AccountAddress(ADDRESS));
}

#[test]
fn a_human_readable_writer_emits_bytes() {
    assert_ser_tokens(
        &SignerAddress(ADDRESS).readable(),
        &[
            Token::NewtypeStruct {
                name: "SignerAddress",
            },
            Token::Bytes(&ADDRESS),
        ],
    );
    assert_ser_tokens(
        &AccountAddress(ADDRESS).readable(),
        &[
            Token::NewtypeStruct {
                name: "AccountAddress",
            },
            Token::Bytes(&ADDRESS),
        ],
    );
}

#[test]
fn a_human_readable_reader_accepts_the_number_list() {
    let list = serde_json::to_string(&ADDRESS.to_vec()).expect("list encodes");
    let signer: SignerAddress = serde_json::from_str(&list).expect("signer decodes");
    let account: AccountAddress = serde_json::from_str(&list).expect("account decodes");
    assert_eq!(signer, SignerAddress(ADDRESS));
    assert_eq!(account, AccountAddress(ADDRESS));
}

#[test]
fn a_wrong_length_is_refused() {
    let short = serde_json::to_string(&ADDRESS[..19].to_vec()).expect("list encodes");
    assert!(serde_json::from_str::<SignerAddress>(&short).is_err());
}
