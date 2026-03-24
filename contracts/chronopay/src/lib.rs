#![no_std]
//! ChronoPay time token contract with professional identity authorization.
//!
//! This contract implements time tokenization with strict professional identity
//! authorization to ensure only verified professionals can create time slots.
//!
//! # Authorization Model
//!
//! - Only the contract admin can authorize/unauthorize professionals
//! - Only authorized professionals can create time slots
//! - All operations are logged for audit purposes
//! - Authorization status is stored persistently

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, vec, Address, Env, String,
    Symbol, Vec,
};

/// Errors that can occur during contract execution.
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ChronoPayError {
    /// The caller is not authorized to perform this operation.
    Unauthorized = 1,
    /// The professional is not authorized to create time slots.
    ProfessionalNotAuthorized = 2,
    /// The professional is already authorized.
    AlreadyAuthorized = 3,
    /// The professional is not found in the registry.
    ProfessionalNotFound = 4,
    /// Invalid input parameters provided.
    InvalidInput = 5,
    /// Admin operation failed.
    AdminError = 6,
}

/// Status of a time token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimeTokenStatus {
    Available,
    Sold,
    Redeemed,
}

/// Data keys for contract storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Sequence counter for slot IDs.
    SlotSeq,
    /// Owner of a specific token (token_id -> owner).
    Owner(Symbol),
    /// Status of a specific token (token_id -> status).
    Status(Symbol),
    /// Contract admin address.
    Admin,
    /// Authorized professionals registry (professional_address -> bool).
    AuthorizedProfessional(Address),
    /// List of all authorized professionals for enumeration.
    AuthorizedProfessionalsList,
    /// Professional metadata (professional_address -> metadata).
    ProfessionalMetadata(Address),
}

/// Metadata stored for each professional.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfessionalMetadata {
    /// Display name of the professional.
    pub name: String,
    /// Verification timestamp.
    pub verified_at: u64,
    /// Professional category/specialization.
    pub category: String,
}

/// Events emitted by the contract.
pub mod events {
    use super::*;

    /// Emitted when a professional is authorized.
    pub fn professional_authorized(
        env: &Env,
        professional: &Address,
        name: &String,
        category: &String,
    ) {
        env.events().publish(
            (symbol_short!("auth"), symbol_short!("pro")),
            (professional.clone(), name.clone(), category.clone()),
        );
    }

    /// Emitted when a professional is unauthorized.
    pub fn professional_unauthorized(env: &Env, professional: &Address) {
        env.events().publish(
            (symbol_short!("unauth"), symbol_short!("pro")),
            professional.clone(),
        );
    }

    /// Emitted when authorization is checked.
    pub fn authorization_checked(env: &Env, professional: &Address, authorized: bool) {
        env.events().publish(
            (symbol_short!("check"), symbol_short!("auth")),
            (professional.clone(), authorized),
        );
    }

    /// Emitted when a time slot is created.
    pub fn time_slot_created(
        env: &Env,
        slot_id: u32,
        professional: &Address,
        start_time: u64,
        end_time: u64,
    ) {
        env.events().publish(
            (symbol_short!("slot"), symbol_short!("create")),
            (slot_id, professional.clone(), start_time, end_time),
        );
    }
}

#[contract]
pub struct ChronoPayContract;

#[contractimpl]
impl ChronoPayContract {
    /// Initialize the contract with an admin address.
    /// Must be called once after deployment.
    ///
    /// # Arguments
    /// * `admin` - The address that will have admin privileges
    ///
    /// # Errors
    /// * `ChronoPayError::InvalidInput` - If admin address is invalid
    pub fn initialize(env: Env, admin: Address) -> Result<(), ChronoPayError> {
        if admin == env.current_contract_address() {
            return Err(ChronoPayError::InvalidInput);
        }

        // Verify admin address is valid by requiring authorization
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::SlotSeq, &0u32);
        env.storage().instance().set(
            &DataKey::AuthorizedProfessionalsList,
            &Vec::<Address>::new(&env),
        );

        Ok(())
    }

    /// Authorize a professional to create time slots.
    /// Only callable by the contract admin.
    ///
    /// # Arguments
    /// * `professional` - The address to authorize
    /// * `name` - Display name of the professional
    /// * `category` - Professional category/specialization
    ///
    /// # Errors
    /// * `ChronoPayError::Unauthorized` - If caller is not admin
    /// * `ChronoPayError::AlreadyAuthorized` - If professional is already authorized
    pub fn authorize_professional(
        env: Env,
        professional: Address,
        name: String,
        category: String,
    ) -> Result<(), ChronoPayError> {
        // Only admin can authorize professionals
        Self::require_admin(&env)?;

        // Check if already authorized
        if Self::is_professional_authorized(&env, &professional) {
            return Err(ChronoPayError::AlreadyAuthorized);
        }

        // Validate inputs
        if name.is_empty() || category.is_empty() {
            return Err(ChronoPayError::InvalidInput);
        }

        // Store authorization status
        env.storage().instance().set(
            &DataKey::AuthorizedProfessional(professional.clone()),
            &true,
        );

        // Store professional metadata
        let metadata = ProfessionalMetadata {
            name: name.clone(),
            verified_at: env.ledger().timestamp(),
            category: category.clone(),
        };
        env.storage().instance().set(
            &DataKey::ProfessionalMetadata(professional.clone()),
            &metadata,
        );

        // Add to list of authorized professionals
        let mut pro_list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorizedProfessionalsList)
            .unwrap_or_else(|| Vec::new(&env));
        pro_list.push_back(professional.clone());
        env.storage()
            .instance()
            .set(&DataKey::AuthorizedProfessionalsList, &pro_list);

        // Emit event
        events::professional_authorized(&env, &professional, &name, &category);

        Ok(())
    }

    /// Unauthorize a professional, preventing them from creating new time slots.
    /// Only callable by the contract admin.
    ///
    /// # Arguments
    /// * `professional` - The address to unauthorize
    ///
    /// # Errors
    /// * `ChronoPayError::Unauthorized` - If caller is not admin
    /// * `ChronoPayError::ProfessionalNotFound` - If professional is not authorized
    pub fn unauthorize_professional(env: Env, professional: Address) -> Result<(), ChronoPayError> {
        // Only admin can unauthorize professionals
        Self::require_admin(&env)?;

        // Check if professional exists
        if !Self::is_professional_authorized(&env, &professional) {
            return Err(ChronoPayError::ProfessionalNotFound);
        }

        // Remove authorization
        env.storage()
            .instance()
            .remove(&DataKey::AuthorizedProfessional(professional.clone()));

        // Remove metadata
        env.storage()
            .instance()
            .remove(&DataKey::ProfessionalMetadata(professional.clone()));

        // Remove from list
        let mut pro_list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AuthorizedProfessionalsList)
            .unwrap_or_else(|| Vec::new(&env));

        // Find and remove the professional from the list
        let mut new_list = Vec::new(&env);
        for pro in pro_list.iter() {
            if pro != professional {
                new_list.push_back(pro);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::AuthorizedProfessionalsList, &new_list);

        // Emit event
        events::professional_unauthorized(&env, &professional);

        Ok(())
    }

    /// Check if a professional is authorized.
    ///
    /// # Arguments
    /// * `professional` - The address to check
    ///
    /// # Returns
    /// * `bool` - True if authorized, false otherwise
    pub fn is_professional_authorized(env: &Env, professional: &Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::AuthorizedProfessional(professional.clone()))
            .unwrap_or(false)
    }

    /// Get professional metadata.
    ///
    /// # Arguments
    /// * `professional` - The address to query
    ///
    /// # Returns
    /// * `Option<ProfessionalMetadata>` - Metadata if professional exists
    pub fn get_professional_metadata(
        env: Env,
        professional: Address,
    ) -> Option<ProfessionalMetadata> {
        env.storage()
            .instance()
            .get(&DataKey::ProfessionalMetadata(professional))
    }

    /// Get the list of all authorized professionals.
    ///
    /// # Returns
    /// * `Vec<Address>` - List of authorized professional addresses
    pub fn get_authorized_professionals(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::AuthorizedProfessionalsList)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Create a time slot with an auto-incrementing slot id.
    /// Only authorized professionals can create time slots.
    ///
    /// # Arguments
    /// * `professional` - The professional creating the slot (must be authorized)
    /// * `start_time` - Start timestamp of the slot
    /// * `end_time` - End timestamp of the slot
    ///
    /// # Returns
    /// * `u32` - The newly assigned slot id
    ///
    /// # Errors
    /// * `ChronoPayError::ProfessionalNotAuthorized` - If professional is not authorized
    /// * `ChronoPayError::InvalidInput` - If time parameters are invalid
    pub fn create_time_slot(
        env: Env,
        professional: Address,
        start_time: u64,
        end_time: u64,
    ) -> Result<u32, ChronoPayError> {
        // Require professional authorization
        professional.require_auth();

        // Verify professional is authorized
        if !Self::is_professional_authorized(&env, &professional) {
            events::authorization_checked(&env, &professional, false);
            return Err(ChronoPayError::ProfessionalNotAuthorized);
        }

        events::authorization_checked(&env, &professional, true);

        // Validate time parameters
        if start_time >= end_time {
            return Err(ChronoPayError::InvalidInput);
        }

        let current_time = env.ledger().timestamp();
        if start_time < current_time {
            return Err(ChronoPayError::InvalidInput);
        }

        // Get and increment slot sequence
        let current_seq: u32 = env
            .storage()
            .instance()
            .get(&DataKey::SlotSeq)
            .unwrap_or(0u32);

        let next_seq = current_seq
            .checked_add(1)
            .ok_or(ChronoPayError::InvalidInput)?;

        env.storage().instance().set(&DataKey::SlotSeq, &next_seq);

        // Emit event for slot creation
        events::time_slot_created(&env, next_seq, &professional, start_time, end_time);

        Ok(next_seq)
    }

    /// Mint a time token for a slot.
    /// Only authorized professionals can mint tokens.
    ///
    /// # Arguments
    /// * `professional` - The professional minting the token
    /// * `slot_id` - The slot ID to mint for
    ///
    /// # Returns
    /// * `Symbol` - The token identifier
    ///
    /// # Errors
    /// * `ChronoPayError::ProfessionalNotAuthorized` - If professional is not authorized
    pub fn mint_time_token(
        env: Env,
        professional: Address,
        slot_id: u32,
    ) -> Result<Symbol, ChronoPayError> {
        // Require professional authorization
        professional.require_auth();

        // Verify professional is authorized
        if !Self::is_professional_authorized(&env, &professional) {
            return Err(ChronoPayError::ProfessionalNotAuthorized);
        }

        let token_id = Symbol::new(&env, "TIME_TOKEN");

        // Set initial owner and status
        env.storage()
            .instance()
            .set(&DataKey::Owner(token_id.clone()), &professional);
        env.storage().instance().set(
            &DataKey::Status(token_id.clone()),
            &TimeTokenStatus::Available,
        );

        Ok(token_id)
    }

    /// Buy / transfer time token.
    ///
    /// # Arguments
    /// * `token_id` - The token to buy
    /// * `buyer` - The buyer's address
    /// * `seller` - The seller's address
    ///
    /// # Returns
    /// * `bool` - True if successful
    pub fn buy_time_token(
        env: Env,
        token_id: Symbol,
        buyer: Address,
        seller: Address,
    ) -> Result<bool, ChronoPayError> {
        let _ = (buyer, seller);

        // Check token exists and is available
        let status: TimeTokenStatus = env
            .storage()
            .instance()
            .get(&DataKey::Status(token_id.clone()))
            .ok_or(ChronoPayError::InvalidInput)?;

        if status != TimeTokenStatus::Available {
            return Err(ChronoPayError::InvalidInput);
        }

        // Update status to sold
        env.storage()
            .instance()
            .set(&DataKey::Status(token_id.clone()), &TimeTokenStatus::Sold);

        Ok(true)
    }

    /// Redeem time token.
    ///
    /// # Arguments
    /// * `token_id` - The token to redeem
    ///
    /// # Returns
    /// * `bool` - True if successful
    pub fn redeem_time_token(env: Env, token_id: Symbol) -> Result<bool, ChronoPayError> {
        // Check token exists
        let status: TimeTokenStatus = env
            .storage()
            .instance()
            .get(&DataKey::Status(token_id.clone()))
            .ok_or(ChronoPayError::InvalidInput)?;

        // Can only redeem sold tokens
        if status != TimeTokenStatus::Sold {
            return Err(ChronoPayError::InvalidInput);
        }

        // Mark as redeemed
        env.storage().instance().set(
            &DataKey::Status(token_id.clone()),
            &TimeTokenStatus::Redeemed,
        );

        Ok(true)
    }

    /// Get the admin address.
    ///
    /// # Returns
    /// * `Option<Address>` - The admin address if set
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    /// Hello-style entrypoint for CI and SDK sanity check.
    pub fn hello(env: Env, to: String) -> Vec<String> {
        vec![&env, String::from_str(&env, "ChronoPay"), to]
    }

    // Internal helper functions

    /// Require that the caller is the admin.
    fn require_admin(env: &Env) -> Result<(), ChronoPayError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ChronoPayError::AdminError)?;

        admin.require_auth();
        Ok(())
    }
}

mod test;
