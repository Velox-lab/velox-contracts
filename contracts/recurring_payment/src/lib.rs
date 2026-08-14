#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};

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
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RecurringPayment;

#[contractimpl]
impl RecurringPayment {
    /// Initialise the recurring payment schedule.
    /// Called once by the sender to set the schedule parameters.
    pub fn initialize(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        interval: u64,
        first_payment_time: u64,
    ) {
        sender.require_auth();

        assert!(amount > 0, "amount must be greater than zero");
        assert!(interval > 0, "interval must be greater than zero");
        assert!(
            first_payment_time >= env.ledger().timestamp(),
            "first payment time must be in the future"
        );

        let storage = env.storage().persistent();
        storage.set(&ScheduleKey::Sender, &sender);
        storage.set(&ScheduleKey::Recipient, &recipient);
        storage.set(&ScheduleKey::Token, &token);
        storage.set(&ScheduleKey::Amount, &amount);
        storage.set(&ScheduleKey::Interval, &interval);
        storage.set(&ScheduleKey::NextPaymentTime, &first_payment_time);
        storage.set(&ScheduleKey::Status, &ScheduleStatus::Active);
    }

    /// Execute a single scheduled payment.
    /// Called by the velox-scheduler daemon when payment is due.
    /// Transfers the fixed amount from sender to recipient.
    pub fn execute_payment(env: Env) {
        Self::assert_schedule_is_active(&env);

        let now = env.ledger().timestamp();
        let next_payment_time: u64 = env
            .storage()
            .persistent()
            .get(&ScheduleKey::NextPaymentTime)
            .unwrap();

        assert!(now >= next_payment_time, "payment is not due yet");

        let sender: Address = env.storage().persistent().get(&ScheduleKey::Sender).unwrap();
        let recipient: Address = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Recipient)
            .unwrap();
        let token: Address = env.storage().persistent().get(&ScheduleKey::Token).unwrap();
        let amount: i128 = env.storage().persistent().get(&ScheduleKey::Amount).unwrap();
        let interval: u64 = env.storage().persistent().get(&ScheduleKey::Interval).unwrap();

        sender.require_auth();

        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &recipient, &amount);

        // Advance next payment time by one interval
        env.storage()
            .persistent()
            .set(&ScheduleKey::NextPaymentTime, &(next_payment_time + interval));
    }

    /// Sender cancels the recurring schedule. No further payments will be made.
    pub fn cancel(env: Env) {
        let sender: Address = env.storage().persistent().get(&ScheduleKey::Sender).unwrap();
        sender.require_auth();

        Self::assert_schedule_is_active(&env);

        env.storage()
            .persistent()
            .set(&ScheduleKey::Status, &ScheduleStatus::Cancelled);
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
        env.storage().persistent().get(&ScheduleKey::Amount).unwrap()
    }

    /// Returns the interval in seconds between payments.
    pub fn get_interval(env: Env) -> u64 {
        env.storage().persistent().get(&ScheduleKey::Interval).unwrap()
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
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Panic if the schedule is not in Active status.
    fn assert_schedule_is_active(env: &Env) {
        let status: ScheduleStatus = env
            .storage()
            .persistent()
            .get(&ScheduleKey::Status)
            .unwrap();
        assert!(status == ScheduleStatus::Active, "schedule is not active");
    }
}
