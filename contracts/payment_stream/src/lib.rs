#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env};

// ── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum StreamKey {
    Sender,
    Recipient,
    Token,
    RatePerSecond,
    StartTime,
    EndTime,
    TotalFunded,
    TotalWithdrawn,
    Status,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum StreamStatus {
    Active,
    Cancelled,
    Completed,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct PaymentStream;

#[contractimpl]
impl PaymentStream {
    /// Initialise the stream. Called once by StreamFactory after deployment.
    /// Transfers the full funded amount from sender into contract escrow.
    pub fn initialize(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        rate_per_second: i128,
        start_time: u64,
        end_time: u64,
        total_funded: i128,
    ) {
        sender.require_auth();

        assert!(start_time < end_time, "start_time must be before end_time");
        assert!(rate_per_second > 0, "rate must be greater than zero");
        assert!(total_funded > 0, "funded amount must be greater than zero");

        let storage = env.storage().persistent();
        storage.set(&StreamKey::Sender, &sender);
        storage.set(&StreamKey::Recipient, &recipient);
        storage.set(&StreamKey::Token, &token);
        storage.set(&StreamKey::RatePerSecond, &rate_per_second);
        storage.set(&StreamKey::StartTime, &start_time);
        storage.set(&StreamKey::EndTime, &end_time);
        storage.set(&StreamKey::TotalFunded, &total_funded);
        storage.set(&StreamKey::TotalWithdrawn, &0_i128);
        storage.set(&StreamKey::Status, &StreamStatus::Active);

        // Transfer funded tokens from sender into this contract
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &env.current_contract_address(), &total_funded);
    }

    /// Recipient withdraws all currently claimable tokens.
    pub fn withdraw(env: Env) {
        let recipient: Address = env.storage().persistent().get(&StreamKey::Recipient).unwrap();
        recipient.require_auth();

        Self::assert_stream_is_active(&env);

        let claimable = Self::calculate_claimable_balance(&env);
        assert!(claimable > 0, "nothing to withdraw");

        let total_withdrawn: i128 = env.storage().persistent().get(&StreamKey::TotalWithdrawn).unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::TotalWithdrawn, &(total_withdrawn + claimable));

        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &recipient, &claimable);

        Self::mark_completed_if_finished(&env);
    }

    /// Sender cancels the stream and reclaims all unstreamed tokens.
    pub fn cancel(env: Env) {
        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        sender.require_auth();

        Self::assert_stream_is_active(&env);

        // Pay out whatever the recipient has earned up to now
        let claimable = Self::calculate_claimable_balance(&env);
        let recipient: Address = env.storage().persistent().get(&StreamKey::Recipient).unwrap();
        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token);

        if claimable > 0 {
            let total_withdrawn: i128 =
                env.storage().persistent().get(&StreamKey::TotalWithdrawn).unwrap();
            env.storage()
                .persistent()
                .set(&StreamKey::TotalWithdrawn, &(total_withdrawn + claimable));
            token_client.transfer(&env.current_contract_address(), &recipient, &claimable);
        }

        // Return remaining balance to sender
        let total_funded: i128 = env.storage().persistent().get(&StreamKey::TotalFunded).unwrap();
        let total_withdrawn: i128 =
            env.storage().persistent().get(&StreamKey::TotalWithdrawn).unwrap();
        let remaining = total_funded - total_withdrawn;

        if remaining > 0 {
            token_client.transfer(&env.current_contract_address(), &sender, &remaining);
        }

        env.storage()
            .persistent()
            .set(&StreamKey::Status, &StreamStatus::Cancelled);
    }

    /// Sender tops up the stream with additional tokens, extending its duration.
    pub fn top_up(env: Env, additional_amount: i128) {
        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        sender.require_auth();

        Self::assert_stream_is_active(&env);
        assert!(additional_amount > 0, "top-up amount must be greater than zero");

        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &env.current_contract_address(), &additional_amount);

        let total_funded: i128 = env.storage().persistent().get(&StreamKey::TotalFunded).unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::TotalFunded, &(total_funded + additional_amount));

        // Extend end_time proportionally based on rate
        let rate: i128 = env.storage().persistent().get(&StreamKey::RatePerSecond).unwrap();
        let extra_seconds = (additional_amount / rate) as u64;
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::EndTime, &(end_time + extra_seconds));
    }

    /// Returns how many tokens the recipient can withdraw right now.
    pub fn get_claimable_balance(env: Env) -> i128 {
        Self::calculate_claimable_balance(&env)
    }

    /// Returns the current status of the stream.
    pub fn get_stream_status(env: Env) -> StreamStatus {
        env.storage().persistent().get(&StreamKey::Status).unwrap()
    }

    /// Returns the stream sender address.
    pub fn get_sender(env: Env) -> Address {
        env.storage().persistent().get(&StreamKey::Sender).unwrap()
    }

    /// Returns the stream recipient address.
    pub fn get_recipient(env: Env) -> Address {
        env.storage().persistent().get(&StreamKey::Recipient).unwrap()
    }

    /// Returns the total amount funded into the stream.
    pub fn get_total_funded(env: Env) -> i128 {
        env.storage().persistent().get(&StreamKey::TotalFunded).unwrap()
    }

    /// Returns the total amount withdrawn from the stream so far.
    pub fn get_total_withdrawn(env: Env) -> i128 {
        env.storage().persistent().get(&StreamKey::TotalWithdrawn).unwrap()
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Calculate how many tokens are claimable at the current ledger timestamp.
    /// claimable = (rate * elapsed) - already_withdrawn
    fn calculate_claimable_balance(env: &Env) -> i128 {
        let now = env.ledger().timestamp();
        let start_time: u64 = env.storage().persistent().get(&StreamKey::StartTime).unwrap();
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        let rate: i128 = env.storage().persistent().get(&StreamKey::RatePerSecond).unwrap();
        let total_funded: i128 = env.storage().persistent().get(&StreamKey::TotalFunded).unwrap();
        let total_withdrawn: i128 = env.storage().persistent().get(&StreamKey::TotalWithdrawn).unwrap();

        if now <= start_time {
            return 0;
        }

        let effective_end = now.min(end_time);
        let elapsed = (effective_end - start_time) as i128;
        let earned = (rate * elapsed).min(total_funded);

        (earned - total_withdrawn).max(0)
    }

    /// Panic if the stream is not in Active status.
    fn assert_stream_is_active(env: &Env) {
        let status: StreamStatus = env.storage().persistent().get(&StreamKey::Status).unwrap();
        assert!(status == StreamStatus::Active, "stream is not active");
    }

    /// Mark the stream as Completed if end_time has passed and balance is fully withdrawn.
    fn mark_completed_if_finished(env: &Env) {
        let now = env.ledger().timestamp();
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        let total_funded: i128 = env.storage().persistent().get(&StreamKey::TotalFunded).unwrap();
        let total_withdrawn: i128 = env.storage().persistent().get(&StreamKey::TotalWithdrawn).unwrap();

        if now >= end_time && total_withdrawn >= total_funded {
            env.storage()
                .persistent()
                .set(&StreamKey::Status, &StreamStatus::Completed);
        }
    }
}
