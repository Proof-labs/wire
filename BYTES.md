# Byte Allocation Ledger

`BYTES.md` is the allocation ledger for the two numeric namespaces this crate
owns: the **outer action bytes** (byte 2 of the transaction envelope, assigned
by `define_actions!` in [`src/codec.rs`](src/codec.rs)) and the **inner
`AdminAction` tags** (`AdminActionType` in [`src/types.rs`](src/types.rs)).

The rule: **any change that takes a new byte adds its row here in the same
commit.** `codec.rs`'s ledger tests fail if an assigned byte or inner tag is
missing from its table, so the ledger cannot silently drift from the code.

A byte, once shipped on a persistent chain, is permanent: the outer byte routes
a transaction to its handler and the inner tag is committed by
`admin_proposal_content_hash`, so a duplicate or a renumber corrupts consensus.
Bytes `0x00` and `0xFF` are reserved as sentinels (unused / max).

## Action types

Byte 2 of the transaction envelope, assigned by `define_actions!`. Names below
follow the gateway's `ACTION_*` spelling; the engine's identifiers are the bare
CamelCase forms (`PlaceOrder`, not `ACTION_PLACE_ORDER`).

| Byte | Constant                          | Status   |
|------|-----------------------------------|----------|
| 0x01 | `ACTION_PLACE_ORDER`              | shipped  |
| 0x02 | `ACTION_CANCEL_ORDER`             | shipped  |
| 0x03 | `ACTION_ORACLE_UPDATE`            | shipped  |
| 0x04 | `ACTION_MARKET_ORDER`             | shipped  |
| 0x05 | `ACTION_DEPOSIT`                  | shipped  |
| 0x06 | `ACTION_WITHDRAW`                 | shipped  |
| 0x07 | `ACTION_CREATE_MARKET`            | shipped  |
| 0x08 | `ACTION_WITHDRAW_REQUEST`         | shipped  |
| 0x09 | `ACTION_CONFIRM_DEPOSIT`          | shipped  |
| 0x0A | `ACTION_CONFIRM_WITHDRAWAL`       | shipped  |
| 0x0B | `ACTION_FAIL_WITHDRAWAL`          | shipped  |
| 0x0C | `ACTION_APPROVE_AGENT`            | shipped  |
| 0x0D | `ACTION_REVOKE_AGENT`             | shipped  |
| 0x0E | `ACTION_CREATE_IMPACT_MARKET`     | shipped  |
| 0x0F | `ACTION_RESOLVE_EVENT`            | shipped  |
| 0x10 | `ACTION_UPDATE_MARKET_FEES`       | shipped  |
| 0x11 | `ACTION_RUN_LIQUIDATION_SWEEP`    | shipped  |
| 0x12 | `ACTION_RUN_FUNDING_TICK`         | shipped  |
| 0x13 | `ACTION_SET_ACCOUNT_FEE_OVERRIDE` | planned  |
| 0x14 | `ACTION_ORACLE_UPDATE_COMPOSITE`  | planned  |
| 0x15 | `ACTION_FAIL_DEPOSIT`             | planned  |
| 0x16 | `ACTION_SET_USER_MARKET_LEVERAGE` | planned  |
| 0x17 | `ACTION_CLOSE_POSITION`            | shipped  |
| 0x18 | `ACTION_CANCEL_CLIENT_ORDER`       | shipped  |
| 0x19 | `ACTION_CANCEL_ALL_ORDERS`         | shipped  |
| 0x1A | `ACTION_CANCEL_REPLACE_ORDER`      | shipped  |
| 0x1B | `ACTION_AMEND_ORDER`               | shipped  |
| 0x1C | `ACTION_ATOMIC_BASKET_ORDER`       | shipped  |
| 0x1D | `ACTION_LIQUIDATE_ACCOUNTS`        | shipped  |
| 0x1E | `ACTION_PROPOSE_ADMIN_ACTION`      | planned  |
| 0x1F | `ACTION_APPROVE_ADMIN_ACTION`      | planned  |
| 0x20 | `ACTION_REJECT_ADMIN_ACTION`       | planned  |
| 0x21 | `ACTION_EMERGENCY_ADMIN_ACTION`    | planned  |
| 0x22 | `ConfirmWithdrawalReceipt`          | engine-native |
| 0x23 | `FailWithdrawalReceipt`             | engine-native |
| 0x24 | `AuthorizeWithdrawal`               | engine-native |
| 0x25 | `SetPositionTriggers`               | dormant behind compiled activation gate |
| 0x26 | `CancelPositionTriggers`            | dormant behind compiled activation gate |
| 0x27 | `ReservedRt01`                      | reserved range, no action arm (see `external_action_reservations`) |
| 0x28 | `ReservedRt01`                      | reserved |
| 0x29 | `ReservedRt01`                      | reserved |
| 0x2A | `ReservedRt01`                      | reserved |
| 0x2B | `ReservedRt01`                      | reserved |
| 0x2C | `ReservedRt01`                      | reserved |
| 0x2D | _free_                              | —        |
| ...  | up to 0xFF                         | —        |

## Inner admin action types

`AdminActionType` in [`src/types.rs`](src/types.rs). These tags are committed by
`admin_proposal_content_hash`, which derives them from the typed action; they
are not outer transaction action bytes.

| Byte | Variant                           | Status   |
|------|-----------------------------------|----------|
| 0x01 | `CreateMarket`                    | planned  |
| 0x02 | `UpdateAdminSignerRegistry`       | planned  |
| 0x03 | `CreateImpactMarket`              | planned  |
| 0x04 | `Batch`                           | planned  |
| 0x05 | `SetTriggerMarketConfig`          | dormant behind trigger-index gate |
| 0x06 | `UnpauseBridge`                   | height-gated per lineage (`UNPAUSE_BRIDGE_ACTIVATIONS`) |
| 0x07 | `UpdateAuthoritySet`              | dormant behind authority-governance gate |
| 0x08 | `CancelAllOrdersForAccount`       | height-gated per lineage (`CANCEL_ALL_FOR_ACCOUNT_ACTIVATIONS`) |
| 0x09 | `CreateEvent`                     | governed; creates a standalone event |
| 0x0A | `ReservedRt01B`                   | reserved; discriminant only, no behaviour |
| 0x0B | `ReservedRt01C`                   | reserved; discriminant only, no behaviour |

Tags `0x03`/`0x04` are admin-actions v2: admission is height-gated by
`UPGRADE_HEIGHT_ADMIN_ACTIONS_V2` (parked at `u64::MAX` on trunk, pinned at
release-tag time), but the tags are ASSIGNED from this commit on. The content
hash commits them, so they can never be repurposed regardless of when they
activate.

Tag `0x05` has its own `UPGRADE_HEIGHT_TRIGGER_INDEX` admission gate, also
parked at `u64::MAX`; the dormant allocation reserves and tests the wire
contract but cannot schedule or apply a live policy.

Tags `0x07` and `0x08` are admission-gated per lineage
(`AUTHORITY_GOVERNANCE_ACTIVATIONS` and `CANCEL_ALL_FOR_ACCOUNT_ACTIVATIONS`):
each is ASSIGNED from this commit, the content hash commits it, and each
lineage decides independently when to activate it.

## Emergency action arm tags

`EmergencyActionType` in [`src/types.rs`](src/types.rs). Committed by the
on-chain emergency audit log (`EmergencyActionRecord::action_tag`) and by the
governance digest; never repurpose after use on a persistent chain. No arm is
admitted at initial release; each activates later behind its own pairing gate.

| Byte | Arm                | Status   |
|------|--------------------|----------|
| 0x01 | `PauseMarket`      | planned  |
| 0x02 | `HaltTrading`      | planned  |
| 0x03 | `SetReduceOnly`    | planned  |

## Reservation tracker

Reserved-but-not-yet-live slots in this crate's namespaces, so a new feature
claims from the reservation instead of re-scanning the source. The implementing
feature renames each `ReservedRt01*` slot to its real name in the same commit
that gives it behaviour.

| Namespace | Reserved range | Mechanism | Status |
|---|---|---|---|
| Outer action bytes | `0x27`–`0x2C` | `external_action_reservations` (no Rust arm) | parked |
| Inner admin tags | `0x09`–`0x0B` | `AdminActionType` discriminant-only variants | parked |

## Process for adding a new byte

1. Pick the next free slot in the relevant table above.
2. Update the `define_actions!` list in `codec.rs` (outer action byte) or the
   `AdminActionType` enum in `types.rs` (inner admin tag).
3. Update this file in the **same commit**: the ledger test treats the table as
   part of the contract and fails if the assigned row is missing.
