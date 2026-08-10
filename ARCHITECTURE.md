# Architecture — velox-contracts

This document describes the architectural design of the `velox-contracts` repository: the on-chain layer of the Velox protocol built with Soroban on the Stellar network.

---

## Overview

`velox-contracts` is a collection of composable Soroban smart contracts. Each contract has a single, well-defined responsibility. They communicate through explicit function calls and shared data types — never through shared mutable state.

The design follows a **factory + instance** pattern:
- A factory contract creates and tracks individual stream/schedule instances
- Each instance is an isolated contract with its own storage and lifecycle
- A registry contract provides discoverability across all instances

---

## System Context

```
┌─────────────────────────────────────────────────────────────┐
│                        Stellar Network                       │
│                                                             │
│   External Callers          Velox Contracts                 │
│   ─────────────────         ────────────────                │
│                                                             │
│   User Wallet    ─────────▶  StreamFactory                  │
│   velox-scheduler ────────▶  RecurringPayment               │
│   velox-frontend  ────────▶  PaymentStream (per instance)   │
│                             VeloxRegistry                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Contract Map

```
velox-contracts/
│
├── StreamFactory          # Entry point for creating streams
├── PaymentStream          # Per-stream instance (one per stream)
├── RecurringPayment       # Fixed-interval recurring payment schedules
└── VeloxRegistry          # On-chain index of all active streams/schedules
```

---

## Contract Responsibilities

### StreamFactory

**Responsibility:** Create and register new `PaymentStream` contract instances.

- Accepts stream parameters from the caller
- Deploys a new `PaymentStream` contract instance
- Registers the new stream in `VeloxRegistry`
- Returns the new stream's contract ID to the caller
- Does NOT manage stream state — that belongs to `PaymentStream`

```
Caller
  │
  ▼
StreamFactory.create_stream(sender, recipient, token, rate, start, end)
  │
  ├──▶ Deploy new PaymentStream instance
  │
  └──▶ VeloxRegistry.register_stream(stream_id, metadata)
```

---

### PaymentStream

**Responsibility:** Manage the full lifecycle of a single payment stream.

Each `PaymentStream` is an independent contract instance. It holds the streamed funds in escrow and releases them to the recipient based on elapsed time and the configured rate.

State machine:

```
         create_stream()
               │
               ▼
           [ Active ]
          /           \
  cancel()          withdraw()
      │                  │
      ▼                  ▼
 [Cancelled]        (partial or full)
                         │
                  end_time reached
                  + balance = zero
                         │
                         ▼
                    [Completed]
```

Key invariants:
- `claimable_balance = rate * elapsed_seconds`
- `elapsed_seconds = min(now, end_time) - start_time`
- Withdrawals never exceed the total funded amount
- Only the recipient can call `withdraw()`
- Only the sender can call `cancel()` or `top_up()`

---

### RecurringPayment

**Responsibility:** Manage fixed-interval payment schedules (e.g., weekly payroll).

Unlike `PaymentStream` which streams continuously, `RecurringPayment` releases discrete payments at defined intervals. It does not trigger itself — the `velox-scheduler` calls `execute_payment()` at each interval.

```
Caller (velox-scheduler)
  │
  ▼
RecurringPayment.execute_payment(schedule_id)
  │
  ├── Check: is payment due? (now >= next_payment_time)
  ├── Check: is schedule active?
  ├── Transfer token amount to recipient
  └── Update next_payment_time += interval
```

---

### VeloxRegistry

**Responsibility:** Maintain an on-chain index of all streams and schedules for discoverability.

- Acts as a lookup table — does not hold funds or enforce payment logic
- Used by `velox-scheduler` to discover active schedules
- Used by `velox-frontend` to display streams for a given wallet
- Written to by `StreamFactory` and `RecurringPayment` on creation

```
StreamFactory ──▶ VeloxRegistry.register_stream()
RecurringPayment ▶ VeloxRegistry.register_schedule()

velox-scheduler ──▶ VeloxRegistry.get_all_active()
velox-frontend  ──▶ VeloxRegistry.list_streams_by_sender(address)
```

---

## Data Flow: Creating a Stream

```
1. User calls StreamFactory.create_stream(params)
2. StreamFactory validates inputs
3. StreamFactory deploys a new PaymentStream instance
4. StreamFactory calls VeloxRegistry.register_stream(id, metadata)
5. StreamFactory returns stream_id to caller
6. User's tokens are transferred into the PaymentStream escrow
```

## Data Flow: Withdrawing from a Stream

```
1. Recipient calls PaymentStream.withdraw()
2. Contract calculates claimable_balance based on elapsed time
3. Contract transfers claimable_balance to recipient
4. Contract updates internal accounting (last_withdrawn_at)
5. If end_time has passed and balance is zero → status = Completed
```

## Data Flow: Recurring Payment Execution

```
1. velox-scheduler polls VeloxRegistry.get_all_active()
2. Scheduler filters schedules where next_payment_time <= now
3. Scheduler calls RecurringPayment.execute_payment(schedule_id)
4. Contract verifies payment is due
5. Contract transfers amount to recipient
6. Contract updates next_payment_time
```

---

## Storage Design

Each contract uses Soroban's key-value storage. Storage keys are typed enums to prevent collision and improve readability.

### PaymentStream Storage Keys

| Key | Type | Description |
|-----|------|-------------|
| `Sender` | `Address` | Stream creator |
| `Recipient` | `Address` | Payment receiver |
| `Token` | `Address` | Token contract address |
| `RatePerSecond` | `i128` | Tokens released per second |
| `StartTime` | `u64` | Unix timestamp |
| `EndTime` | `u64` | Unix timestamp |
| `TotalFunded` | `i128` | Total tokens deposited |
| `TotalWithdrawn` | `i128` | Total tokens withdrawn |
| `Status` | `StreamStatus` | Active / Cancelled / Completed |

### RecurringPayment Storage Keys

| Key | Type | Description |
|-----|------|-------------|
| `Sender` | `Address` | Schedule creator |
| `Recipient` | `Address` | Payment receiver |
| `Token` | `Address` | Token contract address |
| `Amount` | `i128` | Amount per interval |
| `Interval` | `u64` | Seconds between payments |
| `NextPaymentTime` | `u64` | Unix timestamp of next payment |
| `Status` | `ScheduleStatus` | Active / Cancelled / Completed |

---

## Security Considerations

### Authorization
- All sensitive functions use `require_auth()` on the caller
- `withdraw()` — authorized to recipient only
- `cancel()` — authorized to sender only
- `execute_payment()` — authorized to a designated operator address
- `register_stream()` on VeloxRegistry — authorized to StreamFactory only

### Reentrancy
- Soroban's execution model prevents reentrancy at the protocol level
- Internal state is updated before any token transfers as an additional safeguard

### Integer Overflow
- All arithmetic uses checked operations
- Token amounts use `i128` to accommodate large values with precision

### Time Manipulation
- `start_time` and `end_time` are validated on creation (`start < end`, `start >= now`)
- Ledger timestamp is used as the time source — not caller-provided values

---

## Testing Strategy

All contracts follow strict Test-Driven Development:

1. **Unit tests** — each function tested in isolation with mocked dependencies
2. **Integration tests** — full flow tests (create → withdraw → cancel) on a local Soroban environment
3. **Edge case tests** — zero amounts, expired streams, double-withdraw attempts, unauthorized callers
4. **Fuzz tests** — randomized inputs to surface arithmetic edge cases

Test files live alongside source files in `src/tests.rs` for each contract.

Target coverage: **100% of public functions**

---

## Dependency Graph

```
VeloxRegistry    (no dependencies on other Velox contracts)
PaymentStream    (no dependencies on other Velox contracts)
RecurringPayment (no dependencies on other Velox contracts)
StreamFactory    (depends on: PaymentStream, VeloxRegistry)
```

Contracts at the bottom have no Velox-internal dependencies — they can be deployed and tested independently. `StreamFactory` is the only contract with cross-contract dependencies.

---

## Deployment Order

```
1. Deploy VeloxRegistry
2. Deploy PaymentStream (WASM, used as template)
3. Deploy RecurringPayment
4. Deploy StreamFactory (with VeloxRegistry address as constructor arg)
```

---

> Each contract does one thing. Each function does one thing. The system is predictable because each piece is understandable in isolation.
