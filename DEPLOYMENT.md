# Deployment

How to deploy the full Velox contract set to a Stellar network: `VeloxRegistry`,
`StreamFactory`, and the `PaymentStream` and `RecurringPayment` WASM that the
factory deploys instances from. See [ARCHITECTURE.md](ARCHITECTURE.md) for why
the contracts have to go up in this order.

> **Not yet run end to end.** `scripts/deploy.sh` has been checked command by
> command against `stellar contract <cmd> --help` and the contracts' real
> argument names, and the parts that need no network (building the WASM,
> generating a key, reading its public key) have been run successfully. The
> machine this was written on cannot reach `*.stellar.org`, so the deploy
> itself has not been run against a live network. Please run it and open an
> issue with the output if anything does not match.

## Prerequisites

- The [Stellar CLI](https://developers.stellar.org/docs/tools/cli) installed
  (`stellar --version`).
- Rust with the `wasm32v1-none` target — see the README's Prerequisites section.
- An identity to deploy with. This account pays every transaction fee and
  becomes the admin of both `VeloxRegistry` and `StreamFactory`, so keep its
  secret key somewhere safe; it is not needed again once deployment succeeds.

## 1. Create and fund a deploy identity

```bash
stellar keys generate velox-admin
stellar keys fund velox-admin --network testnet   # testnet only; fund mainnet separately
```

`stellar keys fund` only works on test networks. For mainnet, send the account
some XLM yourself before deploying.

## 2. Build the contracts

```bash
cargo build --locked --target wasm32v1-none --release
```

## 3. Run the deploy script

```bash
scripts/deploy.sh testnet velox-admin
```

This runs, in order:

1. Deploy `VeloxRegistry`, passing the deploy identity as its `admin`.
2. Upload the `PaymentStream` WASM and note its hash.
3. Upload the `RecurringPayment` WASM and note its hash.
4. Deploy `StreamFactory`, then call its `initialize` with the admin,
   `VeloxRegistry`'s address, and both WASM hashes.
5. Call `VeloxRegistry.set_factory` with `StreamFactory`'s address, so the
   registry accepts entries from it (see the "Authorization" section of
   ARCHITECTURE.md).

On success it writes `deployed.<network>.env`, containing the two contract
addresses and WASM hashes. That file is gitignored: each deployment produces
its own, and it is not meant to be committed.

## 4. Wire up the other repos

- **velox-scheduler** needs `REGISTRY_CONTRACT_ID` from `deployed.<network>.env`
  in its `.env` (see that repo's `.env.example`).
- **velox-frontend** needs `StreamFactory`'s address (`STREAM_FACTORY_CONTRACT_ID`
  in the same file) to call `create_stream` and `create_schedule`.

## Upgrading the stream or schedule WASM

`StreamFactory` deploys every new stream and schedule from the WASM hashes it
was initialized with, or from whatever `set_stream_wasm` / `set_schedule_wasm`
were last called with. To ship a fixed or improved `PaymentStream` or
`RecurringPayment`:

```bash
cargo build --locked --target wasm32v1-none --release
NEW_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/payment_stream.wasm \
  --source-account velox-admin \
  --network testnet)
stellar contract invoke \
  --contract-id <StreamFactory address> \
  --source-account velox-admin \
  --network testnet \
  -- set_stream_wasm --new-hash "$NEW_HASH"
```

Streams and schedules already deployed are unaffected; only ones created
after the call use the new WASM.
