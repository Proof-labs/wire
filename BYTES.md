# Byte Allocation Ledger

> [!important] Two ledgers, one rule — and in *this* repository, this file is the wire-byte record.
> Within Proof-labs/wire, `BYTES.md` is the allocation ledger for the outer
> action bytes and inner admin-action tags: claim a byte here in the same
> commit as the code — `codec.rs`'s ledger tests fail otherwise. (Before the
> 2026-09-09 extraction this file had been demoted to narrative in favour of
> `protocol-allocations.json` at the root of Proof-labs/exchange; that
> manifest remains the authority for the engine-side namespaces — state
> prefixes, schema numbers, activation heights — enforced there by
> `exchange-core/tests/protocol_allocations.rs`.) A byte claimed on either
> side is claimed everywhere: coordinate in the exchange manifest first,
> then mirror the row here.
>
> This file survives as narrative — what each namespace is for, why a collision
> hurts, and the history of past allocations. Treat any number in it as commentary.

## Why this file stopped being the record

It claimed to be "the single source of truth" and could not be. Audited 2026-08-11
against every surface it names, it was wrong in five ways at once, none of which
could fail a build:

- It said the action type is the **first byte of every transaction envelope**. It is
  **byte 2**. The envelope is a 6-element msgpack fixarray, so byte 0 is `0x96` and
  byte 1 is the envelope version — the golden vectors read `96 02 01 01`
  (`vectors/place_order.hex`). The prefix claim, byte 0 of a KV key,
  was correct.
- It pointed at `exchange-core/src/codec.rs::ACTION_*`. **No such constants exist.**
  Action bytes come from the `define_actions!` macro at `codec.rs:84`.
- Of its 33 action names, **13 exist only in `api-gateway`** and the other **20 exist
  in no repository at all** — the engine's identifiers are bare CamelCase.
- **Eight rows were marked "open PR" for code shipped in `v2.1.0`**, and five prefix
  rows likewise for merged work.
- Its "next free action byte: 0x22" was **false across surfaces**: `trading-sdk`
  ships 0x22, 0x23 and 0x24 today. At audit time the true next free byte was
  **0x25**; W32-10 now assigns 0x25 and 0x26, making 0x27 the next free byte.

Prose cannot be checked, so it drifted, and the drift was invisible.

## The namespaces

1. **Action types** — byte 2 of the transaction envelope, assigned by
   `define_actions!` (`exchange-core/src/codec.rs`). A duplicate routes a transaction
   to the wrong handler. Mirrored by hand in `exchange-node/check_tx.go`,
   `api-gateway/src/action_type.rs`, and the canonical SDK.
2. **State-key prefixes** — byte 0 of every KV key
   (`exchange-core/src/keys.rs::Prefix::*`). Two colliding prefixes overlap their
   keyspaces: no compile error, no CheckTx error, just wrong data under the wrong
   handler. `keys.rs` is the only definition site in any repository.
3. **Schema versions** — `SCHEMA_V*` in `exchange-core/src/repo.rs`. Two branches
   naming the same number produce two migrations that cannot both run.
4. **Activation heights** — `UPGRADE_HEIGHT_*` in the same file. These are consensus
   pins. A pinned height reverted to the fail-closed sentinel silently cancels a
   scheduled upgrade, which is why the manifest covers them: four open pull requests
   currently carry `UPGRADE_HEIGHT_V9 = u64::MAX` at their heads, inherited from bases
   that predate the pin. A correct three-way merge keeps the live value; a conflict
   resolved by taking the branch's whole file does not, and nothing else would notice.

Inner `AdminAction` tags and execution-result codes are separate numeric namespaces,
not yet in the manifest. ABCI event types are strings. Overlap *between* namespaces is
legal; reuse *inside* one is not.

## Action types

Assigned by `define_actions!` in `exchange-core/src/codec.rs`. Names below are the
gateway's spelling and are kept for continuity with older reviews; the engine's
identifiers are bare CamelCase (`PlaceOrder`, not `ACTION_PLACE_ORDER`). The manifest
carries the authoritative names.

| Byte | Constant                          | Source             | Status   |
|------|-----------------------------------|--------------------|----------|
| 0x01 | `ACTION_PLACE_ORDER`              | main               | shipped  |
| 0x02 | `ACTION_CANCEL_ORDER`             | main               | shipped  |
| 0x03 | `ACTION_ORACLE_UPDATE`            | main               | shipped  |
| 0x04 | `ACTION_MARKET_ORDER`             | main               | shipped  |
| 0x05 | `ACTION_DEPOSIT`                  | main               | shipped  |
| 0x06 | `ACTION_WITHDRAW`                 | main               | shipped  |
| 0x07 | `ACTION_CREATE_MARKET`            | main               | shipped  |
| 0x08 | `ACTION_WITHDRAW_REQUEST`         | main               | shipped  |
| 0x09 | `ACTION_CONFIRM_DEPOSIT`          | main               | shipped  |
| 0x0A | `ACTION_CONFIRM_WITHDRAWAL`       | main               | shipped  |
| 0x0B | `ACTION_FAIL_WITHDRAWAL`          | main               | shipped  |
| 0x0C | `ACTION_APPROVE_AGENT`            | main               | shipped  |
| 0x0D | `ACTION_REVOKE_AGENT`             | main               | shipped  |
| 0x0E | `ACTION_CREATE_IMPACT_MARKET`     | main               | shipped  |
| 0x0F | `ACTION_RESOLVE_IMPACT_MARKET`    | main               | shipped (renamed from ResolveEvent in 1.6.0) |
| 0x10 | `ACTION_UPDATE_MARKET_FEES`       | main               | shipped  |
| 0x11 | `ACTION_RUN_LIQUIDATION_SWEEP`    | main               | shipped  |
| 0x12 | `ACTION_RUN_FUNDING_TICK`         | main               | shipped  |
| 0x13 | `ACTION_SET_ACCOUNT_FEE_OVERRIDE` | PR #39 (`feat/be46-account-fee-overrides`) | open PR  |
| 0x14 | `ACTION_ORACLE_UPDATE_COMPOSITE`  | PR #52 (`feat/be-31-multi-source-mark-phase-b`) | open PR  |
| 0x15 | `ACTION_FAIL_DEPOSIT`             | `feat/be-40-fail-deposit` (BE-40)             | open PR  |
| 0x16 | `ACTION_SET_USER_MARKET_LEVERAGE` | `feat/be-16-per-user-leverage` (BE-16)        | open PR  |
| 0x17 | `ACTION_CLOSE_POSITION`            | main               | shipped  |
| 0x18 | `ACTION_CANCEL_CLIENT_ORDER`       | main               | shipped  |
| 0x19 | `ACTION_CANCEL_ALL_ORDERS`         | main               | shipped  |
| 0x1A | `ACTION_CANCEL_REPLACE_ORDER`      | main               | shipped  |
| 0x1B | `ACTION_AMEND_ORDER`               | main               | shipped  |
| 0x1C | `ACTION_ATOMIC_BASKET_ORDER`       | main               | shipped  |
| 0x1D | `ACTION_LIQUIDATE_ACCOUNTS`        | main               | shipped  |
| 0x1E | `ACTION_PROPOSE_ADMIN_ACTION`      | W29-04 / PR #282    | open PR  |
| 0x1F | `ACTION_APPROVE_ADMIN_ACTION`      | W29-04 / PR #282    | open PR  |
| 0x20 | `ACTION_REJECT_ADMIN_ACTION`       | W29-04 / PR #282    | open PR  |
| 0x21 | `ACTION_EMERGENCY_ADMIN_ACTION`    | W29-04 / PR #282    | open PR  |
| 0x22 | `ConfirmWithdrawalReceipt`          | W28-20 / #316      | merged to dev — engine-native |
| 0x23 | `FailWithdrawalReceipt`             | W28-20 / #316      | merged to dev — engine-native |
| 0x24 | `AuthorizeWithdrawal`               | W28-20 / #316      | merged to dev — engine-native |
| 0x25 | `SetPositionTriggers`               | W32-10             | dormant behind compiled activation gate |
| 0x26 | `CancelPositionTriggers`            | W32-10             | dormant behind compiled activation gate |
| 0x27 | `ResolveEvent`                      | 1.6.0              | resolve a standalone event |
| 0x28 | `ReservedRt01`                      | RT-01              | reserved |
| 0x29 | `ReservedRt01`                      | RT-01              | reserved |
| 0x2A | `ReservedRt01`                      | RT-01              | reserved |
| 0x2B | `ReservedRt01`                      | RT-01              | reserved |
| 0x2C | `ReservedRt01`                      | RT-01              | reserved |
| 0x2D | `SubmitOracleObservation`            | oracle policy | dormant behind oracle-policy gate |
| 0x2E | _free_                              | —                  | —        |
| ...  | up to 0xFF                         | —                  | —        |

Bytes 0x00 and 0xFF are reserved as sentinels (unused / max).

## Inner admin action types

`exchange-core/src/types.rs::AdminActionType`. These tags are committed by
`admin_proposal_content_hash`; the helper derives them from the typed action.
They are not outer transaction action bytes.

| Byte | Variant                           | Source             | Status   |
|------|-----------------------------------|--------------------|----------|
| 0x01 | `CreateMarket`                    | W29-04 / PR #282    | open PR  |
| 0x02 | `UpdateAdminSignerRegistry`       | W29-04 / PR #282    | open PR  |
| 0x03 | `CreateImpactMarket`              | Specs §11 / PR #334 | open PR  |
| 0x04 | `Batch`                           | Specs §11 / PR #334 | open PR  |
| 0x05 | `SetTriggerMarketConfig`          | W32-10 / TR-1       | dormant behind trigger-index gate |
| 0x06 | `UnpauseBridge`                   | W29-15 / DEC-65     | height-gated per lineage (`UNPAUSE_BRIDGE_ACTIVATIONS`) |
| 0x07 | `UpdateAuthoritySet`              | #422                | dormant behind authority-governance gate |
| 0x08 | `CancelAllOrdersForAccount`       | #467                | height-gated per lineage (`CANCEL_ALL_FOR_ACCOUNT_ACTIVATIONS`) |
| 0x09 | `CreateEvent`                     | EN-01               | governed; creates a standalone event (G17) |
| 0x0A | `ReservedRt01B`                   | RT-01               | reserved; discriminant only, no behaviour |
| 0x0B | `ReservedRt01C`                   | RT-01               | reserved; discriminant only, no behaviour |
| 0x0C | `ConfigureOraclePolicy`           | oracle policy       | dormant behind oracle-policy gate |
| 0x0D | `SetOracleGuards`                 | oracle guards       | dormant behind `UPGRADE_HEIGHT_ORACLE_GUARDS_CONFIG` |

The oracle-policy outer `0x2D`, inner `0x0C`, and state prefixes `0x51`–`0x53`
are reserved now. Admission is disabled by `UPGRADE_HEIGHT_ORACLE_POLICY =
u64::MAX`. Source messages authenticate an approved relay, not a provider proof.

Tags `0x03`/`0x04` are admin-actions v2: admission is height-gated by
`UPGRADE_HEIGHT_ADMIN_ACTIONS_V2` (parked at `u64::MAX` on trunk, pinned at
release-tag time), but the tags are ASSIGNED from this commit on — the
content hash commits them, so they can never be repurposed regardless of
when they activate.

Tag `0x05` has its own `UPGRADE_HEIGHT_TRIGGER_INDEX` admission gate. That
gate is also parked at `u64::MAX`; the dormant state pull request allocates
and tests the wire contract but cannot schedule or apply a live policy.

Tag `0x07` (`UpdateAuthoritySet`, #422) is admission-gated per lineage by
`AUTHORITY_GOVERNANCE_ACTIVATIONS` (parked at `u64::MAX` on
`exchange-devnet-1`, active from genesis on `proof-dev`). The tag is
ASSIGNED from this commit — the content hash commits it — regardless of
when a lineage activates it.

Tag `0x08` (`CancelAllOrdersForAccount`, #467) is admission-gated per
lineage by `CANCEL_ALL_FOR_ACCOUNT_ACTIVATIONS` (parked at `u64::MAX` on
`exchange-devnet-1`, active from genesis on `proof-dev`). Same rule: the
tag is ASSIGNED from this commit regardless of when a lineage activates it.

Tag `0x0D` (`SetOracleGuards`) is admission-gated by the compiled
`UPGRADE_HEIGHT_ORACLE_GUARDS_CONFIG` (parked at `u64::MAX` on trunk; a
release branch pins it). Same rule:
the tag is ASSIGNED from this commit regardless of when it activates.

## Emergency action arm tags

`exchange-core/src/types.rs::EmergencyActionType`. Committed by the
on-chain emergency audit log (`EmergencyActionRecord::action_tag`) and by the
governance digest; never repurpose after use on a persistent chain. Zero arms
are admitted at release A — each activates later as its §3.7 pairing triple.

| Byte | Arm                | Source          | Status   |
|------|--------------------|-----------------|----------|
| 0x01 | `PauseMarket`      | W29-04 slice A3 | open PR  |
| 0x02 | `HaltTrading`      | W29-04 slice A3 | open PR  |
| 0x03 | `SetReduceOnly`    | W29-04 slice A3 | open PR  |

## Governance absorption outcome tags

`exchange-core/src/engine/governance.rs::GovOutcomeTag`. One byte absorbed
into the rolling checksum for every governance transaction (design record
§4.7: `tx_bytes ‖ outcome_tag ‖ governance_digest ‖ applied_state_digest`).
Consensus constants: never renumber once absorbed on a persistent chain.

| Byte | Variant                          | Meaning                                  |
|------|----------------------------------|------------------------------------------|
| 0x00 | `GovOutcomeTag::OuterFailed`     | outer tx failed, no governance mutation  |
| 0x01 | `GovOutcomeTag::VoteRecorded`    | accepted, no terminal transition         |
| 0x02 | `GovOutcomeTag::Executed`        | threshold reached, inner action applied  |
| 0x03 | `GovOutcomeTag::ExecutionFailed` | threshold reached, inner apply failed    |
| 0x04 | `GovOutcomeTag::Rejected`        | terminal rejection                       |
| 0x05 | `GovOutcomeTag::EmergencyExecuted`| single-signer emergency action executed |

## State-key prefixes

`exchange-core/src/keys.rs::Prefix` — value of each enum discriminant.

| Byte | Prefix                          | Source             | Status   |
|------|---------------------------------|--------------------|----------|
| 0x01 | `Order`                         | main               | shipped  |
| 0x02 | `PriceLevel`                    | main               | shipped  |
| 0x03 | `OraclePrice`                   | main               | shipped  |
| 0x04 | `Meta`                          | main               | shipped  |
| 0x05 | `OracleAuth`                    | main               | shipped  |
| 0x06 | `AccountBalance`                | main               | shipped  |
| 0x07 | `Position`                      | main               | shipped  |
| 0x08 | `MarketConfig`                  | main               | shipped  |
| 0x09 | `OwnerOrder`                    | main               | shipped  |
| 0x0A | `InsuranceFund`                 | main               | shipped  |
| 0x0B | `RelayerAuth`                   | main               | shipped  |
| 0x0C | `WithdrawalRecord`              | main               | shipped  |
| 0x0D | `DepositSignature`              | main               | shipped  |
| 0x0E | `AgentAuth`                     | main               | shipped  |
| 0x0F | `AccountNonce`                  | main               | legacy compatibility only |
| 0x10 | `LastTradePrice`                | main               | shipped  |
| 0x11 | `FeePool`                       | main               | shipped  |
| 0x12 | `FundingState`                  | main               | shipped  |
| 0x13 | `CumulativeFunding`             | main               | shipped  |
| 0x14 | `MarkPriceEwma`                 | main               | shipped  |
| 0x15 | `FundingRate`                   | main               | shipped  |
| 0x16 | `ImpactMarketInfo`              | main               | shipped  |
| 0x17 | `InsuranceFundByPool`           | main               | shipped  |
| 0x18 | `HlpConfig`                     | main               | shipped  |
| 0x19 | `AccountFeeOverride`            | main               | shipped  |
| 0x1A | `RealizedBadDebtByPool`         | W28-05 / P0.2      | reserved |
| 0x1B | `PoolConfig`                    | W28-05 / P0.2      | reserved |
| 0x1C | `FailedDepositSignature`        | main               | shipped  |
| 0x1D | `UserMarketLeverage`            | main               | shipped  |
| 0x1E | `AccountFeesAccrued`            | main               | shipped  |
| 0x1F | `Account30dVolume`              | main               | shipped  |
| 0x20 | `AccountFeeOverrideSeq`         | main               | shipped  |
| 0x21 | `CexCompositePrice`             | main               | shipped  |
| 0x22 | `CexCompositePublishTime`       | main               | shipped  |
| 0x23 | `CexCompositeAuth`              | main               | shipped  |
| 0x24 | `AccountRecentNonces`           | issue-111 timestamp nonces | shipped  |
| 0x25 | `OwnerClientOrder`              | private-alpha MM controls | shipped  |
| 0x26 | `MarketOpenInterest`            | W29-09 (v10)       | active — write-through at the position chokepoint since schema v10; no longer trigger-owned |
| 0x27 | `ChildMidTwap`                  | W28-05 / P0.2      | reserved |
| 0x28 | `SocializedLossBlockAccum`      | W28-05 / A-3       | active   |
| 0x29 | `SettlementTwapRing`            | W28-05 / P0.2      | reserved |
| 0x2A | `MarketHaltStatus`              | W28-05 / P0.2      | reserved |
| 0x2B | `RealizedVolEwma`               | W28-05 / P0.2      | reserved |
| 0x2C | `LiqDwell`                      | W28-05 / P0.2      | reserved |
| 0x2D | `PoolBudgetLaggedMin`           | W28-05 / P0.2      | reserved |
| 0x2E | `RelOiActiveCap`                | W28-05 / P0.2      | reserved |
| 0x2F | `MarketCreatedAt`               | W28-05 / P0.2      | reserved |
| 0x30 | `SustainedDepthMs`              | W28-05 / P0.2      | reserved |
| 0x31 | `CumulativeSocializedLoss`      | schema v9 compatibility | active |
| 0x32 | `MarketTwoSidedOpenSize`        | schema v9 compatibility | active |
| 0x33 | `PositionLastSocializedLossIndex` | schema v9 compatibility | active |
| 0x34 | `MarketPositionOwner`           | W29-02 / PR #299   | open PR  |
| 0x35 | `AdminSignerRegistry`           | W29-04 / PR #282   | active   |
| 0x36 | `AdminProposal`                 | W29-04 / PR #282   | active   |
| 0x37 | `EmergencyActionLog`            | W29-04 / PR #282   | active   |
| 0x38 | `OperatorReceiptRegistry`       | W28-20 / PR #316   | open PR  |
| 0x39 | `ConsumedWithdrawalReceipt`     | W28-20 / PR #316   | open PR  |
| 0x3A | `PriceLevelAggregate`           | PR #336            | open PR  |
| 0x3B | `WithdrawalPolicy`              | W28-20 / PR #316   | open PR  |
| 0x3C | `RegistryEpochObligations`      | W28-20 / PR #316   | open PR  |
| 0x3D | `PoolPosition`                  | W29-02 / PR #299   | open PR  |
| 0x3E | `ActiveImpactMarket`            | W29-02 / PR #299   | open PR  |
| 0x3F | `PositionEpoch`                 | W32-10 / TR-1      | dormant allocation |
| 0x40 | `PositionTriggerBracket`        | W32-10 / TR-1      | dormant allocation |
| 0x41 | `TriggerThresholdIndex`         | W32-10 / TR-1      | dormant allocation |
| 0x42 | `TriggerAccountState`           | W32-10 / TR-1      | dormant allocation |
| 0x43 | `TriggerMarketState`            | W32-10 / TR-1      | dormant allocation |
| 0x44 | `TriggerMarketConfig`           | W32-10 / TR-1      | dormant allocation |
| 0x45 | `TriggerClientState`            | W32-10 / TR-1      | dormant allocation |
| 0x46 | `TriggerMeta`                   | W32-10 / TR-1      | dormant allocation |
| 0x47 | `QueuePriorityLevel`            | W32-10 / TR-3      | active behind trigger-index migration |
| ...  | up to 0xFF                      | —                  | —        |

### TriggerMeta subkeys

`Prefix::TriggerMeta` (`0x46`) isolates W32-10 singleton ids, counters, and
migration state from the legacy generic `Meta` namespace. These second bytes
are persistent consensus allocations and must never be renumbered or reused.

| Byte | Trigger meta key                | Source             | Status   |
|------|---------------------------------|--------------------|----------|
| 0x01 | `NextTriggerGroupId`            | W32-10 / TR-1      | dormant allocation |
| 0x02 | `NextTriggerLimbId`             | W32-10 / TR-1      | dormant allocation |
| 0x03 | `TriggerGlobalState`            | W32-10 / TR-1      | dormant allocation |
| 0x04 | `TriggerMigrationState`         | W32-10 / TR-1      | dormant allocation |
| 0x05 | `NextActiveFillId`              | W32-10 / TR-3      | dormant allocation; audited release seed required |
| ...  | up to 0xFF                      | —                  | —        |

### Meta subkeys

`Prefix::Meta` (`0x04`) has a second byte allocated by `MetaKey`. These values
share one namespace and must not be reused independently.

| Byte | Meta key                         | Source             | Status   |
|------|----------------------------------|--------------------|----------|
| 0x01 | `NextOrderId`                    | main               | shipped  |
| 0x02 | `OpenOrderCount`                 | main               | shipped  |
| 0x03 | `Height`                         | main               | shipped  |
| 0x04 | `ChainId`                        | main               | shipped  |
| 0x05 | `NextWithdrawalId`               | main               | shipped  |
| 0x06 | `SchemaVersion`                  | main               | shipped  |
| 0x07 | `NextQueuePriority`              | main               | shipped  |
| 0x08 | `LiqSweepCursor`                 | main               | shipped  |
| 0x09 | `NextAdminProposalId`            | W29-04 / PR #282   | active   |
| 0x0A | `NextEmergencyId`                | W29-04 / PR #282   | active   |
| 0x0B | `LivePositionCount`              | W29-02 / PR #299   | open PR  |
| 0x0C | `ActiveImpactMarketCount`        | W29-02 / PR #299   | open PR  |
| 0x0D | _free_                           | —                  | —        |
| ...  | up to 0xFF                       | —                  | —        |

## Process for adding a new byte

1. Pick the next free slot in the table above.
2. Update the relevant `codec.rs` and/or `keys.rs` constant.
3. Update this file in the **same commit** (don't split into a follow-up
   PR — the table is part of the contract).
4. If your branch will sit unmerged for more than a day or two, watch the
   `feat/*` branches that aren't yet on main for collisions on the byte
   you picked. Run:

   ```sh
   git for-each-ref --format='%(refname:short)' refs/heads refs/remotes/origin |
     while read br; do
       git show "$br":exchange-core/src/keys.rs 2>/dev/null |
         grep -E "= 0x<your byte>"
     done
   ```

   to scan all branches before pushing.

## Upgrade tracker

A single ledger of reserved-but-not-yet-live protocol slots, so a new epic
claims from the reservation instead of re-scanning the source. RT-01
(registry-and-ladder) reserved the wave below; the implementing feature
renames each slot from `ReservedRt01*` to its real name in the same commit
that gives it behaviour.

| Namespace | Reserved range | Mechanism | Status |
|---|---|---|---|
| Outer action bytes | `0x27`–`0x2C` | `external_action_reservations` (no Rust arm) | parked |
| Inner admin tags | `0x09`–`0x0B` | `AdminActionType` discriminant-only variants | parked |
| State-key prefixes | `0x4A`–`0x4D` | `Prefix` variants, no keyspace written | parked |
| Schema rungs | `v15`–`v16` | `SCHEMA_V15/16_*` + `UPGRADE_HEIGHT_V15/16_*` at `u64::MAX` | parked |

## History

- **2026-08-06 — W29-02 bounded-work indexes (PR #299).** Assigned
  `MarketPositionOwner = 0x34`, `PoolPosition = 0x3D`, and
  `ActiveImpactMarket = 0x3E`, plus Meta subkeys `0x0B`/`0x0C`. The
  intervening bridge-receipt allocations are recorded from open PR #316 so
  neither stack can silently reuse them. Open PR #258 must consume this
  canonical `0x34` index and drop its duplicate prefix/migration slice when it
  rebases; the same key cannot be introduced independently at v10.
- **2026-08-05 — Admin-actions v2 inner tags (PR #334).** Assigned inner
  admin tags `0x03 CreateImpactMarket` and `0x04 Batch` (height-gated
  admission, see the inner-admin table note). Added
  `byte_ledger_covers_every_assigned_inner_admin_tag` so the inner table is
  regression-checked like the outer one.
- **2026-08-04 — W28-20 withdrawal authorization.** Entered action byte `0x24`
  (`AuthorizeWithdrawal`), assigned in `codec.rs` one commit earlier without a
  ledger entry. The ledger test now derives its bound from `ActionType::ALL`,
  so an action byte assigned without a ledger row fails the suite instead of
  relying on this table being edited by hand in the same commit.
- **2026-08-03 — W28-20 bridge receipts.** Entered action bytes `0x22`/`0x23`
  (`ConfirmWithdrawalReceipt` / `FailWithdrawalReceipt`) and state prefixes
  `0x38`/`0x39` (`OperatorReceiptRegistry` / `ConsumedWithdrawalReceipt`).
  Recorded that prefixes `0x31`–`0x37` had already been taken in `keys.rs`
  without a ledger entry, so the previous `0x31 = free` row was wrong and a
  branch following the documented process would have collided.
- **2026-07-10 — W28-05 v9 registry.** Reserved the shared risk-platform
  prefix block (`0x1A`, `0x1B`, `0x26`–`0x30`) and activated
  `SocializedLossBlockAccum = 0x28`. Open PR #234 must rebase onto this
  allocation instead of retaining its conflicting `0x26`–`0x28` claims.
- **2026-05-03 — Initial ledger.** Created during the BE-16/26/29/33/40/45/47/51/54/57 + FE-23 implementation pass to deconflict bytes 0x13/0x19/0x1A/0x1B that 4 of those branches initially grabbed. Renumbering applied: BE-40 → 0x15/0x1C, BE-16 → 0x16/0x1D, BE-45 → 0x1E, BE-47 → 0x1F.
