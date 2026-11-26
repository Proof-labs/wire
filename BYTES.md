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
spelling derives from (`PlaceOrders`; JSON type `placeOrders`).

| Byte | Name                                | Status   |
|------|-------------------------------------|----------|
| 0x01 | `PlaceOrder`                        | retired  |
  ↳ replaced by the list `PlaceOrders` (0x35); see "List actions" below.
| 0x02 | `CancelOrder`                       | retired  |
  ↳ replaced by the list `CancelOrders` (0x36).
| 0x03 | `OracleUpdate`                      | retired  |
| 0x04 | `MarketOrder`                       | retired  |
  ↳ replaced by a `PlaceOrders` market entry (0x35).
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
| 0x12 | `RunFundingTick`                    | retired  |
| 0x13 | `SetAccountFeeOverride`             | planned  |
| 0x14 | `OracleUpdateComposite`             | retired  |
| 0x15 | `FailDeposit`                       | planned  |
| 0x16 | `SetUserMarketLeverage`             | shipped  |
| 0x17 | `ClosePosition`                     | retired  |
  ↳ replaced by a reduce-only `PlaceOrders` market entry on the opposite side (0x35).
| 0x18 | `CancelClientOrder`                 | retired  |
  ↳ replaced by the list `CancelClientOrders` (0x37).
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
| 0x22 | `ConfirmWithdrawalReceipt`          | retired (wire 12: the vote model replaces the receipt path) |
| 0x23 | `FailWithdrawalReceipt`             | retired (wire 12: the vote model replaces the receipt path) |
| 0x24 | `AuthorizeWithdrawal`               | retired (wire 12: the vote model replaces the receipt path) |
| 0x25 | `SetPositionTriggers`               | dormant behind compiled activation gate |
| 0x26 | `CancelPositionTriggers`            | dormant behind compiled activation gate |
| 0x27 | `ResolveEvent`                      | shipped  |
| 0x28 | `CreateSubAccount`                  | shipped  |
| 0x29 | `SubAccountTransfer`                | shipped  |
| 0x2A | `ReservedRt01`                      | reserved |
| 0x2B | `ReservedRt01`                      | reserved |
| 0x2C | `ReservedRt01`                      | reserved |
| 0x2D | `SubmitOracleObservation`           | dormant behind oracle-policy gate |
| 0x2E | `ClaimWithdrawalPayout`             | retired (wire 14: the payout slot is the only lock) |
| 0x2F | `ExecuteTriggers`                   | internal, proposer-injected trigger candidates |
| 0x30 | `VoteWithdrawalPaid`                | one operator-registry member's vote to settle a withdrawal as Paid; the engine aggregates to the threshold |
| 0x31 | `RecordWithdrawalPayout`            | a bridge operator's record of its Squads vault payout proposal, signed by its registry key (wire 12 repurposed the payload from a Solana signature; wire 14 replaced the envelope-signed holder with the operator signature) |
| 0x32 | `VoteDeposit`                       | one operator-registry member's vote to credit an observed deposit; the engine aggregates to the threshold |
| 0x33 | `VotePauseBridge`                   | one operator-registry member's vote to pause the bridge; the threshold pauses |
| 0x34 | `VoteWithdrawalPayoutDead`          | one operator-registry member's vote that the recorded payout is dead; the threshold clears the payout slot |
| 0x35 | `PlaceOrders`                       | shipped  |
| 0x36 | `CancelOrders`                      | shipped  |
| 0x37 | `CancelClientOrders`                | shipped  |
| ...  | up to 0xFF                         | —        |

### List actions

`PlaceOrders` (0x35), `CancelOrders` (0x36) and `CancelClientOrders` (0x37)
carry 1..=100 entries (`MAX_LIST_ENTRIES`) for one owner, always as a list:
`[owner, [entry, ...]]`. A `PlaceOrders` entry is an `OrderEntry`: market,
side, quantity, kind (`Limit { price, time_in_force, post_only }` or
`Market { worst_price }`), reduce_only, client_order_id and the two trigger
limbs. A cancel entry is an order id or a client order id.
A single order is a list of one. 0 or more than 100 entries do not decode.
They replace the single-order actions on 0x01, 0x02 and 0x18, which are
retired: transactions signed for those bytes no longer decode, and a stored
proposal holding one is refused as retired. The market order and close on
0x04 and 0x17 are retired the same way.

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
| 0x10 | `SetPlpConfig`                    | retired  |
| 0x11 | `SetLiquidationConfig`            | governed; replaces the global liquidation configuration (compare-and-set on its sequence) |
| 0x12 | `FundInsuranceFund`               | quorum-authorized transfer from a named account to the insurance accounts of pools with registered markets |
| 0x13 | `SetWithdrawalLimit`              | governed; writes the per-account rolling withdrawal limit (CU-06) |
| 0x14 | `SetOperatorReceiptRegistry`       | governed; writes the operator receipt registry (rotation, DEC-112) |
| 0x15 | `EnableSubAccounts`                | retired  |
| 0x16 | reserved                         | partial-liquidation settings; no behavior in this release |
| 0x17 | reserved                         | partial-liquidation settings; no behavior in this release |
| 0x18 | reserved                         | partial-liquidation settings; no behavior in this release |
| 0x19 | `WithdrawInsuranceFund`           | quorum-authorized transfer from pool insurance accounts to a named account |
| 0x1A | `ExecInsuranceFund`               | quorum executes one whitelisted action (agent approve/revoke, leverage, cancels, a `PlaceOrders` of reduce-only market orders) as a pool's insurance account |
| 0x1B | `SetMarginOffsetPolicy`            | governed; replaces the explicit partial-margin offset policy (compare-and-set on its sequence) |
| 0x1C | `SetDepositLimit`                 | governed; writes the `ConfirmDeposit` / `Deposit` per-deposit maximum and protocol-wide rolling-window cap (exchange#425); both off at 0 and when unset |
| 0x1D | `VoidEvent`                       | governed; voids an event, settling every binary and conditional on it in cash at the quorum-set Yes price |
| 0x1E | `SettleEvent`                     | governed; settles on the quorum's Yes or No a price event whose resolution window closed unresolved, or an event with no oracle rule from its settlement time |
| 0x1F | `SetMarketStatus`                 | governed; moves a perpetual Trading to ReduceOnly, back, or ReduceOnly to Delisted (settles its positions at the last certified price and retires its oracle) |
| 0x20 | `PauseBridge`                     | governed; sets the stored bridge halt flag — claims, payout reports and bridge votes refuse until `UnpauseBridge` (0x06) clears it. Tags 0x16-0x18 stay reserved for the partial-liquidation settings |
| 0x21 | `SetBridgePayoutConfig`           | governed; writes the Squads payout vault and the ops rent wallet payout reports are checked against |
| 0x22 | `SetDepositReceiptLimit`          | governed; writes the caps on vote-credited deposits (DEC-292), a ledger separate from `SetDepositLimit` |
| 0x23 | `UpdateMarketFees`                | governed; `QuorumUpdateMarketFees`: `predecessor`, the `MarketSettingsSequence` the market was at when proposed (the v2 markets read serves it as `settings_sequence`), then `change`, a `MarketSettingsChange`: the outer `UpdateMarketFees` (0x10) payload without its `signer`, its other fields in the same order, since the quorum is the authorization. A proposal is refused with code 155 unless `predecessor` is the market's current sequence, and with code 9 (`UnknownMarket`) for a market that does not exist. Runs the checks of 0x10, and at execution fails with code 155 when the market's sequence has moved since the proposal. Every `UpdateMarketFees` that changes a setting, by a key (0x10) or by the quorum, moves the sequence up by one; `SetOracleGuards` does not. Vector `vectors/propose_update_market_fees.hex`: market 1, BTC on DevNet (`sz_decimals` 5), `max_open_interest` from 30,000 to 35,000 and every other field nil, so BTC keeps its other settings; `predecessor` 0, the sequence of a market with no settings change recorded |
| 0x24 | `SetAccountFeeOverride`           | governed; `AccountFeeOverrideChange`: the outer `SetAccountFeeOverride` (0x13) payload without its `signer` (`account`, `taker_fee_bps`, `maker_fee_bps`, `seq`), its other fields in the same order, since the quorum is the authorization. Runs the checks of 0x13: a fee above 10,000 basis points that is not the revert sentinel `u32::MAX` is refused with code 42, and a `seq` at or below the account's stored one with code 43, both at proposal and again at execution, where another change may have taken the `seq`. Emits `AccountFeeOverrideSet` with no signer. `seq` only has to rise: by convention it is the proposal's time in milliseconds. Pending proposals for one account execute in `seq` order: once a higher `seq` executes, every pending lower one fails with code 43. Once the signer registry exists, the outer 0x13 is refused with code 70. Vector `vectors/propose_set_account_fee_override.hex`: account `0xDD…DD`, taker 3 and maker 0 basis points, `seq` 1 |
| 0x25 | `SetWithdrawalPolicy`             | governed; writes the `WithdrawalPolicy` (`fee_micro_usdc`, `min_withdrawal_micro_usdc`) every later `WithdrawRequest` uses; a minimum below the fee is refused. Emits `WithdrawalPolicySet`. Frozen vector in `codec.rs` (`set_withdrawal_policy_wire_vector_frozen`): $1 fee, $2 minimum |

The unmerged insurance prototype's withdrawal tag `0x14` and treasury-registry
placeholder `0x13` were never activated. They are not compatibility assignments:
the merged receipt registry owns `0x14`, and merged #836 owns `0x13`. Consumers
must pin the revised insurance wire source before qualification or submission.

Insurance transfer events start with the named account movement, followed by
`InsuranceFundUpdated` and the corresponding funding/withdrawal receipt for
each allocation in ascending pool order. Funding uses `InsuranceSourceDebited`
(not a treasury-only account); withdrawal uses `InsuranceRecipientCredited`.
Action tags, payload bytes and funding/withdrawal replay IDs are unchanged by
this naming.

The oracle-policy outer `0x2D`, inner `0x0C`, and state prefixes `0x51`–`0x53`
are reserved now. Admission is disabled by `UPGRADE_HEIGHT_ORACLE_POLICY =
u64::MAX`. Source messages authenticate an approved relay, not a provider proof.

Tag `0x04` (`Batch`) is admin-actions v2: admission is height-gated by
`UPGRADE_HEIGHT_ADMIN_ACTIONS_V2` (parked at `u64::MAX` on trunk, pinned at
release-tag time), but the tag is ASSIGNED from this commit on. The content
hash commits it, so it can never be repurposed regardless of when it
activates. Tag `0x03` was `CreateImpactMarket`, retired with the conditional
family in 2.0.0; historical proposal hashes commit it, so it is never
reassigned. Tag `0x10` was `SetPlpConfig`, retired with the engine's
liquidity-provider backstop, and tag `0x15` was `EnableSubAccounts`, retired
when sub-accounts became always on; neither is ever reassigned.

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
governance digest; never repurpose after use on a persistent chain.

| Byte | Arm                | Status   |
|------|--------------------|----------|
| 0x01 | `PauseMarket`      | live; halts one market, `SetMarketStatus` lifts it |
| 0x02 | `HaltTrading`      | retired, never admitted |
| 0x03 | `SetReduceOnly`    | retired, never admitted |

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
