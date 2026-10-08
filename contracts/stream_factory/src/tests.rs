#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{StellarAssetClient, TokenClient},
        Address, BytesN, Env,
    };
    use crate::{StreamFactory, StreamFactoryClient};

    // Real contract WASM, so the factory can deploy instances by hash.
    // Build first with: cargo build --target wasm32v1-none --release
    mod payment_stream_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/payment_stream.wasm"
        );
    }
    mod recurring_payment_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/recurring_payment.wasm"
        );
    }
    mod registry_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/velox_registry.wasm"
        );
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// A factory wired to a real registry and real stream/schedule WASM,
    /// plus a token with 10_000 minted to the returned sender.
    struct Deployed<'a> {
        factory: StreamFactoryClient<'a>,
        registry: registry_wasm::Client<'a>,
        token: Address,
        sender: Address,
        recipient: Address,
    }

    fn setup_deployed_factory(env: &Env) -> Deployed<'_> {
        env.mock_all_auths();

        let registry_id = env.register(registry_wasm::WASM, (Address::generate(env),));
        let registry = registry_wasm::Client::new(env, &registry_id);
        let stream_hash = env.deployer().upload_contract_wasm(payment_stream_wasm::WASM);
        let schedule_hash = env.deployer().upload_contract_wasm(recurring_payment_wasm::WASM);

        let factory = StreamFactoryClient::new(env, &env.register(StreamFactory, ()));
        factory.initialize(&Address::generate(env), &registry_id, &stream_hash, &schedule_hash);
        registry.set_factory(&factory.address);

        let sender = Address::generate(env);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(env))
            .address();
        StellarAssetClient::new(env, &token).mint(&sender, &10_000);

        Deployed {
            factory,
            registry,
            token,
            sender,
            recipient: Address::generate(env),
        }
    }

    fn create_env() -> Env {
        Env::default()
    }

    fn register_contract(env: &Env) -> Address {
        env.register(StreamFactory, ())
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

    #[test]
    #[should_panic(expected = "already initialized")]
    fn initialize_panics_when_called_twice() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = StreamFactoryClient::new(&env, &contract_id);
        setup_factory(&env, &client);

        // An attacker tries to take over as admin
        let attacker = Address::generate(&env);
        client.initialize(
            &attacker,
            &Address::generate(&env),
            &dummy_wasm_hash(&env),
            &dummy_wasm_hash(&env),
        );
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
        env.ledger().with_mut(|li| li.timestamp = 1000);

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
        env.ledger().with_mut(|li| li.timestamp = 1000);

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
        env.ledger().with_mut(|li| li.timestamp = 1000);

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

    // ── create_stream deployment ──────────────────────────────────────────────

    #[test]
    fn create_stream_initializes_deployed_stream() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1000);

        let stream_id = d.factory.create_stream(
            &d.sender, &d.recipient, &d.token, &10_i128, &1000_u64, &1100_u64, &1000_i128,
        );

        let info = payment_stream_wasm::Client::new(&env, &stream_id).get_stream_info();
        assert_eq!(info.sender, d.sender);
        assert_eq!(info.recipient, d.recipient);
        assert_eq!(info.token, d.token);
        assert_eq!(info.rate_per_second, 10);
        assert_eq!(info.start_time, 1000);
        assert_eq!(info.end_time, 1100);
        assert_eq!(info.total_funded, 1000);
        assert_eq!(info.total_withdrawn, 0);
        assert_eq!(info.status, payment_stream_wasm::StreamStatus::Active);

        // Funds moved from the sender into the stream's escrow
        let token = TokenClient::new(&env, &d.token);
        assert_eq!(token.balance(&stream_id), 1000);
        assert_eq!(token.balance(&d.sender), 9000);
    }

    #[test]
    fn create_stream_registers_stream_in_registry() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1000);

        let stream_id = d.factory.create_stream(
            &d.sender, &d.recipient, &d.token, &10_i128, &1000_u64, &1100_u64, &1000_i128,
        );

        let all = d.registry.get_all_streams();
        assert_eq!(all.len(), 1);
        let entry = all.get(0).unwrap();
        assert_eq!(entry.stream_id, stream_id);
        assert_eq!(entry.sender, d.sender);
        assert_eq!(entry.recipient, d.recipient);
        assert_eq!(entry.registered_at, 1000);

        assert_eq!(d.registry.list_streams_by_sender(&d.sender).len(), 1);
        assert_eq!(d.registry.list_streams_by_recipient(&d.recipient).len(), 1);
    }

    #[test]
    fn create_stream_deploys_each_stream_at_a_distinct_address() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1000);

        // Identical parameters must still produce two separate streams
        let first = d.factory.create_stream(
            &d.sender, &d.recipient, &d.token, &10_i128, &1000_u64, &1100_u64, &1000_i128,
        );
        let second = d.factory.create_stream(
            &d.sender, &d.recipient, &d.token, &10_i128, &1000_u64, &1100_u64, &1000_i128,
        );

        assert_ne!(first, second);
        assert_eq!(d.registry.get_all_streams().len(), 2);
    }

    #[test]
    fn stream_created_by_factory_can_be_withdrawn_from() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1000);

        let stream_id = d.factory.create_stream(
            &d.sender, &d.recipient, &d.token, &10_i128, &1000_u64, &1100_u64, &1000_i128,
        );

        env.ledger().with_mut(|li| li.timestamp = 1050);
        payment_stream_wasm::Client::new(&env, &stream_id).withdraw();

        assert_eq!(TokenClient::new(&env, &d.token).balance(&d.recipient), 500);
    }

    // ── create_schedule deployment ────────────────────────────────────────────

    #[test]
    fn create_schedule_initializes_deployed_schedule() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1_000_000);

        let schedule_id = d.factory.create_schedule(
            &d.sender, &d.recipient, &d.token, &100_i128, &604_800_u64, &1_604_800_u64,
        );

        let info = recurring_payment_wasm::Client::new(&env, &schedule_id).get_schedule_info();
        assert_eq!(info.sender, d.sender);
        assert_eq!(info.recipient, d.recipient);
        assert_eq!(info.token, d.token);
        assert_eq!(info.amount, 100);
        assert_eq!(info.interval, 604_800);
        assert_eq!(info.next_payment_time, 1_604_800);
        assert_eq!(info.status, recurring_payment_wasm::ScheduleStatus::Active);
    }

    #[test]
    fn create_schedule_registers_schedule_in_registry() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1_000_000);

        let schedule_id = d.factory.create_schedule(
            &d.sender, &d.recipient, &d.token, &100_i128, &604_800_u64, &1_604_800_u64,
        );

        let all = d.registry.get_all_schedules();
        assert_eq!(all.len(), 1);
        let entry = all.get(0).unwrap();
        assert_eq!(entry.schedule_id, schedule_id);
        assert_eq!(entry.sender, d.sender);
        assert_eq!(entry.recipient, d.recipient);
        assert_eq!(entry.registered_at, 1_000_000);
    }

    #[test]
    fn schedule_created_by_factory_executes_after_approval() {
        let env = create_env();
        let d = setup_deployed_factory(&env);
        env.ledger().with_mut(|li| li.timestamp = 1_000_000);

        let schedule_id = d.factory.create_schedule(
            &d.sender, &d.recipient, &d.token, &100_i128, &604_800_u64, &1_604_800_u64,
        );

        let token = TokenClient::new(&env, &d.token);
        let expiration_ledger = env.ledger().sequence() + 100_000;
        token.approve(&d.sender, &schedule_id, &1_000, &expiration_ledger);

        env.ledger().with_mut(|li| li.timestamp = 1_604_800);
        recurring_payment_wasm::Client::new(&env, &schedule_id).execute_payment();

        assert_eq!(token.balance(&d.recipient), 100);
    }
}
