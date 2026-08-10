#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };
    use crate::{PaymentStream, PaymentStreamClient, StreamStatus};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn create_env() -> Env {
        Env::default()
    }

    fn register_contract(env: &Env) -> Address {
        env.register_contract(None, PaymentStream)
    }

    /// Sets up a basic stream: 10 tokens/sec, 100 seconds, 1000 total funded.
    fn setup_stream(env: &Env, client: &PaymentStreamClient) -> (Address, Address, Address) {
        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let token = Address::generate(env);

        env.mock_all_auths();
        env.ledger().set_timestamp(1000);

        client.initialize(
            &sender,
            &recipient,
            &token,
            &10_i128,   // rate per second
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
    #[should_panic(expected = "start_time must be before end_time")]
    fn initialize_panics_when_start_time_after_end_time() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&sender, &recipient, &token, &10, &2000_u64, &1000_u64, &1000);
    }

    #[test]
    #[should_panic(expected = "rate must be greater than zero")]
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

    // ── get_claimable_balance ─────────────────────────────────────────────────

    #[test]
    fn claimable_balance_is_zero_before_start_time() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(500); // before start
        assert_eq!(client.get_claimable_balance(), 0_i128);
    }

    #[test]
    fn claimable_balance_reflects_elapsed_time() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(1050); // 50 seconds in
        assert_eq!(client.get_claimable_balance(), 500_i128); // 10/sec * 50
    }

    #[test]
    fn claimable_balance_is_capped_at_total_funded() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(9999); // far past end_time
        assert_eq!(client.get_claimable_balance(), 1000_i128); // capped at funded
    }

    // ── withdraw ──────────────────────────────────────────────────────────────

    #[test]
    fn withdraw_reduces_claimable_balance_to_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(1050);
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

        env.ledger().set_timestamp(1050);
        env.mock_all_auths();
        client.withdraw();

        assert_eq!(client.get_total_withdrawn(), 500_i128);
    }

    #[test]
    #[should_panic(expected = "nothing to withdraw")]
    fn withdraw_panics_when_nothing_is_claimable() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(1000); // at start, nothing earned yet
        env.mock_all_auths();
        client.withdraw();
    }

    // ── cancel ────────────────────────────────────────────────────────────────

    #[test]
    fn cancel_sets_status_to_cancelled() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(1050);
        env.mock_all_auths();
        client.cancel();

        assert_eq!(client.get_stream_status(), StreamStatus::Cancelled);
    }

    #[test]
    #[should_panic(expected = "stream is not active")]
    fn cancel_panics_on_already_cancelled_stream() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.ledger().set_timestamp(1050);
        env.mock_all_auths();
        client.cancel();
        client.cancel(); // second cancel should panic
    }

    // ── top_up ────────────────────────────────────────────────────────────────

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
    #[should_panic(expected = "top-up amount must be greater than zero")]
    fn top_up_panics_when_amount_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = PaymentStreamClient::new(&env, &contract_id);

        setup_stream(&env, &client);

        env.mock_all_auths();
        client.top_up(&0_i128);
    }
}
