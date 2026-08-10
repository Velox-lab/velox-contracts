#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::Address as _, Address, Env};
    use crate::{VeloxRegistry, VeloxRegistryClient, ScheduleEntry, StreamEntry};

    fn create_env() -> Env {
        Env::default()
    }

    fn register_contract(env: &Env) -> Address {
        env.register_contract(None, VeloxRegistry)
    }

    // ── register_stream ──────────────────────────────────────────────────────

    #[test]
    fn register_stream_stores_entry() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = VeloxRegistryClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let stream_id = Address::generate(&env);

        let entry = StreamEntry {
            stream_id: stream_id.clone(),
            sender: sender.clone(),
            recipient: recipient.clone(),
            registered_at: 1000,
        };

        env.mock_all_auths();
        client.register_stream(&entry);

        let stored = client.get_stream(&stream_id).unwrap();
        assert_eq!(stored.sender, sender);
        assert_eq!(stored.recipient, recipient);
    }

    #[test]
    fn register_stream_appears_in_get_all_streams() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = VeloxRegistryClient::new(&env, &contract_id);

        let entry = StreamEntry {
            stream_id: Address::generate(&env),
            sender: Address::generate(&env),
            recipient: Address::generate(&env),
            registered_at: 1000,
        };

        env.mock_all_auths();
        client.register_stream(&entry);

        let all = client.get_all_streams();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn multiple_streams_are_all_stored() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = VeloxRegistryClient::new(&env, &contract_id);

        env.mock_all_auths();

        for _ in 0..3 {
            let entry = StreamEntry {
                stream_id: Address::generate(&env),
                sender: Address::generate(&env),
                recipient: Address::generate(&env),
                registered_at: 1000,
            };
            client.register_stream(&entry);
        }

        assert_eq!(client.get_all_streams().len(), 3);
    }

    // ── register_schedule ────────────────────────────────────────────────────

    #[test]
    fn register_schedule_stores_entry() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = VeloxRegistryClient::new(&env, &contract_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let schedule_id = Address::generate(&env);

        let entry = ScheduleEntry {
            schedule_id: schedule_id.clone(),
            sender: sender.clone(),
            recipient: recipient.clone(),
            registered_at: 2000,
        };

        env.mock_all_auths();
        client.register_schedule(&entry);

        let stored = client.get_schedule(&schedule_id).unwrap();
        assert_eq!(stored.sender, sender);
        assert_eq!(stored.recipient, recipient);
    }

    #[test]
    fn get_stream_returns_none_for_unknown_id() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = VeloxRegistryClient::new(&env, &contract_id);

        let unknown = Address::generate(&env);
        assert!(client.get_stream(&unknown).is_none());
    }

    #[test]
    fn get_schedule_returns_none_for_unknown_id() {
        let env = create_env();
        let contract_id = register_contract(&env);
        let client = VeloxRegistryClient::new(&env, &contract_id);

        let unknown = Address::generate(&env);
        assert!(client.get_schedule(&unknown).is_none());
    }
}
