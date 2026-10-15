# Contributing to velox-contracts

Thanks for helping. This repo holds the Soroban smart contracts for Velox. Money moves through these contracts, so changes get careful review and every behaviour needs a test.

## Finding work

Pick an open issue labeled [`good first issue`](https://github.com/Velox-lab/velox-contracts/labels/good%20first%20issue) or [`help wanted`](https://github.com/Velox-lab/velox-contracts/labels/help%20wanted). Comment on the issue to say you are taking it, so two people do not build the same thing. For anything not covered by an issue, open one first and describe the change.

## Setup

You need stable Rust and the Wasm target:

```bash
rustup target add wasm32v1-none
git clone https://github.com/Velox-lab/velox-contracts.git
cd velox-contracts
```

## Checks to run before opening a PR

The `StreamFactory` tests deploy the real `PaymentStream`, `RecurringPayment` and `VeloxRegistry` WASM, so build it first. CI runs exactly these commands:

```bash
cargo build --locked --target wasm32v1-none --release
cargo test --locked
```

`--locked` means `Cargo.lock` must be up to date. If you change dependencies, commit the updated `Cargo.lock`.

## Branches and commits

- Branch from `main`: `feat/short-name`, `fix/short-name`, `docs/short-name`, `test/short-name`.
- Commit messages follow the existing style: `feat:`, `fix:`, `test:`, `docs:`, `ci:`, `chore:`, then a short imperative summary. Explain the why in the body when it is not obvious.
- One concern per PR. A bug fix and an unrelated refactor are two PRs.

## Contract conventions

- **Tests first.** Each contract has its tests in `src/tests.rs`, wired in with `#[cfg(test)] mod tests;`. Cover the success path and each panic, using `#[should_panic(expected = "...")]` with the exact message.
- **Assert values.** A test that only checks "an event was emitted" or "a call did not panic" is not enough. Check the amounts, addresses and state the change should produce.
- **Authorization.** Every function that changes state or moves funds calls `require_auth()` on the right address. Add a test that fails without it.
- **Update state before transferring tokens.**
- **Validate inputs** with `assert!` and a clear message.
- Keep each function doing one thing, as described in [ARCHITECTURE.md](ARCHITECTURE.md). If your change alters storage layout, authorization or the deployment steps, update that file and the README in the same PR.

## Pull requests

- Link the issue with `Closes #N` in the description.
- Fill in the PR template.
- Wait for the CI check to pass. If it shows "waiting for approval" because this is your first contribution, a maintainer needs to approve the run.
- Expect review comments; small follow-up commits on the same branch are fine.

## Reporting security problems

Do not describe a vulnerability in a public issue or pull request. Open an issue that only asks the maintainers for a private way to report it, with no details, and we will get in touch.
