# velox-contracts

> Soroban smart contracts powering streaming and recurring payments on the Stellar network.

---

## What Is This?

`velox-contracts` is the on-chain backbone of the Velox protocol. It contains a suite of auditable, composable Soroban smart contracts that enforce the rules of streaming and recurring payments directly on the Stellar blockchain.

No intermediaries. No manual triggers. No trust assumptions.

A payment stream is defined once — the amount, the recipient, the rate, the duration — and the contract enforces it. Contributors, DAOs, subscription platforms, and payroll systems can all build on top of this layer with confidence.

---

## Why It Exists

Stellar is fast, cheap, and built for payments. But it has no native primitive for *continuous* or *scheduled* payments. Every team that needs recurring payments on Stellar today builds their own fragile, off-chain solution.

`velox-contracts` changes that. It brings a standardized, tested, and open payment streaming primitive to the Stellar ecosystem — one that any developer can integrate, audit, and extend.

---

## Core Contracts

### `StreamFactory`
Creates and registers new payment streams. Each stream is a self-contained contract instance with its own state.

- `create_stream(sender, recipient, token, amount_per_second, start_time, end_time)`
- `get_stream(stream_id)`
- `list_streams_by_sender(sender)`
- `list_streams_by_recipient(recipient)`

### `PaymentStream`
The core streaming contract. Holds funds in escrow and releases them to the recipient at a defined rate.

- `initialize(sender, recipient, token, rate, start_time, end_time)`
- `withdraw(recipient)` — recipient claims accrued balance
- `cancel(sender)` — sender cancels and reclaims unstreamed funds
- `top_up(sender, amount)` — sender adds more funds to extend the stream
- `get_claimable_balance(recipient)` — view how much is currently withdrawable
- `get_stream_status()` — returns `Active`, `Paused`, `Completed`, or `Cancelled`

### `RecurringPayment`
Handles fixed-interval recurring payments (e.g., weekly payroll, monthly subscriptions).

- `create_schedule(sender, recipient, token, amount, interval, start_time)`
- `execute_payment(schedule_id)` — called by the scheduler daemon at each interval
- `cancel_schedule(sender, schedule_id)`
- `get_next_payment_time(schedule_id)`

### `VeloxRegistry`
A lightweight on-chain registry that indexes all active streams and schedules for discoverability.

- `register_stream(stream_id, metadata)`
- `register_schedule(schedule_id, metadata)`
- `get_all_active()`

---

## Architecture

```
┌─────────────────────────────────────────────┐
│                  Stellar Network             │
│                                             │
│   ┌─────────────┐    ┌──────────────────┐   │
│   │ StreamFactory│───▶│  PaymentStream   │   │
│   └─────────────┘    │  (per stream)    │   │
│                      └──────────────────┘   │
│   ┌──────────────────┐                      │
│   │ RecurringPayment │                      │
│   └──────────────────┘                      │
│   ┌──────────────────┐                      │
│   │  VeloxRegistry   │                      │
│   └──────────────────┘                      │
└─────────────────────────────────────────────┘
```

---

## Development Principles

This project is built with discipline. Every contribution must respect the following:

### Test-Driven Development (TDD)
Every function is written test-first. No contract logic is merged without a corresponding test that was written before the implementation. The test suite is the specification.

```
Write a failing test → Write the minimum code to pass it → Refactor → Repeat
```

### SOLID Principles
- **Single Responsibility** — Each contract does one thing. `StreamFactory` creates streams. `PaymentStream` manages a stream. They do not cross boundaries.
- **Open/Closed** — Contracts are open for extension (new stream types) but closed for modification of core logic.
- **Liskov Substitution** — Any stream type can be substituted where a base stream is expected.
- **Interface Segregation** — Contracts expose only the methods relevant to their role.
- **Dependency Inversion** — High-level contracts depend on abstractions, not concrete implementations.

### Clean Code Standards
- Methods are named after exactly what they do — `get_claimable_balance`, not `calc` or `fetch`
- Methods do one thing and one thing only
- No magic numbers — all constants are named and documented
- Every public function has inline documentation

---

## Tech Stack

| Tool | Purpose |
|------|---------|
| [Soroban SDK](https://soroban.stellar.org) | Smart contract framework |
| Rust | Contract implementation language |
| `soroban-cli` | Local development and deployment |
| `cargo test` | Unit and integration testing |
| Stellar Testnet | Staging environment |

---

## Getting Started

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Soroban CLI
cargo install --locked soroban-cli

# Add the Wasm target
rustup target add wasm32-unknown-unknown
```

### Clone & Build

```bash
git clone https://github.com/Velox-lab/velox-contracts.git
cd velox-contracts
cargo build
```

### Run Tests

```bash
cargo test
```

### Deploy to Testnet

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/payment_stream.wasm \
  --network testnet \
  --source <your-testnet-account>
```

---

## Project Structure

```
velox-contracts/
├── contracts/
│   ├── stream_factory/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   └── tests.rs
│   │   └── Cargo.toml
│   ├── payment_stream/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   └── tests.rs
│   │   └── Cargo.toml
│   ├── recurring_payment/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   └── tests.rs
│   │   └── Cargo.toml
│   └── velox_registry/
│       ├── src/
│       │   ├── lib.rs
│       │   └── tests.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

---

## Contributing

This repository is part of the **Velox** open-source project on the Stellar ecosystem. Contributions are welcome and rewarded through the Stellar Wave Program.

**Before you contribute:**
1. Read the [Contributing Guide](CONTRIBUTING.md)
2. Check open issues labeled `good first issue` or `help wanted`
3. Follow the TDD workflow — tests before implementation
4. Ensure `cargo test` passes before submitting a PR
5. One concern per PR — keep changes focused

**Good first issues include:**
- Adding a new stream status transition
- Writing tests for edge cases in `get_claimable_balance`
- Implementing `pause_stream` and `resume_stream`
- Adding events/logs to contract functions

---

## Roadmap

- [x] Project scaffold and architecture design
- [ ] `PaymentStream` contract — core implementation
- [ ] `StreamFactory` contract
- [ ] `RecurringPayment` contract
- [ ] `VeloxRegistry` contract
- [ ] Full test coverage (target: 100%)
- [ ] Testnet deployment
- [ ] Security audit
- [ ] Mainnet deployment

---

## License

MIT — free to use, modify, and build upon.

---

> Built for the Stellar ecosystem. Powered by the community.
