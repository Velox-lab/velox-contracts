#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, BytesN};

// ── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum FactoryKey {
    Registry,       // Address of VeloxRegistry contract
    StreamWasm,     // WASM hash of PaymentStream contract
    ScheduleWasm,   // WASM hash of RecurringPayment contract
    Admin,          // Admin address allowed to update WASM hashes
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct StreamFactory;

#[contractimpl]
impl StreamFactory {
    /// Initialise the factory with registry address and contract WASM hashes.
    /// Must be called once after deployment before any streams can be created.
    pub fn initialize(
        env: Env,
        admin: Address,
        registry: Address,
        stream_wasm_hash: BytesN<32>,
        schedule_wasm_hash: BytesN<32>,
    ) {
        admin.require_auth();

        let storage = env.storage().persistent();
        storage.set(&FactoryKey::Admin, &admin);
        storage.set(&FactoryKey::Registry, &registry);
        storage.set(&FactoryKey::StreamWasm, &stream_wasm_hash);
        storage.set(&FactoryKey::ScheduleWasm, &schedule_wasm_hash);
    }

    /// Deploy a new PaymentStream contract instance and register it in VeloxRegistry.
    /// Returns the new stream's contract address.
    pub fn create_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        rate_per_second: i128,
        start_time: u64,
        end_time: u64,
        total_funded: i128,
    ) -> Address {
        sender.require_auth();

        assert!(rate_per_second > 0, "rate must be greater than zero");
        assert!(total_funded > 0, "funded amount must be greater than zero");
        assert!(start_time < end_time, "start_time must be before end_time");

        let wasm_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&FactoryKey::StreamWasm)
            .unwrap();

        // Deploy a fresh PaymentStream contract instance
        let stream_address = env
            .deployer()
            .with_current_contract(env.crypto().sha256(&soroban_sdk::Bytes::from_array(
                &env,
                &[
                    sender.clone().to_string().as_bytes(),
                    recipient.clone().to_string().as_bytes(),
                    &start_time.to_be_bytes(),
                ]
                .concat()
                .try_into()
                .unwrap_or([0u8; 32]),
            )))
            .deploy(wasm_hash);

        // TODO: invoke PaymentStream.initialize on the deployed instance
        // This will be wired up once cross-contract call patterns are finalised
        // by contributors — see open issue: "Wire StreamFactory → PaymentStream init"

        stream_address
    }

    /// Deploy a new RecurringPayment contract instance and register it.
    /// Returns the new schedule's contract address.
    pub fn create_schedule(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        interval: u64,
        first_payment_time: u64,
    ) -> Address {
        sender.require_auth();

        assert!(amount > 0, "amount must be greater than zero");
        assert!(interval > 0, "interval must be greater than zero");

        let wasm_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&FactoryKey::ScheduleWasm)
            .unwrap();

        let schedule_address = env
            .deployer()
            .with_current_contract(env.crypto().sha256(&soroban_sdk::Bytes::from_array(
                &env,
                &[
                    sender.clone().to_string().as_bytes(),
                    recipient.clone().to_string().as_bytes(),
                    &first_payment_time.to_be_bytes(),
                ]
                .concat()
                .try_into()
                .unwrap_or([0u8; 32]),
            )))
            .deploy(wasm_hash);

        // TODO: invoke RecurringPayment.initialize on the deployed instance
        // See open issue: "Wire StreamFactory → RecurringPayment init"

        schedule_address
    }

    /// Returns the VeloxRegistry contract address.
    pub fn get_registry(env: Env) -> Address {
        env.storage().persistent().get(&FactoryKey::Registry).unwrap()
    }

    /// Returns the admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage().persistent().get(&FactoryKey::Admin).unwrap()
    }

    /// Admin updates the PaymentStream WASM hash (for upgrades).
    pub fn set_stream_wasm(env: Env, new_hash: BytesN<32>) {
        let admin: Address = env.storage().persistent().get(&FactoryKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&FactoryKey::StreamWasm, &new_hash);
    }

    /// Admin updates the RecurringPayment WASM hash (for upgrades).
    pub fn set_schedule_wasm(env: Env, new_hash: BytesN<32>) {
        let admin: Address = env.storage().persistent().get(&FactoryKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&FactoryKey::ScheduleWasm, &new_hash);
    }
}
