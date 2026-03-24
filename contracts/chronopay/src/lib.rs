#![no_std]
//! ChronoPay time token contract.
//!
//! # Slot overlap invariant
//! Each professional may only hold non-overlapping time slots.
//! A new slot `[start, end)` is rejected if any existing slot for the same
//! professional satisfies `existing_start < end && start < existing_end`.
//!
//! # Storage layout
//! - `DataKey::SlotSeq`              → `u32`  global auto-increment counter
//! - `DataKey::Slot(id)`             → `TimeSlot`
//! - `DataKey::ProfSlots(professional)` → `Vec<u32>` slot ids owned by that professional

use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, vec, Env, String, Symbol, Vec};

// ── Error codes ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// start_time must be strictly less than end_time.
    InvalidRange = 1,
    /// The requested slot overlaps an existing slot for this professional.
    SlotOverlap = 2,
    /// Slot id does not exist.
    SlotNotFound = 3,
}

impl soroban_sdk::contracterror::ContractError for Error {
    fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Error::InvalidRange),
            2 => Some(Error::SlotOverlap),
            3 => Some(Error::SlotNotFound),
            _ => None,
        }
    }
}

// ── Types ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeSlot {
    pub professional: String,
    pub start_time: u64,
    pub end_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimeTokenStatus {
    Available,
    Sold,
    Redeemed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    SlotSeq,
    /// Stores a `TimeSlot` for the given slot id.
    Slot(u32),
    /// Stores a `Vec<u32>` of slot ids for the given professional.
    ProfSlots(String),
    Owner,
    Status,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct ChronoPayContract;

#[contractimpl]
impl ChronoPayContract {
    /// Create a time slot for `professional` covering `[start_time, end_time)`.
    ///
    /// # Errors
    /// - `Error::InvalidRange`  if `start_time >= end_time`
    /// - `Error::SlotOverlap`   if the interval overlaps any existing slot for
    ///   this professional
    ///
    /// # Returns
    /// The newly assigned slot id (1-based, auto-incrementing).
    pub fn create_time_slot(
        env: Env,
        professional: String,
        start_time: u64,
        end_time: u64,
    ) -> u32 {
        // ── Validate range ────────────────────────────────────────────────
        if start_time >= end_time {
            panic_with_error!(&env, Error::InvalidRange);
        }

        // ── Check for overlaps ────────────────────────────────────────────
        // Two intervals [a,b) and [c,d) overlap iff a < d && c < b.
        let existing_ids: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::ProfSlots(professional.clone()))
            .unwrap_or_else(|| vec![&env]);

        for i in 0..existing_ids.len() {
            let id = existing_ids.get(i).unwrap();
            let slot: TimeSlot = env
                .storage()
                .instance()
                .get(&DataKey::Slot(id))
                .unwrap(); // always present if id is in the list
            if start_time < slot.end_time && slot.start_time < end_time {
                panic_with_error!(&env, Error::SlotOverlap);
            }
        }

        // ── Assign id ─────────────────────────────────────────────────────
        let current_seq: u32 = env
            .storage()
            .instance()
            .get(&DataKey::SlotSeq)
            .unwrap_or(0u32);
        let slot_id = current_seq.checked_add(1).expect("slot id overflow");
        env.storage().instance().set(&DataKey::SlotSeq, &slot_id);

        // ── Persist slot ──────────────────────────────────────────────────
        let slot = TimeSlot {
            professional: professional.clone(),
            start_time,
            end_time,
        };
        env.storage()
            .instance()
            .set(&DataKey::Slot(slot_id), &slot);

        // ── Update professional's slot list ───────────────────────────────
        let mut ids = existing_ids;
        ids.push_back(slot_id);
        env.storage()
            .instance()
            .set(&DataKey::ProfSlots(professional), &ids);

        slot_id
    }

    /// Mint a time token for a slot.
    pub fn mint_time_token(env: Env, slot_id: u32) -> Symbol {
        let _ = slot_id;
        Symbol::new(&env, "TIME_TOKEN")
    }

    /// Buy / transfer a time token.
    pub fn buy_time_token(env: Env, token_id: Symbol, buyer: String, seller: String) -> bool {
        let _ = (token_id, buyer, seller);
        env.storage()
            .instance()
            .set(&DataKey::Owner, &env.current_contract_address());
        true
    }

    /// Redeem a time token.
    pub fn redeem_time_token(env: Env, token_id: Symbol) -> bool {
        let _ = token_id;
        env.storage()
            .instance()
            .set(&DataKey::Status, &TimeTokenStatus::Redeemed);
        true
    }

    /// Hello-style entrypoint for CI and SDK sanity check.
    pub fn hello(env: Env, to: String) -> Vec<String> {
        vec![&env, String::from_str(&env, "ChronoPay"), to]
    }
}

mod test;
