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
- 1.5.0 → 1.6.0: `ResolveEvent` (outer action `0x27`) for standalone events; the `0x0F` event resolution is renamed `ResolveImpactMarket` (payload unchanged)
- 1.7.0 → 1.8.0: `ScheduleUpgrade` (inner tag `0x0E`) and `CancelUpgrade` (`0x0F`): the on-chain upgrade-plan path replacing `UPGRADE_PLAN` env coordination
- 1.8.0 → 1.9.0: sub-accounts: `CreateSubAccount` (outer `0x28`), `SubAccountTransfer` (`0x29`), the frozen address derivation, error codes 83–90
- 1.9.0 → 2.0.0: **breaking**: the event and its conditional is removed (`CreateImpactMarket` `0x0E`, `ResolveImpactMarket` `0x0F`, inner tag `0x03`, `ImpactMarketInfo`, error codes 24/25/31, all retired, never reused); `ConditionalPerp` is re-rooted onto `EventId`, `EventInfo` carries its attached conditionals, `AttachConditional` claims inner tag `0x0A`, event error codes 92–96

## The byte ledger

[`BYTES.md`](BYTES.md) is the allocation ledger for outer action bytes and
inner admin-action tags. The rule: **any change that takes a new byte updates
the table in the same commit**, and `codec.rs`'s ledger tests fail if an
assigned byte is missing from the table.

## Golden vectors

Every file under [`vectors/`](vectors/) is one frozen MessagePack value as a
single line of lowercase hex. The directory says how to decode it:

| Path | Holds | Named by |
|---|---|---|
| `vectors/*.hex` | a signed transaction envelope, or a bare action payload | the action |
| `vectors/events/<event type>.hex` | one `types::Event` variant | the ABCI event type it renders as (`trade_executed.hex` is `Event::TradeExecuted`) |
| `vectors/responses/<wire type>.hex` | one read-model response | the Rust type in snake case (`account_info.hex` is `query::AccountInfo`) |

The response set is `account_info` and `position_brief` (`/v1/account`),
`market_config` (one row of `/v1/markets`) and `event_info` (one row of the
event queries). The event set is every variant the indexer folds into a
derived table. Each value is built from a fixture in
[`tests/frozen_read_vectors.rs`](tests/frozen_read_vectors.rs) (events and
responses) or `src/codec.rs` (transactions), which checks that the fixture
encodes to the file and that the file decodes and re-encodes to the same bytes.
Optional fields carry a value in the fixtures, so the vectors pin their
payload encoding and not only `nil`.

Before a release, CI decodes every vector of the last wire release that has
since changed or gone with the current crate, by kind
(`examples/decode_tx.rs`); one that no longer decodes makes the change MAJOR.

### Fetching the set as a consumer

Fetch the vectors of the wire version your dependency pins, from the public
mirror at its tag, and decode each with your own decoder:

```sh
git clone --depth 1 --branch v<MAJOR.MINOR.PATCH> https://github.com/Proof-labs/wire.git
# or a single file:
curl -fsSL https://raw.githubusercontent.com/Proof-labs/wire/v<MAJOR.MINOR.PATCH>/vectors/responses/account_info.hex
```

Decode every file in the directories you consume and fail on any error, so a
new file is covered without a code change. The same set ships as
`golden-vectors.tgz` on each engine release. To see drift before it is
released, also decode the mirror's `dev` branch: a failure there is a change
your decoder will need when you next move the pin.

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
