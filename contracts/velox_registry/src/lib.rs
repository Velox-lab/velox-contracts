#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

// ── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
pub enum RegistryKey {
    Stream(Address),
    Schedule(Address),
    AllStreams,
    AllSchedules,
    Admin,   // Address allowed to set the factory
    Factory, // The only address allowed to register entries
}

// ── Data Types ───────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct StreamEntry {
    pub stream_id: Address,
    pub sender: Address,
    pub recipient: Address,
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ScheduleEntry {
    pub schedule_id: Address,
    pub sender: Address,
    pub recipient: Address,
    pub registered_at: u64,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct VeloxRegistry;

#[contractimpl]
impl VeloxRegistry {
    /// Set the admin at deployment. Running as a constructor means no one can
    /// front-run initialization between deploy and setup.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().persistent().set(&RegistryKey::Admin, &admin);
    }

    /// Admin sets the factory allowed to register entries.
    /// The factory is deployed after the registry, so it cannot be a constructor argument.
    pub fn set_factory(env: Env, factory: Address) {
        let admin: Address = env.storage().persistent().get(&RegistryKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&RegistryKey::Factory, &factory);
    }

    /// Returns the factory allowed to register entries, if one is set.
    pub fn get_factory(env: Env) -> Option<Address> {
        env.storage().persistent().get(&RegistryKey::Factory)
    }

    /// Register a new payment stream in the registry.
    /// Only callable by the configured factory contract.
    pub fn register_stream(env: Env, entry: StreamEntry) {
        Self::require_factory(&env);

        let mut streams: Vec<StreamEntry> = env
            .storage()
            .persistent()
            .get(&RegistryKey::AllStreams)
            .unwrap_or(Vec::new(&env));

        streams.push_back(entry.clone());

        env.storage()
            .persistent()
            .set(&RegistryKey::AllStreams, &streams);

        env.storage()
            .persistent()
            .set(&RegistryKey::Stream(entry.stream_id.clone()), &entry);
    }

    /// Register a new recurring payment schedule in the registry.
    /// Only callable by the configured factory contract.
    pub fn register_schedule(env: Env, entry: ScheduleEntry) {
        Self::require_factory(&env);

        let mut schedules: Vec<ScheduleEntry> = env
            .storage()
            .persistent()
            .get(&RegistryKey::AllSchedules)
            .unwrap_or(Vec::new(&env));

        schedules.push_back(entry.clone());

        env.storage()
            .persistent()
            .set(&RegistryKey::AllSchedules, &schedules);

        env.storage()
            .persistent()
            .set(&RegistryKey::Schedule(entry.schedule_id.clone()), &entry);
    }

    /// Return all registered streams.
    pub fn get_all_streams(env: Env) -> Vec<StreamEntry> {
        env.storage()
            .persistent()
            .get(&RegistryKey::AllStreams)
            .unwrap_or(Vec::new(&env))
    }

    /// Return all registered schedules.
    pub fn get_all_schedules(env: Env) -> Vec<ScheduleEntry> {
        env.storage()
            .persistent()
            .get(&RegistryKey::AllSchedules)
            .unwrap_or(Vec::new(&env))
    }

    /// Return a single stream entry by its contract address.
    pub fn get_stream(env: Env, stream_id: Address) -> Option<StreamEntry> {
        env.storage()
            .persistent()
            .get(&RegistryKey::Stream(stream_id))
    }

    /// Return a single schedule entry by its contract address.
    pub fn get_schedule(env: Env, schedule_id: Address) -> Option<ScheduleEntry> {
        env.storage()
            .persistent()
            .get(&RegistryKey::Schedule(schedule_id))
    }

    /// Return all streams where sender matches the given address.
    pub fn list_streams_by_sender(env: Env, sender: Address) -> Vec<StreamEntry> {
        let all: Vec<StreamEntry> = env
            .storage()
            .persistent()
            .get(&RegistryKey::AllStreams)
            .unwrap_or(Vec::new(&env));

        let mut result = Vec::new(&env);
        for i in 0..all.len() {
            let entry = all.get(i).unwrap();
            if entry.sender == sender {
                result.push_back(entry);
            }
        }
        result
    }

    /// Return all streams where recipient matches the given address.
    pub fn list_streams_by_recipient(env: Env, recipient: Address) -> Vec<StreamEntry> {
        let all: Vec<StreamEntry> = env
            .storage()
            .persistent()
            .get(&RegistryKey::AllStreams)
            .unwrap_or(Vec::new(&env));

        let mut result = Vec::new(&env);
        for i in 0..all.len() {
            let entry = all.get(i).unwrap();
            if entry.recipient == recipient {
                result.push_back(entry);
            }
        }
        result
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Panic unless the call is authorized by the configured factory.
    fn require_factory(env: &Env) {
        let factory: Address = env
            .storage()
            .persistent()
            .get(&RegistryKey::Factory)
            .expect("factory not set");
        factory.require_auth();
    }
}

#[cfg(test)]
mod tests;
