#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env,
};

// ── Events ────────────────────────────────────────────────────────────────────

#[contractevent]
pub struct ScheduleCreated {
    #[topic]
    pub sender: Address,
    #[topic]
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub interval: u64,
    pub first_payment_time: u64,
}

#[contractevent]
pub struct PaymentExecuted {
    #[topic]
    pub recipient: Address,
    pub amount: i128,
}

#[contractevent]
pub struct ScheduleCancelled {
    #[topic]
    pub sender: Address,
}

#[contractevent]
pub struct ScheduleCompleted {
    #[topic]
    pub sender: Address,
    pub payments_made: u32,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    AmountMustBePositive = 2,
    IntervalMustBePositive = 3,
    FirstPaymentInPast = 4,
    ScheduleNotActive = 5,
    PaymentNotDue = 6,
    ArithmeticOverflow = 7,
}

// ── Storage TTL ───────────────────────────────────────────────────────────────
//
// Soroban persistent entries expire after a number of ledgers and are
// inaccessible past that point, so a long-running schedule must have its
// storage TTL bumped periodically or it would lose its state entirely.
// Assuming a 5-second ledger close time: 1 day ≈ 17_280 ledgers.

/// Bump when the remaining TTL drops below ~7 days.
const TTL_THRESHOLD: u32 = 120_960;
/// Bump up to ~30 days of TTL remaining.
const TTL_EXTEND_TO: u32 = 518_400;

// ── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum ScheduleKey {
    Sender,
    Recipient,
    Token,
    Amount,
    Interval,
    NextPaymentTime,
    Status,
    TotalPayments,
    PaymentsMade,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum ScheduleStatus {
    Active,
    Cancelled,
    Completed,
}

/// A snapshot of the full schedule state returned by get_schedule_info.
#[contracttype]
#[derive(Clone)]
pub struct ScheduleInfo {
    pub sender: Address,
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    pub interval: u64,
    pub next_payment_time: u64,
    pub status: ScheduleStatus,
    pub total_payments: u32,
    pub payments_made: u32,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RecurringPayment;

#[contractimpl]
impl RecurringPayment {
    /// Initialise the recurring payment schedule.
    /// Called once by the sender to set the schedule parameters.
    ///
    /// The schedule pulls each payment from the sender via `transfer_from`, so the
    /// sender must also call `approve` on the token with this contract as spender.
    /// The approved amount caps the total the schedule can ever pay out.
    ///
    /// `total_payments` caps how many times `execute_payment` will succeed before
    /// the schedule moves itself to `Completed`; `0` means unlimited (the sender
    /// must `cancel()` to stop it).
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        interval: u64,
        first_payment_time: u64,
        total_payments: u32,
    ) -> Result<(), Error> {
        sender.require_auth();

        if env.storage().persistent().has(&ScheduleKey::Status) {
            return Err(Error::AlreadyInitialized);
        }
        if amount <= 0 {
            return Err(Error::AmountMustBePositive);
        }
        if interval == 0 {
            return Err(Error::IntervalMustBePositive);
        }
        if first_payment_time < env.ledger().timestamp() {
            return Err(Error::FirstPaymentInPast);
        }

        let storage = env.storage().persistent();
        storage.set(&ScheduleKey::Sender, &sender);
        storage.set(&ScheduleKey::Recipient, &recipient);
        storage.set(&ScheduleKey::Token, &token);
        storage.set(&ScheduleKey::Amount, &amount);
        storage.set(&ScheduleKey::Interval, &interval);
        storage.set(&ScheduleKey::NextPaymentTime, &first_payment_time);
        storage.set(&ScheduleKey::Status, &ScheduleStatus::Active);
        storage.set(&ScheduleKey::TotalPayments, &total_payments);
        storage.set(&ScheduleKey::PaymentsMade, &0_u32);

        ScheduleCreated {
            sender,
            recipient,
            token,
            amount,
            interval,
            first_payment_time,
        }
        .publish(&env);
        Ok(())
    }

    /// Execute a single scheduled payment.
    /// Called by the velox-scheduler daemon when payment is due.
    /// Transfers the fixed amount from sender to recipient using the sender's allowance.
    ///
    /// Permissionless: no signature is required because the call can only move the
    /// fixed amount, to the fixed recipient, once per interval.
    pub fn execute_payment(env: Env) -> Result<(), Error> {
        Self::assert_schedule_is_active(&env)?;

        let now = env.ledger().timestamp();
        let next_payment_time: u64 = env
            .storage()
            .persistent()
            .get(&ScheduleKey::NextPaymentTime)
            .unwrap();

        if now < next_payment_time {
            return Err(Error::PaymentNotDue);
        }

        let sender: Address = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Sender)
            .unwrap();
        let recipient: Address = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Recipient)
            .unwrap();
        let token: Address = env.storage().persistent().get(&ScheduleKey::Token).unwrap();
        let amount: i128 = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Amount)
            .unwrap();
        let interval: u64 = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Interval)
            .unwrap();

        // Advance next payment time before transferring, per the update-then-transfer rule
        let new_next_payment_time = next_payment_time
            .checked_add(interval)
            .ok_or(Error::ArithmeticOverflow)?;
        env.storage()
            .persistent()
            .set(&ScheduleKey::NextPaymentTime, &new_next_payment_time);

        let token_client = token::Client::new(&env, &token);
        token_client.transfer_from(
            &env.current_contract_address(),
            &sender,
            &recipient,
            &amount,
        );

        let total_payments: u32 = env
            .storage()
            .persistent()
            .get(&ScheduleKey::TotalPayments)
            .unwrap();
        let previous_payments_made: u32 = env
            .storage()
            .persistent()
            .get(&ScheduleKey::PaymentsMade)
            .unwrap();
        let payments_made: u32 = previous_payments_made + 1;
        env.storage()
            .persistent()
            .set(&ScheduleKey::PaymentsMade, &payments_made);

        if total_payments > 0 && payments_made >= total_payments {
            env.storage()
                .persistent()
                .set(&ScheduleKey::Status, &ScheduleStatus::Completed);
        }

        Self::extend_storage_ttl(&env);

        PaymentExecuted {
            recipient: recipient.clone(),
            amount,
        }
        .publish(&env);

        if total_payments > 0 && payments_made >= total_payments {
            ScheduleCompleted {
                sender,
                payments_made,
            }
            .publish(&env);
        }
        Ok(())
    }

    /// Sender cancels the recurring schedule. No further payments will be made.
    pub fn cancel(env: Env) -> Result<(), Error> {
        let sender: Address = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Sender)
            .unwrap();
        sender.require_auth();

        Self::assert_schedule_is_active(&env)?;

        env.storage()
            .persistent()
            .set(&ScheduleKey::Status, &ScheduleStatus::Cancelled);

        ScheduleCancelled { sender }.publish(&env);
        Ok(())
    }

    /// Returns the timestamp of the next scheduled payment.
    pub fn get_next_payment_time(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&ScheduleKey::NextPaymentTime)
            .unwrap()
    }

    /// Returns the current status of the schedule.
    pub fn get_schedule_status(env: Env) -> ScheduleStatus {
        env.storage()
            .persistent()
            .get(&ScheduleKey::Status)
            .unwrap()
    }

    /// Returns the payment amount per interval.
    pub fn get_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&ScheduleKey::Amount)
            .unwrap()
    }

    /// Returns the interval in seconds between payments.
    pub fn get_interval(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&ScheduleKey::Interval)
            .unwrap()
    }

    /// Returns a complete snapshot of all schedule fields in a single call.
    pub fn get_schedule_info(env: Env) -> ScheduleInfo {
        let storage = env.storage().persistent();
        ScheduleInfo {
            sender: storage.get(&ScheduleKey::Sender).unwrap(),
            recipient: storage.get(&ScheduleKey::Recipient).unwrap(),
            token: storage.get(&ScheduleKey::Token).unwrap(),
            amount: storage.get(&ScheduleKey::Amount).unwrap(),
            interval: storage.get(&ScheduleKey::Interval).unwrap(),
            next_payment_time: storage.get(&ScheduleKey::NextPaymentTime).unwrap(),
            status: storage.get(&ScheduleKey::Status).unwrap(),
            total_payments: storage.get(&ScheduleKey::TotalPayments).unwrap(),
            payments_made: storage.get(&ScheduleKey::PaymentsMade).unwrap(),
        }
    }

    /// Returns the total number of payments this schedule will ever make, or
    /// `0` if it is unlimited and only stops when the sender calls `cancel()`.
    pub fn get_total_payments(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&ScheduleKey::TotalPayments)
            .unwrap()
    }

    /// Returns how many payments have executed successfully so far.
    pub fn get_payments_made(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&ScheduleKey::PaymentsMade)
            .unwrap()
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Return an error if the schedule is not in Active status.
    fn assert_schedule_is_active(env: &Env) -> Result<(), Error> {
        let status: ScheduleStatus = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Status)
            .unwrap();
        if status != ScheduleStatus::Active {
            return Err(Error::ScheduleNotActive);
        }
        Ok(())
    }

    /// Bump the TTL of every storage entry this schedule owns, so a
    /// long-running schedule that keeps executing payments never loses its
    /// state to Soroban's persistent-entry expiry.
    fn extend_storage_ttl(env: &Env) {
        let storage = env.storage().persistent();
        storage.extend_ttl(&ScheduleKey::Sender, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::Recipient, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::Token, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::Amount, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::Interval, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::NextPaymentTime, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::Status, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::TotalPayments, TTL_THRESHOLD, TTL_EXTEND_TO);
        storage.extend_ttl(&ScheduleKey::PaymentsMade, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

#[cfg(test)]
mod tests;
