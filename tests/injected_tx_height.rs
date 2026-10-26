//! A proposer-injected action carries the height of the block it is proposed
//! for as its envelope `seq`. The same batch in two blocks therefore has two
//! sets of bytes and two transaction hashes (CometBFT hashes a transaction as
//! the SHA-256 of its bytes), and the unstamped `seq = 0` form is no longer
//! canonical.

#![allow(clippy::expect_used, clippy::panic)]

use proof_wire::codec::{
    canonical_action_type, decode_tx, encode_execute_triggers_tx, encode_liquidate_accounts_tx,
    encode_signed_tx, ActionType,
};
use proof_wire::types::{
    Action, BlockHeight, ExecuteTriggers, LiquidateAccounts, OwnerAddress, TriggerRef,
};
use sha2::{Digest, Sha256};

const OWNER: [u8; 20] = [0x77; 20];

fn height(height: u64) -> BlockHeight {
    BlockHeight::new(height).expect("a block height is never zero")
}

fn trigger() -> TriggerRef {
    TriggerRef {
        owner: OwnerAddress::new(OWNER),
        market: 1,
    }
}

#[test]
fn the_same_owner_set_in_two_blocks_has_two_hashes() {
    let first = encode_liquidate_accounts_tx(vec![OWNER], height(7_267_207)).expect("encodes");
    let second = encode_liquidate_accounts_tx(vec![OWNER], height(7_269_687)).expect("encodes");

    assert_ne!(Sha256::digest(&first), Sha256::digest(&second));
    assert_eq!(decode_tx(&first).expect("decodes").seq, 7_267_207);
    assert_eq!(decode_tx(&second).expect("decodes").seq, 7_269_687);
    assert_eq!(
        canonical_action_type(&second),
        Some(ActionType::LiquidateAccounts)
    );
}

#[test]
fn the_same_trigger_batch_in_two_blocks_has_two_hashes() {
    let first = encode_execute_triggers_tx(vec![trigger()], height(100)).expect("encodes");
    let second = encode_execute_triggers_tx(vec![trigger()], height(101)).expect("encodes");

    assert_ne!(Sha256::digest(&first), Sha256::digest(&second));
    assert_eq!(
        canonical_action_type(&second),
        Some(ActionType::ExecuteTriggers)
    );
}

#[test]
fn an_unstamped_injected_action_is_not_canonical() {
    let liquidation = encode_signed_tx(
        &Action::LiquidateAccounts(LiquidateAccounts {
            owners: vec![OWNER],
        }),
        0,
        &[0; 32],
        &[0; 64],
    )
    .expect("encodes");
    let triggers = encode_signed_tx(
        &Action::ExecuteTriggers(ExecuteTriggers {
            triggers: vec![trigger()],
        }),
        0,
        &[0; 32],
        &[0; 64],
    )
    .expect("encodes");

    assert!(decode_tx(&liquidation).is_ok(), "old bytes still decode");
    assert_eq!(canonical_action_type(&liquidation), None);
    assert_eq!(canonical_action_type(&triggers), None);
}

#[test]
fn zero_is_not_a_block_height() {
    assert_eq!(BlockHeight::new(0), None);
    assert_eq!(height(1).get(), 1);
}
