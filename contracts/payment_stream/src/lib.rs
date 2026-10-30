#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    StartTimeAfterEndTime = 2,
    StartTimeInPast = 3,
    RateMustBePositive = 4,
    FundedAmountMustBePositive = 5,
    StreamNotActive = 6,
    NothingToWithdraw = 7,
    TopUpAmountMustBePositive = 8,
    StreamNotPaused = 9,
}

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
    /// Ledger timestamp when pause_stream was called. Only set while Paused;
    /// used as a frozen "now" so claimable balance stops growing, and to compute
    /// how long the stream was paused so resume_stream can shift start/end by it.
    PausedAt,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum StreamStatus {
    Active,
    Cancelled,
    Completed,
    // Appended, not inserted before Cancelled/Completed: an unadorned #[contracttype]
    // enum encodes variants by declaration order, so inserting one earlier would
    // silently renumber the later variants and corrupt any already-stored status.
    Paused,
}

/// A snapshot of the full stream state returned by get_stream_info.
/// Allows callers to read all stream fields in a single contract call.
#[contracttype]
#[derive(Clone)]
pub struct StreamInfo {
    pub sender: Address,
    pub recipient: Address,
    pub token: Address,
    pub rate_per_second: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub total_funded: i128,
    pub total_withdrawn: i128,
    pub status: StreamStatus,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct PaymentStream;

#[contractimpl]
impl PaymentStream {
    /// Initialise the stream. Called once by StreamFactory after deployment.
    /// Transfers the full funded amount from sender into contract escrow.
    // One parameter per stream field: Soroban contract functions take typed scalar
    // arguments, not a struct, so there is no fewer-argument alternative here.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        rate_per_second: i128,
        start_time: u64,
        end_time: u64,
        total_funded: i128,
    ) -> Result<(), Error> {
        sender.require_auth();

        if env.storage().persistent().has(&StreamKey::Status) {
            return Err(Error::AlreadyInitialized);
        }
        if start_time >= end_time {
            return Err(Error::StartTimeAfterEndTime);
        }
        if start_time < env.ledger().timestamp() {
            return Err(Error::StartTimeInPast);
        }
        if rate_per_second <= 0 {
            return Err(Error::RateMustBePositive);
        }
        if total_funded <= 0 {
            return Err(Error::FundedAmountMustBePositive);
        }

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
        token_client.transfer(&sender, env.current_contract_address(), &total_funded);
        Ok(())
    }

    /// Recipient withdraws all currently claimable tokens.
    pub fn withdraw(env: Env) -> Result<(), Error> {
        let recipient: Address = env
            .storage()
            .persistent()
            .get(&StreamKey::Recipient)
            .unwrap();
        recipient.require_auth();

        Self::assert_stream_is_active(&env)?;

        let claimable = Self::calculate_claimable_balance(&env);
        if claimable <= 0 {
            return Err(Error::NothingToWithdraw);
        }

        let total_withdrawn: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalWithdrawn)
            .unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::TotalWithdrawn, &(total_withdrawn + claimable));

        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &recipient, &claimable);

        Self::mark_completed_if_finished(&env);
        Ok(())
    }

    /// Sender cancels the stream and reclaims all unstreamed tokens.
    /// Pays out any earned balance to the recipient first, then returns the rest to sender.
    pub fn cancel(env: Env) -> Result<(), Error> {
        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        sender.require_auth();

        Self::assert_stream_is_active(&env)?;
        Self::pay_out_earned_balance(&env);
        Self::return_remaining_to_sender(&env);

        env.storage()
            .persistent()
            .set(&StreamKey::Status, &StreamStatus::Cancelled);
        Ok(())
    }

    /// Sender tops up the stream with additional tokens, extending its duration.
    pub fn top_up(env: Env, additional_amount: i128) -> Result<(), Error> {
        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        sender.require_auth();

        Self::assert_stream_is_active(&env)?;
        if additional_amount <= 0 {
            return Err(Error::TopUpAmountMustBePositive);
        }

        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, env.current_contract_address(), &additional_amount);

        let total_funded: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalFunded)
            .unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::TotalFunded, &(total_funded + additional_amount));

        // Extend end_time proportionally based on rate
        let rate: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::RatePerSecond)
            .unwrap();
        let extra_seconds = (additional_amount / rate) as u64;
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::EndTime, &(end_time + extra_seconds));
        Ok(())
    }

    /// Sender pauses the stream. Claimable balance stops growing until resume_stream.
    pub fn pause_stream(env: Env) -> Result<(), Error> {
        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        sender.require_auth();

        Self::assert_stream_is_active(&env)?;

        let now = env.ledger().timestamp();
        env.storage().persistent().set(&StreamKey::PausedAt, &now);
        env.storage()
            .persistent()
            .set(&StreamKey::Status, &StreamStatus::Paused);
        Ok(())
    }

    /// Sender resumes a paused stream. Shifts start_time and end_time forward by
    /// however long the stream was paused, so the remaining duration and rate are
    /// unchanged and the paused interval never counts as elapsed time.
    pub fn resume_stream(env: Env) -> Result<(), Error> {
        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        sender.require_auth();

        let status: StreamStatus = env.storage().persistent().get(&StreamKey::Status).unwrap();
        if status != StreamStatus::Paused {
            return Err(Error::StreamNotPaused);
        }

        let paused_at: u64 = env
            .storage()
            .persistent()
            .get(&StreamKey::PausedAt)
            .unwrap();
        let now = env.ledger().timestamp();
        let paused_duration = now - paused_at;

        let start_time: u64 = env
            .storage()
            .persistent()
            .get(&StreamKey::StartTime)
            .unwrap();
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        env.storage()
            .persistent()
            .set(&StreamKey::StartTime, &(start_time + paused_duration));
        env.storage()
            .persistent()
            .set(&StreamKey::EndTime, &(end_time + paused_duration));

        env.storage().persistent().remove(&StreamKey::PausedAt);
        env.storage()
            .persistent()
            .set(&StreamKey::Status, &StreamStatus::Active);
        Ok(())
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
        env.storage()
            .persistent()
            .get(&StreamKey::Recipient)
            .unwrap()
    }

    /// Returns the total amount funded into the stream.
    pub fn get_total_funded(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&StreamKey::TotalFunded)
            .unwrap()
    }

    /// Returns the total amount withdrawn from the stream so far.
    pub fn get_total_withdrawn(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&StreamKey::TotalWithdrawn)
            .unwrap()
    }

    /// Returns a complete snapshot of all stream fields in a single call.
    /// Preferred over calling individual getters when multiple fields are needed.
    pub fn get_stream_info(env: Env) -> StreamInfo {
        let storage = env.storage().persistent();
        StreamInfo {
            sender: storage.get(&StreamKey::Sender).unwrap(),
            recipient: storage.get(&StreamKey::Recipient).unwrap(),
            token: storage.get(&StreamKey::Token).unwrap(),
            rate_per_second: storage.get(&StreamKey::RatePerSecond).unwrap(),
            start_time: storage.get(&StreamKey::StartTime).unwrap(),
            end_time: storage.get(&StreamKey::EndTime).unwrap(),
            total_funded: storage.get(&StreamKey::TotalFunded).unwrap(),
            total_withdrawn: storage.get(&StreamKey::TotalWithdrawn).unwrap(),
            status: storage.get(&StreamKey::Status).unwrap(),
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Transfer any balance the recipient has earned up to this moment.
    /// Called during cancellation before returning funds to sender.
    fn pay_out_earned_balance(env: &Env) {
        let claimable = Self::calculate_claimable_balance(env);
        if claimable <= 0 {
            return;
        }

        let recipient: Address = env
            .storage()
            .persistent()
            .get(&StreamKey::Recipient)
            .unwrap();
        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let total_withdrawn: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalWithdrawn)
            .unwrap();

        env.storage()
            .persistent()
            .set(&StreamKey::TotalWithdrawn, &(total_withdrawn + claimable));

        let token_client = token::Client::new(env, &token);
        token_client.transfer(&env.current_contract_address(), &recipient, &claimable);
    }

    /// Transfer all unstreamed tokens back to the sender.
    /// Called during cancellation after paying out the recipient's earned balance.
    fn return_remaining_to_sender(env: &Env) {
        let total_funded: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalFunded)
            .unwrap();
        let total_withdrawn: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalWithdrawn)
            .unwrap();
        let remaining = total_funded - total_withdrawn;

        if remaining <= 0 {
            return;
        }

        let sender: Address = env.storage().persistent().get(&StreamKey::Sender).unwrap();
        let token: Address = env.storage().persistent().get(&StreamKey::Token).unwrap();
        let token_client = token::Client::new(env, &token);
        token_client.transfer(&env.current_contract_address(), &sender, &remaining);
    }

    /// Calculate how many tokens are claimable at the current ledger timestamp.
    /// claimable = (rate * elapsed) - already_withdrawn
    fn calculate_claimable_balance(env: &Env) -> i128 {
        // While paused, freeze "now" at the moment pause_stream was called, so
        // claimable balance stops growing until resume_stream shifts the schedule.
        let status: StreamStatus = env.storage().persistent().get(&StreamKey::Status).unwrap();
        let now = if status == StreamStatus::Paused {
            env.storage()
                .persistent()
                .get(&StreamKey::PausedAt)
                .unwrap()
        } else {
            env.ledger().timestamp()
        };
        let start_time: u64 = env
            .storage()
            .persistent()
            .get(&StreamKey::StartTime)
            .unwrap();
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        let rate: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::RatePerSecond)
            .unwrap();
        let total_funded: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalFunded)
            .unwrap();
        let total_withdrawn: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalWithdrawn)
            .unwrap();

        if now <= start_time {
            return 0;
        }

        let effective_end = now.min(end_time);
        let elapsed = (effective_end - start_time) as i128;
        let earned = (rate * elapsed).min(total_funded);

        (earned - total_withdrawn).max(0)
    }

    /// Return an error if the stream is not in Active status.
    fn assert_stream_is_active(env: &Env) -> Result<(), Error> {
        let status: StreamStatus = env.storage().persistent().get(&StreamKey::Status).unwrap();
        if status != StreamStatus::Active {
            return Err(Error::StreamNotActive);
        }
        Ok(())
    }

    /// Mark the stream as Completed if end_time has passed and balance is fully withdrawn.
    fn mark_completed_if_finished(env: &Env) {
        let now = env.ledger().timestamp();
        let end_time: u64 = env.storage().persistent().get(&StreamKey::EndTime).unwrap();
        let total_funded: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalFunded)
            .unwrap();
        let total_withdrawn: i128 = env
            .storage()
            .persistent()
            .get(&StreamKey::TotalWithdrawn)
            .unwrap();

        if now >= end_time && total_withdrawn >= total_funded {
            env.storage()
                .persistent()
                .set(&StreamKey::Status, &StreamStatus::Completed);
        }
    }
}

#[cfg(test)]
mod tests;
