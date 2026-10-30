use crate::{Error, PaymentStream, PaymentStreamClient, StreamStatus};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

// ── Helpers ───────────────────────────────────────────────────────────────

fn create_env() -> Env {
    Env::default()
}

fn register_contract(env: &Env) -> Address {
    env.register(PaymentStream, ())
}

/// Deploys a test token and mints `amount` to `holder`. Returns the token address.
fn create_funded_token(env: &Env, holder: &Address, amount: i128) -> Address {
    let admin = Address::generate(env);
    let token = env.register_stellar_asset_contract_v2(admin).address();
    env.mock_all_auths();
    StellarAssetClient::new(env, &token).mint(holder, &amount);
    token
}

/// Sets up a basic stream: 10 tokens/sec, 100 seconds, 1000 total funded.
/// The sender is minted 10_000 tokens so top-ups can be tested.
fn setup_stream(env: &Env, client: &PaymentStreamClient) -> (Address, Address, Address) {
    let sender = Address::generate(env);
    let recipient = Address::generate(env);
    let token = create_funded_token(env, &sender, 10_000);

    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 1000);

    client.initialize(
        &sender, &recipient, &token, &10_i128,   // rate per second
        &1000_u64,  // start_time
        &1100_u64,  // end_time (100 seconds)
        &1000_i128, // total_funded
    );

    (sender, recipient, token)
}

// ── initialize ────────────────────────────────────────────────────────────

#[test]
fn initialize_sets_stream_to_active() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    assert_eq!(client.get_stream_status(), StreamStatus::Active);
}

#[test]
fn initialize_records_correct_total_funded() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    assert_eq!(client.get_total_funded(), 1000_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn initialize_panics_when_called_twice() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let (_sender, _recipient, token) = setup_stream(&env, &client);

    // An attacker tries to overwrite the stream with themselves as recipient
    let attacker = Address::generate(&env);
    client.initialize(
        &attacker, &attacker, &token, &10, &1000_u64, &1100_u64, &1000,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn initialize_panics_when_start_time_is_in_the_past() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 5000);
    client.initialize(
        &sender, &recipient, &token, &10, &1000_u64, &6000_u64, &1000,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn initialize_panics_when_start_time_after_end_time() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(
        &sender, &recipient, &token, &10, &2000_u64, &1000_u64, &1000,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn initialize_panics_when_rate_is_zero() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&sender, &recipient, &token, &0, &1000_u64, &2000_u64, &1000);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn initialize_panics_when_funded_amount_is_zero() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&sender, &recipient, &token, &10, &1000_u64, &2000_u64, &0);
}

#[test]
fn initialize_returns_typed_error_when_rate_is_zero() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    let result =
        client.try_initialize(&sender, &recipient, &token, &0, &1000_u64, &2000_u64, &1000);

    assert_eq!(result, Err(Ok(Error::RateMustBePositive)));
}

// ── get_claimable_balance ─────────────────────────────────────────────────

#[test]
fn claimable_balance_is_zero_before_start_time() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 500); // before start
    assert_eq!(client.get_claimable_balance(), 0_i128);
}

#[test]
fn claimable_balance_reflects_elapsed_time() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050); // 50 seconds in
    assert_eq!(client.get_claimable_balance(), 500_i128); // 10/sec * 50
}

#[test]
fn claimable_balance_is_capped_at_total_funded() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 9999); // far past end_time
    assert_eq!(client.get_claimable_balance(), 1000_i128); // capped at funded
}

// ── withdraw ──────────────────────────────────────────────────────────────

#[test]
fn withdraw_reduces_claimable_balance_to_zero() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050);
    env.mock_all_auths();
    client.withdraw();

    assert_eq!(client.get_claimable_balance(), 0_i128);
}

#[test]
fn withdraw_updates_total_withdrawn() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050);
    env.mock_all_auths();
    client.withdraw();

    assert_eq!(client.get_total_withdrawn(), 500_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn withdraw_panics_when_nothing_is_claimable() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1000); // at start, nothing earned yet
    env.mock_all_auths();
    client.withdraw();
}

#[test]
fn withdraw_returns_typed_error_when_nothing_is_claimable() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1000);
    env.mock_all_auths();
    let result = client.try_withdraw();

    assert_eq!(result, Err(Ok(Error::NothingToWithdraw)));
}

// ── cancel ────────────────────────────────────────────────────────────────

#[test]
fn cancel_sets_status_to_cancelled() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050);
    env.mock_all_auths();
    client.cancel();

    assert_eq!(client.get_stream_status(), StreamStatus::Cancelled);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn cancel_panics_on_already_cancelled_stream() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050);
    env.mock_all_auths();
    client.cancel();
    client.cancel(); // second cancel should panic
}

// ── get_stream_info ───────────────────────────────────────────────────────

#[test]
fn get_stream_info_returns_all_fields_correctly() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    let (sender, recipient, token) = setup_stream(&env, &client);

    let info = client.get_stream_info();
    assert_eq!(info.sender, sender);
    assert_eq!(info.recipient, recipient);
    assert_eq!(info.token, token);
    assert_eq!(info.rate_per_second, 10_i128);
    assert_eq!(info.start_time, 1000_u64);
    assert_eq!(info.end_time, 1100_u64);
    assert_eq!(info.total_funded, 1000_i128);
    assert_eq!(info.total_withdrawn, 0_i128);
}

#[test]
fn get_stream_info_reflects_updated_total_withdrawn_after_withdraw() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050);
    env.mock_all_auths();
    client.withdraw();

    let info = client.get_stream_info();
    assert_eq!(info.total_withdrawn, 500_i128);
}

#[test]
fn top_up_increases_total_funded() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    client.top_up(&500_i128);

    assert_eq!(client.get_total_funded(), 1500_i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn top_up_panics_when_amount_is_zero() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    client.top_up(&0_i128);
}

// ── pause_stream / resume_stream ──────────────────────────────────────────

#[test]
fn pause_stream_transitions_active_to_paused() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    client.pause_stream();

    assert_eq!(client.get_stream_status(), StreamStatus::Paused);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn pause_stream_panics_when_already_paused() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    client.pause_stream();
    client.pause_stream();
}

#[test]
fn pause_stream_requires_sender_auth() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);
    let (sender, _recipient, _token) = setup_stream(&env, &client);

    env.mock_all_auths();
    client.pause_stream();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, sender);
}

#[test]
fn resume_stream_transitions_paused_to_active() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    client.pause_stream();
    client.resume_stream();

    assert_eq!(client.get_stream_status(), StreamStatus::Active);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn resume_stream_panics_when_not_paused() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    client.resume_stream(); // still Active, never paused
}

#[test]
fn resume_stream_returns_typed_error_when_not_paused() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.mock_all_auths();
    let result = client.try_resume_stream();

    assert_eq!(result, Err(Ok(Error::StreamNotPaused)));
}

#[test]
fn claimable_balance_stops_growing_while_paused() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client); // start=1000, end=1100, rate=10/s

    env.ledger().with_mut(|li| li.timestamp = 1050); // 50s elapsed, 500 earned
    env.mock_all_auths();
    client.pause_stream();
    assert_eq!(client.get_claimable_balance(), 500_i128);

    // Advance real time by 200s while paused — claimable must not move
    env.ledger().with_mut(|li| li.timestamp = 1250);
    assert_eq!(client.get_claimable_balance(), 500_i128);
}

#[test]
fn claimable_balance_resumes_growing_after_resume_with_no_gap_or_jump() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client); // start=1000, end=1100, rate=10/s

    env.ledger().with_mut(|li| li.timestamp = 1050); // 500 earned
    env.mock_all_auths();
    client.pause_stream();

    env.ledger().with_mut(|li| li.timestamp = 1250); // paused for 200s
    client.resume_stream();

    // Resuming must not grant or lose anything for the paused interval
    assert_eq!(client.get_claimable_balance(), 500_i128);

    // 30 more seconds of real activity after resume earns 30 more
    env.ledger().with_mut(|li| li.timestamp = 1280);
    assert_eq!(client.get_claimable_balance(), 800_i128);

    // The stream still ends 100 total active seconds after it started,
    // just shifted later by however long it was paused (100 - 50 = 50 remain)
    let info = client.get_stream_info();
    assert_eq!(info.end_time, 1100 + 200);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn withdraw_panics_while_paused() {
    let env = create_env();
    let contract_id = register_contract(&env);
    let client = PaymentStreamClient::new(&env, &contract_id);

    setup_stream(&env, &client);

    env.ledger().with_mut(|li| li.timestamp = 1050);
    env.mock_all_auths();
    client.pause_stream();
    client.withdraw();
}
