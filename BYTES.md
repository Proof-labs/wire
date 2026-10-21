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
Bytes `0x00` and `0xFF` are reserved as sentinels (unused / max). A byte
whose action has been removed is **retired**: its row stays, marked
`retired`, and the value is never reassigned.

## Action types

Byte 2 of the transaction envelope, assigned by `define_actions!`. Names are
the engine's PascalCase identifiers, the canonical vocabulary every other
spelling derives from (`PlaceOrder`; JSON type `placeOrder`).

| Byte | Name                                | Status   |
|------|-------------------------------------|----------|
| 0x01 | `PlaceOrder`                        | shipped  |
  ↳ proof-wire 2.1.0: `PlaceOrder` payload gains trailing optional `stop_loss` / `take_profit` limbs (`serde(default)`, nil when absent); byte and field order unchanged.
| 0x02 | `CancelOrder`                       | shipped  |
| 0x03 | `OracleUpdate`                      | shipped  |
| 0x04 | `MarketOrder`                       | shipped  |
  ↳ proof-wire 2.1.0: `MarketOrder` payload gains the same trailing optional `stop_loss` / `take_profit` limbs (`serde(default)`, nil when absent); byte unchanged.
| 0x05 | `Deposit`                           | shipped  |
| 0x06 | `Withdraw`                          | shipped  |
| 0x07 | `CreateMarket`                      | shipped  |
| 0x08 | `WithdrawRequest`                   | shipped  |
| 0x09 | `ConfirmDeposit`                    | shipped  |
| 0x0A | `ConfirmWithdrawal`                 | shipped  |
| 0x0B | `FailWithdrawal`                    | shipped  |
| 0x0C | `ApproveAgent`                      | shipped  |
| 0x0D | `RevokeAgent`                       | shipped  |
| 0x0E | `CreateImpactMarket`                | retired  |
| 0x0F | `ResolveImpactMarket`               | retired  |
| 0x10 | `UpdateMarketFees`                  | shipped  |
| 0x11 | `RunLiquidationSweep`               | retired  |
| 0x12 | `RunFundingTick`                    | shipped  |
| 0x13 | `SetAccountFeeOverride`             | planned  |
| 0x14 | `OracleUpdateComposite`             | planned  |
| 0x15 | `FailDeposit`                       | planned  |
| 0x16 | `SetUserMarketLeverage`             | planned  |
| 0x17 | `ClosePosition`                     | shipped  |
| 0x18 | `CancelClientOrder`                 | shipped  |
| 0x19 | `CancelAllOrders`                   | shipped  |
| 0x1A | `CancelReplaceOrder`                | shipped  |
  ↳ proof-wire 2.1.0: `CancelReplaceOrder` payload gains the same trailing optional `stop_loss` / `take_profit` limbs (`serde(default)`, nil when absent); byte unchanged.
| 0x1B | `AmendOrder`                        | shipped  |
| 0x1C | `AtomicBasketOrder`                 | shipped  |
| 0x1D | `LiquidateAccounts`                 | shipped  |
| 0x1E | `ProposeAdminAction`                | planned  |
| 0x1F | `ApproveAdminAction`                | planned  |
| 0x20 | `RejectAdminAction`                 | planned  |
| 0x21 | `EmergencyAdminAction`              | planned  |
| 0x22 | `ConfirmWithdrawalReceipt`          | engine-native |
| 0x23 | `FailWithdrawalReceipt`             | engine-native |
| 0x24 | `AuthorizeWithdrawal`               | engine-native |
| 0x25 | `SetPositionTriggers`               | dormant behind compiled activation gate |
| 0x26 | `CancelPositionTriggers`            | dormant behind compiled activation gate |
| 0x27 | `ResolveEvent`                      | shipped  |
| 0x28 | `CreateSubAccount`                  | refused with code 90 until the governed `EnableSubAccounts` action executes |
| 0x29 | `SubAccountTransfer`                | refused with code 90 until the governed `EnableSubAccounts` action executes |
| 0x2A | `ReservedRt01`                      | reserved |
| 0x2B | `ReservedRt01`                      | reserved |
| 0x2C | `ReservedRt01`                      | reserved |
| 0x2D | `SubmitOracleObservation`           | dormant behind oracle-policy gate |
| 0x2E | `ClaimWithdrawalPayout`             | single-holder payout claim on a pending withdrawal |
| 0x2F | `ExecuteTriggers`                   | internal, proposer-injected trigger candidates |
| ...  | up to 0xFF                         | —        |

## Inner admin action types

`AdminActionType` in [`src/types.rs`](src/types.rs). These tags are committed by
`admin_proposal_content_hash`, which derives them from the typed action; they
are not outer transaction action bytes.

| Byte | Variant                           | Status   |
|------|-------------------------------------|----------|
| 0x01 | `CreateMarket`                    | planned  |
| 0x02 | `UpdateAdminSignerRegistry`       | planned  |
| 0x03 | `CreateImpactMarket`              | retired  |
| 0x04 | `Batch`                           | planned  |
| 0x05 | `SetTriggerMarketConfig`          | dormant behind trigger-index gate |
| 0x06 | `UnpauseBridge`                   | height-gated per lineage (`UNPAUSE_BRIDGE_ACTIVATIONS`) |
| 0x07 | `UpdateAuthoritySet`              | dormant behind authority-governance gate |
| 0x08 | `CancelAllOrdersForAccount`       | height-gated per lineage (`CANCEL_ALL_FOR_ACCOUNT_ACTIVATIONS`) |
| 0x09 | `CreateEvent`                     | governed; creates a standalone event |
| 0x0A | `AttachConditional`               | governed; attaches a conditional to an event |
| 0x0B | `ReservedRt01C`                   | reserved; discriminant only, no behaviour |
| 0x0C | `ConfigureOraclePolicy`           | dormant behind oracle-policy gate |
| 0x0D | `SetOracleGuards`                 | dormant behind `UPGRADE_HEIGHT_ORACLE_GUARDS_CONFIG` |
| 0x0E | `ScheduleUpgrade`                 | governed; schedules the pending protocol-upgrade plan |
| 0x0F | `CancelUpgrade`                   | governed; cancels the pending plan (before H) |
| 0x10 | `SetPlpConfig`                    | governed; writes the global Proof liquidity provider (PLP) backstop config |
| 0x11 | `SetLiquidationConfig`            | governed; replaces the global liquidation configuration (compare-and-set on its sequence) |
| 0x12 | `FundInsuranceFund`               | quorum-authorized named-account transfer into pools with registered markets |
| 0x13 | `SetWithdrawalLimit`              | governed; writes the per-account rolling withdrawal limit (CU-06) |
| 0x14 | `SetOperatorReceiptRegistry`       | governed; writes the operator receipt registry (rotation, DEC-112) |
| 0x15 | `EnableSubAccounts`                | governed; authorizes the sub-account actions (SA-1), one-way |
| 0x16 | reserved                          | partial-liquidation settings; no behavior in this release |
| 0x17 | reserved                          | partial-liquidation settings; no behavior in this release |
| 0x18 | reserved                          | partial-liquidation settings; no behavior in this release |
| 0x19 | `WithdrawInsuranceFund`           | quorum-authorized pool withdrawal to a named account; pool remains nonnegative |
| 0x1A | `SetMarginOffsetPolicy`           | governed partial-margin offset policy |

The unmerged insurance prototype's withdrawal tag `0x14` and treasury-registry
placeholder `0x13` were never activated. They are not compatibility assignments:
the merged receipt registry owns `0x14`, and merged #836 owns `0x13`. Consumers
must pin the revised insurance wire source before qualification or submission.

Insurance transfer events start with the named account movement, followed by
`InsuranceFundUpdated` and the corresponding funding/withdrawal receipt for
each allocation in ascending pool order. Funding uses `InsuranceSourceDebited`
(not a treasury-only account); withdrawal uses `InsuranceRecipientCredited`.
`TreasurySourceDebited` is accepted only as a historical decode alias for the
unreleased prototype; new events use `InsuranceSourceDebited`. Action tags,
payload bytes and funding/withdrawal replay IDs are unchanged by this naming.

The oracle-policy outer `0x2D`, inner `0x0C`, and state prefixes `0x51`–`0x53`
are reserved now. Admission is disabled by `UPGRADE_HEIGHT_ORACLE_POLICY =
u64::MAX`. Source messages authenticate an approved relay, not a provider proof.

Tag `0x04` (`Batch`) is admin-actions v2: admission is height-gated by
`UPGRADE_HEIGHT_ADMIN_ACTIONS_V2` (parked at `u64::MAX` on trunk, pinned at
release-tag time), but the tag is ASSIGNED from this commit on. The content
hash commits it, so it can never be repurposed regardless of when it
activates. Tag `0x03` was `CreateImpactMarket`, retired with the conditional
family in 2.0.0; historical proposal hashes commit it, so it is never
reassigned.

Tag `0x11` (`SetLiquidationConfig`) claims the RT-01 reservation. Its one
record lives under state prefix `0x68`; genesis and the upgrade that introduces
it write the initial record, so a running chain always has one.

Tag `0x0A` (`AttachConditional`) claims the RT-01 reservation: governed like
`CreateEvent` (tag `0x09`), it attaches one underlying perpetual to an
existing event as two conditional-perp books.

Tag `0x05` has its own `UPGRADE_HEIGHT_TRIGGER_INDEX` admission gate, also
parked at `u64::MAX`; the dormant allocation reserves and tests the wire
contract but cannot schedule or apply a live policy.

Tags `0x07` and `0x08` are admission-gated per lineage
(`AUTHORITY_GOVERNANCE_ACTIVATIONS` and `CANCEL_ALL_FOR_ACCOUNT_ACTIVATIONS`):
each is ASSIGNED from this commit, the content hash commits it, and each
lineage decides independently when to activate it.

Tag `0x0D` (`SetOracleGuards`) is admission-gated by the compiled
`UPGRADE_HEIGHT_ORACLE_GUARDS_CONFIG` (parked at `u64::MAX` on trunk; a
release branch pins it). Same rule:
the tag is ASSIGNED from this commit regardless of when it activates.

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
| Outer action bytes | `0x2A`–`0x2C` | `external_action_reservations` (no Rust arm) | parked |
| Inner admin tags | `0x0B` | `AdminActionType` discriminant-only variant | parked |

## Process for adding a new byte

1. Pick the next free slot in the relevant table above.
2. Update the `define_actions!` list in `codec.rs` (outer action byte) or the
   `AdminActionType` enum in `types.rs` (inner admin tag).
3. Update this file in the **same commit**: the ledger test treats the table as
   part of the contract and fails if the assigned row is missing.

## Process for retiring a byte

1. Stop the byte decoding. An outer action loses its `define_actions!` entry
   and joins `RETIRED_ACTION_BYTES`. An inner admin tag keeps its
   `AdminActionType` discriminant marked `#[deprecated]` (the hole stays
   named), loses its `AdminAction` arm — decoding goes by that arm's name —
   and joins `RETIRED_ADMIN_TAGS`. The ledger tests assert both lists.
2. Keep the row here and set its status to `retired`. Never delete the row and
   never hand the value to a new action: chains, signed transactions and
   proposal hashes that used it still exist.
3. Bump the crate's MAJOR version: bytes that decoded before no longer do.
