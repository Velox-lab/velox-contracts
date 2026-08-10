#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, BytesN, Env,
    };
    use crate::{StreamFactory, StreamFactoryClient};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn create_env() -> Env {
        Env::default()
    }

    fn register_contract(env: &Env) -> Address {
        env.register_contract(None, StreamFactory)
    }

    fn dummy_wasm_hash(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[1u8; 32])
    }

    fn setup_factory(env: &Env, client: &StreamFactoryClient) -> (Address, Address) {
        let admin = Address::generate(env);
        let registry = Address::generate(env);

        env.mock_all_auths();
        client.initialize(
            &admin,
            &registry,
            &dummy_wasm_hash(env),
            &dummy_wasm_hash(env),
        );

        (admin, registry)
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn initialize_stores_registry_address() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);

        let (_admin, registry) = setup_factory(&env, &client);

        assert_eq!(client.get_registry(), registry);
    }

    #[test]
    fn initialize_stores_admin_address() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);

        let (admin, _registry) = setup_factory(&env, &client);

        assert_eq!(client.get_admin(), admin);
    }

    // ── create_stream validation ──────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "rate must be greater than zero")]
    fn create_stream_panics_when_rate_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        env.mock_all_auths();
        env.ledger().set_timestamp(1000);

        client.create_stream(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &0_i128,
            &1000_u64,
            &2000_u64,
            &1000_i128,
        );
    }

    #[test]
    #[should_panic(expected = "funded amount must be greater than zero")]
    fn create_stream_panics_when_funded_amount_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        env.mock_all_auths();
        env.ledger().set_timestamp(1000);

        client.create_stream(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &10_i128,
            &1000_u64,
            &2000_u64,
            &0_i128,
        );
    }

    #[test]
    #[should_panic(expected = "start_time must be before end_time")]
    fn create_stream_panics_when_start_after_end() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        env.mock_all_auths();
        env.ledger().set_timestamp(1000);

        client.create_stream(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &10_i128,
            &2000_u64,
            &1000_u64,
            &1000_i128,
        );
    }

    // ── create_schedule validation ────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "amount must be greater than zero")]
    fn create_schedule_panics_when_amount_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        env.mock_all_auths();

        client.create_schedule(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &0_i128,
            &604_800_u64,
            &1_604_800_u64,
        );
    }

    #[test]
    #[should_panic(expected = "interval must be greater than zero")]
    fn create_schedule_panics_when_interval_is_zero() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        env.mock_all_auths();

        client.create_schedule(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &100_i128,
            &0_u64,
            &1_604_800_u64,
        );
    }

    // ── set_stream_wasm ───────────────────────────────────────────────────────

    #[test]
    fn admin_can_update_stream_wasm_hash() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        let new_hash = BytesN::from_array(&env, &[2u8; 32]);
        env.mock_all_auths();
        client.set_stream_wasm(&new_hash);
        // No panic = success; hash is stored (no getter needed in tests)
    }
}
