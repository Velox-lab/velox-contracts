#![no_std]
use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, Address, Bytes, BytesN, Env,
};

// ── Cross-contract interfaces ────────────────────────────────────────────────
//
// Declared locally rather than depending on the other contract crates, so the
// factory WASM does not link in their exported functions. Signatures and types
// must match the deployed contracts exactly.

#[contractclient(name = "PaymentStreamClient")]
pub trait PaymentStreamInterface {
    fn initialize(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        rate_per_second: i128,
        start_time: u64,
        end_time: u64,
        total_funded: i128,
    );
}

#[contractclient(name = "RecurringPaymentClient")]
pub trait RecurringPaymentInterface {
    fn initialize(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        amount: i128,
        interval: u64,
        first_payment_time: u64,
    );
}

#[contractclient(name = "VeloxRegistryClient")]
pub trait VeloxRegistryInterface {
    fn register_stream(env: Env, entry: StreamEntry);
    fn register_schedule(env: Env, entry: ScheduleEntry);
}

/// Mirror of VeloxRegistry's StreamEntry.
#[contracttype]
#[derive(Clone)]
pub struct StreamEntry {
    pub stream_id: Address,
    pub sender: Address,
    pub recipient: Address,
    pub registered_at: u64,
}

/// Mirror of VeloxRegistry's ScheduleEntry.
#[contracttype]
#[derive(Clone)]
pub struct ScheduleEntry {
    pub schedule_id: Address,
    pub sender: Address,
    pub recipient: Address,
    pub registered_at: u64,
}

// ── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum FactoryKey {
    Registry,       // Address of VeloxRegistry contract
    StreamWasm,     // WASM hash of PaymentStream contract
    ScheduleWasm,   // WASM hash of RecurringPayment contract
    Admin,          // Admin address allowed to update WASM hashes
    DeployNonce,    // Counter used to derive a unique salt per deployment
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
        assert!(!storage.has(&FactoryKey::Admin), "already initialized");
        storage.set(&FactoryKey::Admin, &admin);
        storage.set(&FactoryKey::Registry, &registry);
        storage.set(&FactoryKey::StreamWasm, &stream_wasm_hash);
        storage.set(&FactoryKey::ScheduleWasm, &schedule_wasm_hash);
    }

    /// Deploy a new PaymentStream contract instance, initialize it (which escrows
    /// `total_funded` from the sender), and register it in VeloxRegistry.
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
            .with_current_contract(Self::next_deploy_salt(&env))
            .deploy_v2(wasm_hash, ());

        PaymentStreamClient::new(&env, &stream_address).initialize(
            &sender,
            &recipient,
            &token,
            &rate_per_second,
            &start_time,
            &end_time,
            &total_funded,
        );

        Self::registry_client(&env).register_stream(&StreamEntry {
            stream_id: stream_address.clone(),
            sender,
            recipient,
            registered_at: env.ledger().timestamp(),
        });

        stream_address
    }

    /// Deploy a new RecurringPayment contract instance, initialize it, and
    /// register it in VeloxRegistry. Returns the new schedule's contract address.
    ///
    /// Payments are pulled from the sender's allowance, so the sender must
    /// separately `approve` the returned schedule address on the token.
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
            .with_current_contract(Self::next_deploy_salt(&env))
            .deploy_v2(wasm_hash, ());

        RecurringPaymentClient::new(&env, &schedule_address).initialize(
            &sender,
            &recipient,
            &token,
            &amount,
            &interval,
            &first_payment_time,
        );

        Self::registry_client(&env).register_schedule(&ScheduleEntry {
            schedule_id: schedule_address.clone(),
            sender,
            recipient,
            registered_at: env.ledger().timestamp(),
        });

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

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Client for the VeloxRegistry configured at initialization.
    fn registry_client(env: &Env) -> VeloxRegistryClient<'_> {
        let registry: Address = env.storage().persistent().get(&FactoryKey::Registry).unwrap();
        VeloxRegistryClient::new(env, &registry)
    }

    /// Derive a unique deployment salt from an incrementing nonce.
    /// Guarantees every deployed instance gets a distinct contract address.
    fn next_deploy_salt(env: &Env) -> BytesN<32> {
        let nonce: u64 = env
            .storage()
            .persistent()
            .get(&FactoryKey::DeployNonce)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&FactoryKey::DeployNonce, &(nonce + 1));

        env.crypto()
            .sha256(&Bytes::from_array(env, &nonce.to_be_bytes()))
            .into()
    }
}

#[cfg(test)]
mod tests;
