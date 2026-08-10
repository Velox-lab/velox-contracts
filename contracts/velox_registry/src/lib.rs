#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec, Symbol, symbol_short};

// ── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
pub enum RegistryKey {
    Stream(Address),
    Schedule(Address),
    AllStreams,
    AllSchedules,
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
    /// Register a new payment stream in the registry.
    /// Only callable by an authorised factory contract.
    pub fn register_stream(env: Env, entry: StreamEntry) {
        entry.sender.require_auth();

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
            .set(&RegistryKey::Stream(entry.stream_id), &entry);
    }

    /// Register a new recurring payment schedule in the registry.
    pub fn register_schedule(env: Env, entry: ScheduleEntry) {
        entry.sender.require_auth();

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
            .set(&RegistryKey::Schedule(entry.schedule_id), &entry);
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
}
