# proof-wire

The byte-level contract shared by the Proof exchange engine and its clients:
MessagePack codec and canonicalization, action/event/query types, the Ed25519
signing preimage, the trigger wire types, and the `ExecError` code table.

The engine ([Proof-labs/exchange](https://github.com/Proof-labs/exchange)) and
the public SDK ([Proof-labs/trading-sdk](https://github.com/Proof-labs/trading-sdk))
compile this **one** definition, so the bytes a client signs are exactly the
bytes the engine accepts: drift is impossible by construction.

## Consumers

| Consumer | Dependency form |
|---|---|
| `exchange-core` (the engine) | git dependency, pinned to a `v*` tag |
| `proof-trading-sdk` (TS/Python SDK; WASM + PyO3 builds of this crate) | git dependency, pinned to a `v*` tag |

## Versioning discipline

Every wire-format change is at least a **MINOR** bump; a change that breaks
decoding of pre-existing bytes is a **MAJOR**. The version history for each
bump lives in the header of [`Cargo.toml`](Cargo.toml). Consumers pin tags
(`vMAJOR.MINOR.PATCH`).

- 1.0.0 → 1.1.0: `DepositLocator` on `ConfirmDeposit`/`FailDeposit`
- 1.1.0 → 1.2.0: `AdminAction::UnpauseBridge` (inner tag `0x06`)
- 1.2.0 → 1.3.0: `UpdateAuthoritySet` (inner tag `0x07`)
- 1.3.0 → 1.4.0: `CancelAllOrdersForAccount` (inner tag `0x08`)
- 1.4.0 → 1.5.0: `AdminAction::CreateEvent` (inner tag `0x09`) and the typed `EventKey`
- 1.5.0 → 1.6.0: `ResolveEvent` (outer action `0x27`) for standalone events; the `0x0F` impact-market resolution is renamed `ResolveImpactMarket` (payload unchanged)
- 1.7.0 → 1.8.0: `ScheduleUpgrade` (inner tag `0x0E`) and `CancelUpgrade` (`0x0F`): the on-chain upgrade-plan path replacing `UPGRADE_PLAN` env coordination

## The byte ledger

[`BYTES.md`](BYTES.md) is the allocation ledger for outer action bytes and
inner admin-action tags. The rule: **any change that takes a new byte updates
the table in the same commit**, and `codec.rs`'s ledger tests fail if an
assigned byte is missing from the table.

## Golden vectors

[`vectors/*.hex`](vectors/) are the frozen cross-language vectors (PlaceOrder,
CancelOrder, OracleUpdate, the three withdrawal-receipt terminals). They are
the input to the engine's integration tests and the SDK's conformance suite,
and ship as the contract artifact attached to engine releases.

## Invariants CI enforces

- the crate stays **dep-light and wasm/pyo3-clean**: no `rand_core`/`getrandom`
  in the graph (a `getrandom` anywhere breaks the `wasm32-unknown-unknown`
  build), enforced by a dependency-hygiene gate plus a wasm build;
- the crate's own lints deny `unwrap`/`expect`/`panic`/`float`/arithmetic
  shortcuts;
- the ledger tests, frozen wire vectors, and round-trip/stress suites all run
  on every change.

## License

Apache-2.0.
