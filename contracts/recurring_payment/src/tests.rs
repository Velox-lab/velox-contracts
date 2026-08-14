#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };
    use crate::{RecurringPayment, RecurringPaymentClient, ScheduleStatus};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn create_env() -> Env {
        Env::default()
    }

    fn register_contract(env: &Env) -> Address {
        env.register_contract(None, RecurringPayment)
    }

    /// Sets up a weekly schedule: 100 tokens every 604800 seconds (7 days).
    fn setup_schedule(env: &Env, client: &RecurringPaymentClient) -> (Address, Address, Address) {
        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let token = Address::generate(env);

        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);

        client.initialize(
            &sender,
            &recipient,
            &token,
            &100_i128,          // amount per interval
            &604_800_u64,       // interval: 7 days in seconds
            &1_604_800_u64,     // first payment in ~7 days
        );

        (sender, recipient, token)
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn initialize_sets_schedule_to_active() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        assert_eq!(client.get_schedule_status(), ScheduleStatus::Active);
    }

    #[test]
    fn initialize_stores_correct_amount() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        assert_eq!(client.get_amount(), 100_i128);
    }

    #[test]
    fn initialize_stores_correct_interval() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        assert_eq!(client.get_interval(), 604_800_u64);
    }

    #[test]
    #[should_panic(expected = "amount must be greater than zero")]
    fn initialize_panics_when_amount_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);
        client.initialize(&sender, &recipient, &token, &0, &604_800_u64, &1_604_800_u64);
    }

    #[test]
    #[should_panic(expected = "interval must be greater than zero")]
    fn initialize_panics_when_interval_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token = Address::generate(&env);

        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);
        client.initialize(&sender, &recipient, &token, &100, &0_u64, &1_604_800_u64);
    }

    // ── execute_payment ───────────────────────────────────────────────────────

    #[test]
    fn execute_payment_advances_next_payment_time() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        // Move time to when first payment is due
        env.ledger().set_timestamp(1_604_800);
        env.mock_all_auths();
        client.execute_payment();

        // Next payment should be one interval later
        assert_eq!(client.get_next_payment_time(), 1_604_800_u64 + 604_800_u64);
    }

    #[test]
    #[should_panic(expected = "payment is not due yet")]
    fn execute_payment_panics_before_due_time() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        // Try to execute before the first payment is due
        env.ledger().set_timestamp(1_000_001);
        env.mock_all_auths();
        client.execute_payment();
    }

    // ── get_schedule_info ─────────────────────────────────────────────────────

    #[test]
    fn get_schedule_info_returns_all_fields_correctly() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        let (sender, recipient, token) = setup_schedule(&env, &client);

        let info = client.get_schedule_info();
        assert_eq!(info.sender, sender);
        assert_eq!(info.recipient, recipient);
        assert_eq!(info.token, token);
        assert_eq!(info.amount, 100_i128);
        assert_eq!(info.interval, 604_800_u64);
        assert_eq!(info.next_payment_time, 1_604_800_u64);
    }

    #[test]
    fn get_schedule_info_reflects_cancelled_status_after_cancel() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        env.mock_all_auths();
        client.cancel();

        let info = client.get_schedule_info();
        assert_eq!(info.status, ScheduleStatus::Cancelled);
    }

    // ── cancel ────────────────────────────────────────────────────────────────

    #[test]
    fn cancel_sets_status_to_cancelled() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        env.mock_all_auths();
        client.cancel();

        assert_eq!(client.get_schedule_status(), ScheduleStatus::Cancelled);
    }

    #[test]
    #[should_panic(expected = "schedule is not active")]
    fn cancel_panics_on_already_cancelled_schedule() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        env.mock_all_auths();
        client.cancel();
        client.cancel(); // second cancel should panic
    }

    #[test]
    #[should_panic(expected = "schedule is not active")]
    fn execute_payment_panics_on_cancelled_schedule() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = RecurringPaymentClient::new(&env, &contract_id);

        setup_schedule(&env, &client);

        env.mock_all_auths();
        client.cancel();

        env.ledger().set_timestamp(1_604_800);
        client.execute_payment(); // should panic — schedule cancelled
    }
}
