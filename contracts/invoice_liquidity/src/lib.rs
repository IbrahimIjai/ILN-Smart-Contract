#![no_std]
// Soroban's contractimpl/contractargs macros generate client functions that
// mirror the contract's public interface — these may exceed the 7-argument
// threshold when the source function itself has many arguments.
#![allow(clippy::too_many_arguments)]

#[cfg(test)]
extern crate std;

pub mod access;
pub mod config;
pub mod constants;
pub mod errors;
pub mod events;
pub mod invoice;
pub mod oracle_interface;
pub mod multisig;
pub mod nft;
pub mod rate_logic;
pub mod storage;
pub mod top_payers;

use access::*;
use soroban_sdk::{
    contract, contractimpl, token::Client as TokenClient, vec, Address, BytesN, Env, IntoVal,
    Symbol, Vec,
};
use soroban_sdk::BytesN;
pub mod constants;
pub mod oracle_interface;
#[cfg(test)]
mod tests_discount_rate;
mod tests_lp_pagination;
mod tests_new_features;
mod tests_pagination;
mod tests_regression;
mod tests_reentrancy;
mod tests_xlm_support;
#[cfg(test)]
mod tests_error_cases;
#[cfg(test)]
mod tests_stress;
#[cfg(test)]
mod tests_lifecycle_integration;
#[cfg(test)]
mod tests_dutch_auction;
mod tests_invoice_nft;
#[cfg(test)]
mod tests_lp_whitelist;
#[cfg(test)]
mod tests_multisig_admin;
#[cfg(test)]
mod tests_lp_portfolio_stats;
#[cfg(test)]
mod tests_counter;

pub use crate::invoice::{
    AppealRecord, ContractStats, DisputeRecord, Invoice, InvoiceParams, InvoiceStatus,
    LpFundRequest, ReputationProfile, ReputationScore, StatsDelta, TopPayerEntry,
};
pub use crate::storage::DataKey as StorageKey;
    AppealRecord, Invoice, InvoiceParams, InvoiceStatus, LpFundRequest, LPStats, ReputationProfile,
    ReputationScore, TopPayerEntry,
};
pub use crate::nft::InvoiceNftMetadata;
pub use crate::storage::DataKey;
pub use config::{Config, ConfigError};
pub use errors::ContractError;
pub use events::*;

use crate::storage::get_admin;
use events::{
    AdminChanged, AppealResolved, AuctionFunded, AuctionStarted, ContractPaused, ContractUnpaused, ContractUpgraded,
    DefaultAppealed, DisputeResolved, FundQueueResolved, FundRequested, InvoiceCancelled,
    InvoiceDefaulted, InvoiceDisputed, InvoiceExpired, InvoiceFunded, InvoicePaid, InvoicePartiallyPaid,
    InvoiceSubmitted, InvoiceTokenChanged, InvoiceTransferred, InvoiceUpdated, ParameterUpdated, TokenAdded,
    TokenRemoved,
};

use crate::invoice::{
    add_invoice_to_lp, add_invoice_to_submitter, get_appeal, get_contract_stats, get_dispute,
    get_lp_invoices, get_lp_score, get_payer_score, get_pre_default_payer_score,
    get_reputation, get_submitter_invoices, increment_invoices_defaulted, increment_invoices_paid,
    increment_invoices_submitted, invoice_exists, load_invoice, remove_invoice_from_submitter,
    save_appeal, save_dispute, save_invoice, save_pre_default_payer_score, set_lp_score,
    set_payer_score, set_reputation, try_load_invoice,
};

use invoice::{
    add_invoice_to_lp, add_invoice_to_submitter, add_volume, get_appeal, get_contract_stats,
    get_dispute, get_fund_queue, get_invoice_funders, get_lp_invoices, get_lp_score,
    get_min_payer_reputation, get_payer_score, get_pre_default_payer_score, get_queue_resolution,
    get_reputation, get_submitter_invoices, increment_total_funded, increment_total_invoices,
    increment_total_paid, invoice_exists, is_paused, load_invoice, next_invoice_id,
    remove_invoice_from_lp, remove_invoice_from_submitter, save_appeal, save_dispute, save_fund_queue, save_invoice,
    save_invoice_funders, save_pre_default_payer_score, save_queue_resolution, set_lp_score,
    set_min_payer_reputation, set_paused, set_payer_score, set_reputation, try_load_invoice,
    ContractStats, DisputeRecord, StorageKey, increment_invoices_submitted, increment_invoices_paid,
    increment_invoices_defaulted,
};
use storage::with_reentrancy_guard;
use rate_logic::calculate_auction_rate;
use storage::{get_lp_portfolio_stats as storage_get_lp_portfolio_stats, save_lp_portfolio_stats};
// 30-day window in seconds for a payer to file an appeal after a default.
const APPEAL_WINDOW_SECONDS: u64 = 30 * 24 * 60 * 60;

// ----------------------------------------------------------------
// CONSTANTS (Legacy fallbacks, preferably use constants.rs)
// ----------------------------------------------------------------

/// Minimum invoice duration: 24 hours (in seconds)
const MIN_INVOICE_DURATION: u64 = 24 * 60 * 60;

/// Maximum invoice duration: 365 days (in seconds)
const MAX_INVOICE_DURATION: u64 = 365 * 24 * 60 * 60;

/// Default oracle freshness window: ~24 hours at one ledger per 5 seconds.
/// Governance can override this per-contract via set_max_oracle_age().
pub const DEFAULT_MAX_ORACLE_AGE_LEDGERS: u64 = 17_280;

// ----------------------------------------------------------------
// ORACLE TYPES (Issue #93)
// ----------------------------------------------------------------

use soroban_sdk::contracttype;

/// Response returned by the oracle's get_payer_data() entry point.
/// Combines identity verification with a freshness timestamp so the
/// contract can reject stale data without a second round-trip.
#[contracttype]
#[derive(Clone, Debug)]
pub struct OracleVerificationResponse {
    /// Whether the payer has passed oracle identity/creditworthiness checks.
    pub is_verified: bool,
    /// Ledger sequence number at which this data was last updated by the oracle.
    /// fund_invoice() rejects responses where current_ledger - timestamp ≥ max_oracle_age_ledgers.
    pub timestamp: u32,
}

// ----------------------------------------------------------------
// CONTRACT
// ----------------------------------------------------------------

#[contract]
pub struct InvoiceLiquidityContract;

#[contractimpl]
impl InvoiceLiquidityContract {
    // ------------------------------------------------------------
    // initialize (multi-token aware)
    // ------------------------------------------------------------
    /// Access: Anyone
    pub fn initialize(
        env: Env,
        admin: Address,
        usdc_token: Address,
        eurc_token: Address,
        xlm_token: Address,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&StorageKey::Admin) || 
           env.storage().instance().has(&StorageKey::InvoiceCount) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage().instance().set(&StorageKey::FeeRate, &0_u32);
        env.storage()
            .instance()
            .set(&StorageKey::MaxDiscountRate, &5000_u32);

        if !env.storage().instance().has(&StorageKey::NextInvoiceId) {
            env.storage().instance().set(&StorageKey::NextInvoiceId, &1_u64);
        }

        // Initialize config
        let initial_config = Config {
        // Initialize config with token addresses
        let initial_config = crate::config::Config {
            high_rep_threshold: 70,
            bonus_bps: 100,
            min_discount_rate_bps: 100,
            decay_rate_bps: 50,
            decay_period_ledgers: 10000,
            dispute_timeout_ledgers: 10000,
            xlm_sac_address: xlm_token.clone(),
            usdc_sac_address: usdc_token.clone(),
            eurc_sac_address: eurc_token.clone(),
            price_oracle: None,
            max_oracle_age_ledgers: DEFAULT_MAX_ORACLE_AGE_LEDGERS,
        };
        set_config(&env, &initial_config);

        // approve initial tokens
        env.storage().persistent().set(
            &crate::storage::DataKey::ApprovedToken(usdc_token.clone()),
            &true,
        );

        env.storage().persistent().set(
            &crate::storage::DataKey::ApprovedToken(eurc_token.clone()),
            &true,
        );

        // approve native XLM SAC
        env.storage()
            .persistent()
            .set(&StorageKey::ApprovedToken(xlm_token.clone()), &true);

        let mut list: Vec<Address> = Vec::new(&env);
        list.push_back(usdc_token);
        list.push_back(xlm_token);
        list.push_back(eurc_token);

        env.storage()
            .persistent()
            .set(&StorageKey::TokenList, &list);

        Ok(())
    }

    pub fn get_config(env: Env) -> Result<Config, ContractError> {
        crate::storage::get_config(&env).ok_or(ContractError::Unauthorized)
    }

    pub fn update_config(
        env: Env,
        caller: Address,
        high_rep_threshold: u32,
        bonus_bps: u32,
        min_discount_rate_bps: u32,
        decay_rate_bps: u32,
        decay_period_ledgers: u64,
        dispute_timeout_ledgers: u64,
        xlm_sac_address: Address,
    ) -> Result<(), ContractError> {
        crate::config::update_config(
            &env,
            &caller,
            high_rep_threshold,
            bonus_bps,
            min_discount_rate_bps,
            decay_rate_bps,
            decay_period_ledgers,
            dispute_timeout_ledgers,
            xlm_sac_address,
        )
        .map_err(|_| ContractError::Unauthorized)
    }

    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        require_admin(&env)?;
        let old_admin: Address = env.storage().instance().get(&StorageKey::Admin).unwrap();
        env.storage().instance().set(&StorageKey::Admin, &new_admin);
        env.events().publish_event(&AdminChanged {
            old_admin,
            new_admin,
            timestamp: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn update_fee_rate(env: Env, rate: u32) -> Result<(), ContractError> {
        require_admin(&env)?;
        let old_rate: u32 = env.storage().instance().get(&StorageKey::FeeRate).unwrap_or(0);
        env.storage().instance().set(&StorageKey::FeeRate, &rate);
        let updated_by = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        env.events().publish_event(&ParameterUpdated {
            param_name: Symbol::new(&env, "protocol_fee_rate_bps"),
            old_value: old_rate as i128,
            new_value: rate as i128,
            updated_by,
        });
        Ok(())
    }

    /// Access: Admin only
    pub fn update_protocol_fee_bps(env: Env, bps: u32) -> Result<(), ContractError> {
        require_admin(&env)?;
        if bps > 100 {
            return Err(ContractError::InvalidDiscountRate);
        }
        let old_bps: u32 = env
            .storage()
            .instance()
            .get(&crate::storage::DataKey::ProtocolFeeBps)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&crate::storage::DataKey::ProtocolFeeBps, &bps);
            
        let updated_by = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        env.events().publish_event(&ParameterUpdated {
            param_name: Symbol::new(&env, "protocol_fee_bps"),
            old_value: old_bps as i128,
            new_value: bps as i128,
            updated_by,
        });
        Ok(())
    }

    /// Access: Admin only
    pub fn set_treasury_address(env: Env, treasury: Address) -> Result<(), ContractError> {
        require_admin(&env)?;
        env.storage()
            .instance()
            .set(&crate::storage::DataKey::TreasuryAddress, &treasury);
        Ok(())
    }

    /// Access: Admin only
    pub fn update_max_discount(env: Env, rate: u32) -> Result<(), ContractError> {
        require_admin(&env)?;
        let old_rate: u32 = env
            .storage()
            .instance()
            .get(&StorageKey::MaxDiscountRate)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&StorageKey::MaxDiscountRate, &rate);
        let updated_by = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        env.events().publish_event(&ParameterUpdated {
            param_name: Symbol::new(&env, "max_discount_rate_bps"),
            old_value: old_rate as i128,
            new_value: rate as i128,
            updated_by,
        });
        Ok(())
    }

    pub fn set_distribution_contract(
        env: Env,
        distribution_contract: Address,
    ) -> Result<(), ContractError> {
        require_admin(&env)?;
        env.storage()
            .instance()
            .set(&StorageKey::DistributionContract, &distribution_contract);
        Ok(())
    }

    pub fn set_price_oracle(env: Env, oracle: Address) -> Result<(), ContractError> {
        require_admin(&env)?;
        let admin = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        crate::config::set_price_oracle(&env, &admin, oracle)
            .map_err(|_| ContractError::Unauthorized)?;
        Ok(())
    }

    pub fn get_price_oracle(env: Env) -> Option<Address> {
        get_config(&env).and_then(|config| config.price_oracle)
    }

    /// Update the maximum oracle data age in ledgers. Admin / governance only.
    ///
    /// Setting this to 0 disables the freshness check entirely (not recommended
    /// for production — stale data is as dangerous as no oracle).
    /// Access: Admin only
    pub fn set_max_oracle_age(env: Env, max_age_ledgers: u64) -> Result<(), ContractError> {
        require_admin(&env)?;
        let admin = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        crate::config::set_max_oracle_age(&env, &admin, max_age_ledgers)
            .map_err(|_| ContractError::Unauthorized)?;
        Ok(())
    }

    /// Return the configured maximum oracle data age in ledgers.
    /// Access: Anyone
    pub fn get_max_oracle_age(env: Env) -> u64 {
        crate::storage::get_config(&env)
            .map(|c| c.max_oracle_age_ledgers)
            .unwrap_or(DEFAULT_MAX_ORACLE_AGE_LEDGERS)
    }

    /// Access: Admin only
    ///
    /// Reject tokens that implement fee-on-transfer behavior by ensuring a small
    /// token transfer to the contract results in the same amount being received.
    pub fn add_token(env: Env, token: Address) -> Result<(), ContractError> {
        require_admin(&env)?;
        env.storage()
            .persistent()
            .set(&StorageKey::ApprovedToken(token.clone()), &true);

        let token_client = token_client(&env, &token);
        let contract_address = env.current_contract_address();
        let test_amount: i128 = 1_000_000;
        let admin_address: Address = env
            .storage()
            .instance()
            .get(&crate::storage::DataKey::Admin)
            .unwrap();
        let before_balance = token_client.balance(&contract_address);

        token_client.transfer(&admin_address, &contract_address, &test_amount);

        let after_balance = token_client.balance(&contract_address);
        let received = after_balance.checked_sub(before_balance).unwrap_or(0);
        if received != test_amount {
            if received > 0 {
                token_client.transfer(&contract_address, &admin_address, &received);
            }
            return Err(ContractError::FeeOnTransferToken);
        }

        // Return the exact test amount to the admin account after verification.
        token_client.transfer(&contract_address, &admin_address, &test_amount);

        env.storage().persistent().set(
            &crate::storage::DataKey::ApprovedToken(token.clone()),
            &true,
        );

        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&StorageKey::TokenList)
            .unwrap_or(Vec::new(&env));
        if !list.contains(&token) {
            list.push_back(token.clone());
            env.storage().persistent().set(&StorageKey::TokenList, &list);
        }

        env.events().publish_event(&TokenAdded { token });
        Ok(())
    }

    pub fn remove_token(env: Env, token: Address) -> Result<(), ContractError> {
        require_admin(&env)?;
        env.storage()
            .persistent()
            .set(&StorageKey::ApprovedToken(token.clone()), &false);

        let list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&StorageKey::TokenList)
            .unwrap_or(Vec::new(&env));
        let mut pruned: Vec<Address> = Vec::new(&env);
        for t in list.iter() {
            if t != token {
                pruned.push_back(t);
            }
        }
        env.storage()
            .persistent()
            .set(&StorageKey::TokenList, &pruned);

        env.events().publish_event(&TokenRemoved { token });
        Ok(())
    }

    pub fn pause(env: Env) -> Result<(), ContractError> {
        require_admin(&env)?;
        set_paused(&env, true);
        env.events().publish_event(&ContractPaused {
            timestamp: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), ContractError> {
        require_admin(&env)?;
        set_paused(&env, false);
        env.events().publish_event(&ContractUnpaused {
            timestamp: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), ContractError> {
        require_admin(&env)?;
        let admin = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        env.events().publish_event(&ContractUpgraded {
            admin,
            new_wasm_hash,
            timestamp: env.ledger().timestamp(),
        });
        Ok(())
    }

    // ============================================================
    // Multi-sig Admin Functions (Issue #124)
    // ============================================================

    /// Initialize multi-signature admin functionality.
    ///
    /// Enables multi-sig approval for sensitive operations. Once enabled,
    /// certain admin actions require approval from multiple authorized signers.
    ///
    /// # Arguments
    /// - `env`: The Soroban environment
    /// - `signers`: Vec of addresses authorized to participate in multi-sig
    /// - `threshold`: Number of signatures required to execute (must be <= signers.len())
    ///
    /// # Returns
    /// - `Ok(())` if multi-sig admin was successfully initialized
    /// - `Err(ContractError::InvalidMultisigConfig)` if threshold > signers.len()
    /// - `Err(ContractError::Unauthorized)` if called by non-admin
    ///
    /// Access: Admin only
    pub fn initialize_multisig_admin(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), ContractError> {
        require_admin(&env)?;

        // Validate configuration
        if threshold as usize > signers.len() || threshold == 0 {
            return Err(ContractError::InvalidMultisigConfig);
        }

        let admin = multisig::MultisigAdmin { signers, threshold };
        storage::set_multisig_admin(&env, &admin);
        Ok(())
    }

    /// Propose a pause action.
    ///
    /// Creates a new proposal to pause the contract. Must be called by
    /// an authorized signer if multi-sig is enabled.
    ///
    /// # Arguments
    /// - `env`: The Soroban environment
    /// - `proposer`: The signer proposing the pause
    ///
    /// # Returns
    /// - `Ok(proposal_id)` if proposal was created successfully
    /// - `Err(ContractError::NotAuthorizedSigner)` if proposer is not authorized
    ///
    /// Access: Multi-sig authorized signer
    pub fn propose_pause(env: Env, proposer: Address) -> Result<u64, ContractError> {
        proposer.require_auth();

        let admin = storage::get_multisig_admin(&env)
            .ok_or(ContractError::NotAuthorizedSigner)?;

        if !multisig::is_signer(&env, &admin.signers, &proposer) {
            return Err(ContractError::NotAuthorizedSigner);
        }

        let proposal_id = storage::get_next_proposal_id(&env);
        let proposal = multisig::MultisigProposal {
            id: proposal_id,
            action: multisig::AdminAction::Pause,
            signers_approved: Vec::new(&env),
            state: multisig::ProposalState::Pending,
            expires_at: env.ledger().sequence() + multisig::MULTISIG_WINDOW_LEDGERS,
        };

        storage::save_multisig_proposal(&env, &proposal);
        storage::increment_proposal_id(&env);

        Ok(proposal_id)
    }

    /// Propose an unpause action.
    ///
    /// Creates a new proposal to unpause the contract. Must be called by
    /// an authorized signer if multi-sig is enabled.
    ///
    /// # Arguments
    /// - `env`: The Soroban environment
    /// - `proposer`: The signer proposing the unpause
    ///
    /// # Returns
    /// - `Ok(proposal_id)` if proposal was created successfully
    /// - `Err(ContractError::NotAuthorizedSigner)` if proposer is not authorized
    ///
    /// Access: Multi-sig authorized signer
    pub fn propose_unpause(env: Env, proposer: Address) -> Result<u64, ContractError> {
        proposer.require_auth();

        let admin = storage::get_multisig_admin(&env)
            .ok_or(ContractError::NotAuthorizedSigner)?;

        if !multisig::is_signer(&env, &admin.signers, &proposer) {
            return Err(ContractError::NotAuthorizedSigner);
        }

        let proposal_id = storage::get_next_proposal_id(&env);
        let proposal = multisig::MultisigProposal {
            id: proposal_id,
            action: multisig::AdminAction::Unpause,
            signers_approved: Vec::new(&env),
            state: multisig::ProposalState::Pending,
            expires_at: env.ledger().sequence() + multisig::MULTISIG_WINDOW_LEDGERS,
        };

        storage::save_multisig_proposal(&env, &proposal);
        storage::increment_proposal_id(&env);

        Ok(proposal_id)
    }

    /// Sign a proposal.
    ///
    /// Adds the signer's signature to a proposal. Once the signature threshold
    /// is reached, the proposal becomes executable.
    ///
    /// # Arguments
    /// - `env`: The Soroban environment
    /// - `signer`: The address signing the proposal
    /// - `proposal_id`: The ID of the proposal to sign
    ///
    /// # Returns
    /// - `Ok(())` if signature was added successfully
    /// - `Err(ContractError::NotAuthorizedSigner)` if signer is not authorized
    /// - `Err(ContractError::AlreadySigned)` if signer has already signed this proposal
    /// - `Err(ContractError::ProposalNotFound)` if proposal doesn't exist
    ///
    /// Access: Multi-sig authorized signer
    pub fn sign_proposal(
        env: Env,
        signer: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        signer.require_auth();

        let admin = storage::get_multisig_admin(&env)
            .ok_or(ContractError::NotAuthorizedSigner)?;

        if !multisig::is_signer(&env, &admin.signers, &signer) {
            return Err(ContractError::NotAuthorizedSigner);
        }

        let mut proposal = storage::get_multisig_proposal(&env, proposal_id)
            .ok_or(ContractError::ProposalNotFound)?;

        if multisig::has_signed(&proposal, &signer) {
            return Err(ContractError::AlreadySigned);
        }

        proposal.signers_approved.push_back(signer);
        storage::save_multisig_proposal(&env, &proposal);

        Ok(())
    }

    /// Execute a proposal.
    ///
    /// Executes a proposal that has reached the signature threshold.
    /// The action (pause/unpause) is immediately applied.
    ///
    /// # Arguments
    /// - `env`: The Soroban environment
    /// - `executor`: The address executing the proposal (must be a signer)
    /// - `proposal_id`: The ID of the proposal to execute
    ///
    /// # Returns
    /// - `Ok(())` if proposal was executed successfully
    /// - `Err(ContractError::ThresholdNotReached)` if not enough signatures
    /// - `Err(ContractError::ProposalNotFound)` if proposal doesn't exist
    /// - `Err(ContractError::ProposalAlreadyExecuted)` if already executed
    /// - `Err(ContractError::ProposalExpired)` if outside execution window
    ///
    /// Access: Multi-sig authorized signer
    pub fn execute_proposal(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        executor.require_auth();

        let admin = storage::get_multisig_admin(&env)
            .ok_or(ContractError::NotAuthorizedSigner)?;

        if !multisig::is_signer(&env, &admin.signers, &executor) {
            return Err(ContractError::NotAuthorizedSigner);
        }

        let mut proposal = storage::get_multisig_proposal(&env, proposal_id)
            .ok_or(ContractError::ProposalNotFound)?;

        // Check if already executed
        if proposal.state == multisig::ProposalState::Executed {
            return Err(ContractError::ProposalAlreadyExecuted);
        }

        // Check if expired
        if multisig::is_expired(&env, &proposal) {
            proposal.state = multisig::ProposalState::Expired;
            storage::save_multisig_proposal(&env, &proposal);
            return Err(ContractError::ProposalExpired);
        }

        // Check threshold
        if !multisig::threshold_reached(&proposal, admin.threshold) {
            return Err(ContractError::ThresholdNotReached);
        }

        // Mark as executed and execute action
        proposal.state = multisig::ProposalState::Executed;
        storage::save_multisig_proposal(&env, &proposal);

        match proposal.action {
            multisig::AdminAction::Pause => {
                set_paused(&env, true);
                env.events().publish_event(&ContractPaused {
                    timestamp: env.ledger().timestamp(),
                });
            }
            multisig::AdminAction::Unpause => {
                set_paused(&env, false);
                env.events().publish_event(&ContractUnpaused {
                    timestamp: env.ledger().timestamp(),
                });
            }
            _ => {
                // Other actions not yet implemented in this simplified version
            }
        }

        Ok(())
    }

    // ============================================================
    // END Multi-sig Admin Functions
    // ============================================================

    // ------------------------------------------------------------
    // get_contract_stats (read-only view)
    // ------------------------------------------------------------
    /// Access: Anyone
    pub fn get_contract_stats(env: Env) -> ContractStats {
        invoice::get_contract_stats(&env)
    }

    // ------------------------------------------------------------
    // get_lp_portfolio_stats (read-only view) — Issue #116
    // ------------------------------------------------------------
    /// Return the LP yield analytics snapshot for `lp`.
    ///
    /// All fields are maintained incrementally in persistent storage and are
    /// O(1) to read, making this ideal for LP dashboards that need to avoid
    /// paginating through every invoice.
    ///
    /// # Fields
    /// - `total_funded`   — cumulative capital deployed
    /// - `total_earned`   — cumulative yield received
    /// - `active_positions` — invoices currently in `Funded` state
    /// - `total_positions`  — all-time funded invoice count
    /// - `avg_yield_bps`  — running average discount rate in basis points
    ///
    /// Access: Anyone
    pub fn get_lp_portfolio_stats(env: Env, lp: Address) -> LPStats {
        storage_get_lp_portfolio_stats(&env, &lp)
    }

    // ------------------------------------------------------------
    // get_invoice_count (O(1) counter view) — Issue #115
    // ------------------------------------------------------------
    /// Return the total number of invoices, or the count of invoices currently
    /// in a specific state.
    /// Access: Anyone
    pub fn get_invoice_count(env: Env, state: Option<InvoiceStatus>) -> u64 {
        match state {
            None => {
                env.storage()
                    .persistent()
                    .get(&DataKey::TotalInvoices)
                    .unwrap_or(0)
            }
            Some(status) => crate::storage::get_state_count(&env, &status),
        }
    }

    // ------------------------------------------------------------
    // list_invoices_by_submitter (Paginated)
    // ------------------------------------------------------------
    /// Access: Anyone
    pub fn list_invoices_by_submitter(
        env: Env,
        submitter: Address,
        page: u32,
        page_size: u32,
    ) -> Vec<Invoice> {
        let page_size = page_size.min(50);
        let invoice_ids = get_submitter_invoices(&env, &submitter);
        let total = invoice_ids.len();
        let start = page * page_size;
        if start >= total {
            return Vec::new(&env);
        }
        let end = (start + page_size).min(total);
        let mut result = Vec::new(&env);
        for i in start..end {
            if let Some(id) = invoice_ids.get(i) {
                result.push_back(load_invoice(&env, id));
            }
        }
        result
    }

    pub fn list_invoices_by_lp(env: Env, lp: Address, page: u32, page_size: u32) -> Vec<Invoice> {
        let page_size = page_size.min(50);
        let invoice_ids = get_lp_invoices(&env, &lp);
        let total = invoice_ids.len();
        let start = page * page_size;
        if start >= total {
            return Vec::new(&env);
        }
        let end = (start + page_size).min(total);
        let mut result = Vec::new(&env);
        for i in start..end {
            if let Some(id) = invoice_ids.get(i) {
                result.push_back(load_invoice(&env, id));
            }
        }
        result
    }

    pub fn submit_invoice(
        env: Env,
        freelancer: Address,
        payer: Address,
        amount: i128,
        due_date: u64,
        discount_rate: u32,
        token: Address,
        referral_code: Option<BytesN<32>>,
        allowed_lps: Option<Vec<Address>>,
    ) -> Result<u64, ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }
        require_submitter(&env, &freelancer)?;
        if freelancer == payer {
            return Err(ContractError::SelfInvoice);
        }
        validate_invoice_terms(&env, amount, due_date, discount_rate)?;
        if !is_approved_token(&env, &token) {
            return Err(ContractError::Unauthorized);
        }

        // Issue #122: Validate LP whitelist size (max 10)
        if let Some(ref lps) = allowed_lps {
            if lps.len() > 10 {
                return Err(ContractError::WhitelistTooLarge);
            }
        }

        let id = next_invoice_id(&env)?;
        let submitter_reputation = get_payer_score(&env, &freelancer);

        let invoice = Invoice {
            id,
            freelancer: freelancer.clone(),
            payer: payer.clone(),
            token: token.clone(),
            amount,
            due_date: due_date.try_into().unwrap(),
            discount_rate,
            status: InvoiceStatus::Pending,
            funder: None,
            funded_at: None,
            amount_funded: 0,
            amount_paid: 0,
            referral_code: referral_code.clone(),
            submitter_reputation,
            allowed_lps: allowed_lps.clone(),
        };

        save_invoice(&env, &invoice);
        add_invoice_to_submitter(&env, &freelancer, id);

        // Increment detailed reputation invoices_submitted count
        increment_invoices_submitted(&env, &freelancer);

        // OPTIMIZATION: Batch stat update
        let mut stats_delta = StatsDelta::default();
        stats_delta.total_invoices = 1;
        stats_delta.apply(&env);
        // Issue #119: Mint NFT representing the invoice to the freelancer
        crate::nft::mint_invoice_nft(
            &env,
            id,
            freelancer.clone(),
            amount,
            due_date.try_into().unwrap(),
            discount_rate,
            token.clone(),
        )?;

        env.events().publish_event(&InvoiceSubmitted {
            invoice_id: id,
            freelancer,
            payer: invoice.payer,
            token: invoice.token,
            amount,
            due_date: u64::from(invoice.due_date),
            discount_rate,
            status: invoice.status,
            discount_rate: invoice.discount_rate,
            referral_code: referral_code.clone(),
            status: invoice.status.clone(),
            timestamp: env.ledger().timestamp(),
            allowed_lps: allowed_lps.clone(),
        });

        // Track referral count if provided
        if let Some(code) = referral_code {
            let key = crate::storage::DataKey::ReferralCount(code.clone());
            let current: u64 = env
                .storage()
                .persistent()
                .get(&key)
                .unwrap_or(0);
            env.storage().persistent().set(&key, &(current + 1));
        }

        Ok(id)
    }

    // ----------------------------------------------------------------
    // submit_invoice_auction
    // ----------------------------------------------------------------
    /// Access: Submitter only
    ///
    /// Creates an invoice with Dutch auction funding.
    /// The rate starts high and decreases linearly over time until the first LP accepts.
    pub fn submit_invoice_auction(
        env: Env,
        freelancer: Address,
        payer: Address,
        amount: i128,
        due_date: u64,
        start_rate: u32,           // starting rate in basis points
        min_rate: u32,             // minimum rate in basis points
        rate_decay_per_hour: u32,  // decay in basis points per hour
        token: Address,
        referral_code: Option<BytesN<32>>,
    ) -> Result<u64, ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }

        require_submitter(&env, &freelancer)?;

        if freelancer == payer {
            return Err(ContractError::SelfInvoice);
        }

        // Validate auction parameters
        if start_rate == 0 || start_rate > crate::constants::MAX_DISCOUNT_RATE {
            return Err(ContractError::InvalidAuctionParams);
        }
        if min_rate > start_rate {
            return Err(ContractError::InvalidAuctionParams);
        }
        if rate_decay_per_hour == 0 {
            return Err(ContractError::InvalidAuctionParams);
        }

        // Validate invoice terms using the start_rate as the discount rate for validation
        validate_invoice_terms(&env, amount, due_date, start_rate)?;

        // token validation
        if !is_approved_token(&env, &token) {
            return Err(ContractError::Unauthorized);
        }

        let id = next_invoice_id(&env)?;

        // Capture the freelancer's reputation score at submission time
        let submitter_reputation = get_payer_score(&env, &freelancer);
        let current_time = env.ledger().timestamp();

        let invoice = Invoice {
            id,
            freelancer: freelancer.clone(),
            payer: payer.clone(),
            token: token.clone(),
            amount,
            due_date: due_date.try_into().unwrap(),
            discount_rate: start_rate,
            status: InvoiceStatus::Pending,
            funder: None,
            funded_at: None,
            amount_funded: 0,
            amount_paid: 0,
            referral_code: referral_code.clone(),
            submitter_reputation,
            // Auction fields
            is_auction: true,
            auction_start_rate: Some(start_rate),
            auction_min_rate: Some(min_rate),
            auction_rate_decay_per_hour: Some(rate_decay_per_hour),
            auction_started_at: Some(current_time.try_into().unwrap()),
        };

        save_invoice(&env, &invoice);

        // Update submitter index
        add_invoice_to_submitter(&env, &freelancer, id);

        // Increment total invoices counter
        increment_total_invoices(&env);

        // Increment detailed reputation invoices_submitted count
        increment_invoices_submitted(&env, &freelancer);

        env.events().publish_event(&AuctionStarted {
            invoice_id: invoice.id,
            freelancer: invoice.freelancer.clone(),
            payer: invoice.payer.clone(),
            token: invoice.token.clone(),
            amount: invoice.amount,
            due_date: u64::from(invoice.due_date),
            start_rate,
            min_rate,
            rate_decay_per_hour,
            started_at: current_time,
        });

        // Track referral count if provided
        if let Some(code) = referral_code {
            let key = crate::storage::DataKey::ReferralCount(code.clone());
            let current: u64 = env
                .storage()
                .persistent()
                .get(&key)
                .unwrap_or(0);
            env.storage().persistent().set(&key, &(current + 1));
        }

        Ok(id)
        }

        pub fn update_invoice(
        env: Env,
        freelancer: Address,
        invoice_id: u64,
        amount: i128,
        due_date: u64,
        discount_rate: u32,
        ) -> Result<(), ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }

        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);
        require_submitter_by_id(&env, &freelancer, invoice_id)?;

        if invoice.status == InvoiceStatus::Pending
            && env.ledger().timestamp() >= u64::from(invoice.due_date)
        {
            invoice.status = InvoiceStatus::Expired;
            save_invoice(&env, &invoice);
            return Err(ContractError::InvoiceExpired);
        }

        match invoice.status {
            InvoiceStatus::Pending => {}
            InvoiceStatus::PartiallyFunded | InvoiceStatus::Funded => {
                return Err(ContractError::AlreadyFunded)
            }
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
        }

        validate_invoice_terms(&env, amount, due_date, discount_rate)?;

        invoice.amount = amount;
        invoice.due_date = due_date.try_into().unwrap();
        invoice.discount_rate = discount_rate;

        save_invoice(&env, &invoice);

        env.events().publish_event(&InvoiceUpdated {
            invoice_id: invoice.id,
            freelancer: invoice.freelancer.clone(),
            payer: invoice.payer.clone(),
            token: invoice.token.clone(),
            amount: invoice.amount,
            due_date: u64::from(invoice.due_date),
            discount_rate: invoice.discount_rate,
            status: invoice.status.clone(),
        });

        Ok(())
        }


    // ------------------------------------------------------------
    // convert_invoice_token
    // ------------------------------------------------------------
    /// Access: Submitter only
    pub fn convert_invoice_token(
        env: Env,
        freelancer: Address,
        invoice_id: u64,
        new_token: Address,
    ) -> Result<(), ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }

        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);
        require_submitter_by_id(&env, &freelancer, invoice_id)?;

        // Only allowed in Pending state
        if invoice.status != InvoiceStatus::Pending {
            match invoice.status {
                InvoiceStatus::PartiallyFunded | InvoiceStatus::Funded => {
                    return Err(ContractError::AlreadyFunded)
                }
                InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
                _ => return Err(ContractError::Unauthorized), // Generic unauthorized for other states
            }
        }

        // Check if invoice is expired (mirroring update_invoice logic)
        if env.ledger().timestamp() >= u64::from(invoice.due_date) {
            invoice.status = InvoiceStatus::Expired;
            save_invoice(&env, &invoice);
            return Err(ContractError::InvoiceExpired);
        }

        // New token must be in the allowlist
        if !is_approved_token(&env, &new_token) {
            return Err(ContractError::Unauthorized);
        }

        let old_token = invoice.token.clone();
        invoice.token = new_token.clone();

        save_invoice(&env, &invoice);

        env.events().publish_event(&InvoiceTokenChanged {
            invoice_id,
            old_token,
            new_token,
        });

        Ok(())
    }

    // ------------------------------------------------------------
    // submit_invoices_batch
    // ------------------------------------------------------------
    /// Access: Submitter only
    pub fn submit_invoices_batch(
        env: Env,
        invoices: Vec<InvoiceParams>,
    ) -> Result<Vec<u64>, ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }
        if invoices.len() > 10 {
            return Err(ContractError::BatchTooLarge);
        }

        let mut authenticated_freelancers: Vec<Address> = Vec::new(&env);
        let mut ids = Vec::new(&env);
        let mut stats_delta = StatsDelta::default();

        // OPTIMIZATION: Batch ID generation
        let mut next_id = next_invoice_ids(&env, invoices.len())?;

        for params in invoices.iter() {
            if !authenticated_freelancers.contains(&params.freelancer) {
                require_submitter(&env, &params.freelancer)?;
                authenticated_freelancers.push_back(params.freelancer.clone());
            }
            validate_invoice_terms(&env, params.amount, params.due_date, params.discount_rate)?;
            if !is_approved_token(&env, &params.token) {
                return Err(ContractError::Unauthorized);
            }

            let id = next_id;
            next_id += 1;
            // Issue #122: Validate LP whitelist size (max 10)
            if let Some(ref lps) = params.allowed_lps {
                if lps.len() > 10 {
                    return Err(ContractError::WhitelistTooLarge);
                }
            }

            let id = next_invoice_id(&env)?;

            let submitter_reputation = get_payer_score(&env, &params.freelancer);
            let invoice = Invoice {
                id,
                freelancer: params.freelancer.clone(),
                payer: params.payer.clone(),
                token: params.token.clone(),
                amount: params.amount,
                due_date: params.due_date.try_into().unwrap(),
                discount_rate: params.discount_rate,
                status: InvoiceStatus::Pending,
                funder: None,
                funded_at: None,
                amount_funded: 0,
                amount_paid: 0,
                referral_code: params.referral_code.clone(),
                submitter_reputation,
                allowed_lps: params.allowed_lps.clone(),
            };

            save_invoice(&env, &invoice);
            add_invoice_to_submitter(&env, &params.freelancer, id);
            
            // Increment detailed reputation invoices_submitted count
            increment_invoices_submitted(&env, &params.freelancer);

            stats_delta.total_invoices += 1;

            env.events().publish_event(&InvoiceSubmitted {
                invoice_id: id,
                freelancer: params.freelancer,
                payer: invoice.payer,
                token: invoice.token,
                amount: invoice.amount,
                due_date: u64::from(invoice.due_date),
                discount_rate: invoice.discount_rate,
                status: invoice.status,
                timestamp: env.ledger().timestamp(),
            });
                referral_code: params.referral_code.clone(),
                status: invoice.status.clone(),
                timestamp: env.ledger().timestamp(),
            });

            // Track referral count if provided
            if let Some(code) = params.referral_code {
                let key = crate::storage::DataKey::ReferralCount(code.clone());
                let current: u64 = env
                    .storage()
                    .persistent()
                    .get(&key)
                    .unwrap_or(0);
                env.storage().persistent().set(&key, &(current + 1));
            }

            ids.push_back(id);
        }

        // OPTIMIZATION: Apply all stat changes in one write
        stats_delta.apply(&env);
        Ok(ids)
    }

    /// Access: Anyone
    pub fn get_referral_stats(env: Env, code: BytesN<32>) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::ReferralCount(code))
            .unwrap_or(0)
    }

    // ================================================================
    // Issue #34: LP Priority Queue
    //
    // Design:
    //  1. Any LP calls `join_fund_queue(lp, invoice_id)` to register intent.
    //     Their current LP reputation score is snapshotted.
    //  2. Anyone can call `resolve_fund_queue(invoice_id)` to lock in the
    //     highest-score LP as the approved funder.
    //  3. `fund_invoice` checks: if a QueueResolution exists for this invoice,
    //     only the approved LP may fund it.
    //  If no LP ever joins the queue the existing first-come-first-served
    //  behaviour is preserved unchanged.
    // ================================================================

    /// Register an LP's intent to fund an invoice.
    /// The LP's current reputation score is snapshotted for ordering.
    /// Access: LP only
    pub fn join_fund_queue(env: Env, lp: Address, invoice_id: u64) -> Result<(), ContractError> {
        require_lp(&env, &lp)?;

        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        // Queue resolution already happened — too late to join.
        if get_queue_resolution(&env, invoice_id).is_some() {
            return Err(ContractError::NotApprovedFunder);
        }

        let invoice = load_invoice(&env, invoice_id);
        match invoice.status {
            InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {}
            InvoiceStatus::Funded => return Err(ContractError::AlreadyFunded),
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
        }

        let mut queue = get_fund_queue(&env, invoice_id);

        // Prevent duplicate entries.
        for i in 0..queue.len() {
            if queue.get(i).unwrap().lp == lp {
                return Err(ContractError::AlreadyInQueue);
            }
        }

        let score = get_lp_score(&env, &lp);
        queue.push_back(LpFundRequest {
            lp: lp.clone(),
            score,
        });
        save_fund_queue(&env, invoice_id, &queue);

        env.events().publish_event(&FundRequested {
            invoice_id,
            lp,
            score,
        });

        Ok(())
    }

    /// Select the highest-reputation LP from the queue as the approved funder.
    /// Returns the winning LP address.
    /// Can be called by anyone once at least one LP has joined the queue.
    /// Access: Anyone
    pub fn resolve_fund_queue(env: Env, invoice_id: u64) -> Result<Address, ContractError> {
        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        // Already resolved.
        if let Some(approved) = get_queue_resolution(&env, invoice_id) {
            return Ok(approved);
        }

        let queue = get_fund_queue(&env, invoice_id);
        if queue.is_empty() {
            return Err(ContractError::NotFunded); // no one in queue
        }

        // Find the LP with the highest score (ties broken by first-come-first-served).
        let mut best_lp = queue.get(0).unwrap().lp.clone();
        let mut best_score = queue.get(0).unwrap().score;

        for i in 1..queue.len() {
            let entry = queue.get(i).unwrap();
            if entry.score > best_score {
                best_score = entry.score;
                best_lp = entry.lp.clone();
            }
        }

        save_queue_resolution(&env, invoice_id, &best_lp);

        env.events().publish_event(&FundQueueResolved {
            invoice_id,
            approved_lp: best_lp.clone(),
            score: best_score,
        });

        Ok(best_lp)
    }

    // ────────────────────────────────────────────────────────────
    // fund_invoice (USES invoice.token) — now queue-aware & reentrancy-guarded
    // ────────────────────────────────────────────────────────────
    /// Access: LP only
    ///
    /// `require_oracle_verification` — when `true`, the oracle stored in
    /// contract config is queried for the payer's verification status.
    /// If the oracle returns `false` (unverified), the call returns
    /// `ContractError::PayerUnverified`. When `false`, the oracle is not
    /// consulted and the existing behaviour is preserved.
    pub fn fund_invoice(
        env: Env,
        funder: Address,
        invoice_id: u64,
        fund_amount: i128,
        require_oracle_verification: bool,
    ) -> Result<(), ContractError> {
        with_reentrancy_guard(&env, || {
            if is_paused(&env) {
                return Err(ContractError::ContractPaused);
            }

            require_lp(&env, &funder)?;

            // Issue #71: load the invoice once instead of `invoice_exists` + `load_invoice`
            // (which read the same persistent key twice on the hottest path).
            let mut invoice =
                try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;

            // ── Issue #34: priority queue check ──────────────────────
            // If a queue has been resolved, only the approved LP may fund.
            if let Some(approved) = get_queue_resolution(&env, invoice_id) {
                if approved != funder {
                    return Err(ContractError::NotApprovedFunder);
                }
            }

            // Issue #19: the invoice token must still be on the governance allowlist.
            if !is_approved_token(&env, &invoice.token) {
                return Err(ContractError::Unauthorized);
            }

            // Issue #28: reject funding when the payer's reputation is below the
            // configured minimum threshold (default 0 allows everyone).
            let min_payer_reputation = get_min_payer_reputation(&env);
            if min_payer_reputation > 0
                && get_payer_score(&env, &invoice.payer) < min_payer_reputation
            {
                return Err(ContractError::PayerReputationTooLow);
            }

            if invoice.status == InvoiceStatus::Pending
                && env.ledger().timestamp() >= u64::from(invoice.due_date)
            {
                invoice.status = InvoiceStatus::Expired;
                save_invoice(&env, &invoice);
                return Err(ContractError::InvoiceExpired);
            }

            match invoice.status {
                InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
                InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
                InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
                InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
                InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
                InvoiceStatus::Funded => return Err(ContractError::AlreadyFunded),
                InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {} // all good
                InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
            }

            if invoice.amount_funded + fund_amount > invoice.amount {
                return Err(ContractError::OverfundingRejected);
            }

            // --- Execute transfer ---
            let token = token_client(&env, &invoice.token);
            let contract_address = env.current_contract_address();

            // Handle XLM precision if needed (SAC wrapper handles conversion internally)
            let normalized_fund_amount = if is_xlm_token(&env, &invoice.token) {
                normalize_xlm_amount(fund_amount)
            } else {
                normalize_usdc_amount(fund_amount)
            };

            let fund_discount = normalized_fund_amount
                .checked_mul(discount_rate_as_i128(invoice.discount_rate))
                .unwrap_or(0)
                / 10_000;
            let cost = normalized_fund_amount - fund_discount;

            token.transfer(&funder, &contract_address, &cost);

            // --- Update contributor list ---
            let mut funders = get_invoice_funders(&env, invoice_id);
            let mut found = false;
            for i in 0..funders.len() {
                let (addr, amt) = funders.get(i).unwrap();
                if addr == funder {
                    funders.set(i, (addr, amt + fund_amount));
                    found = true;
                    break;
                }
            }
            if !found {
                funders.push_back((funder.clone(), fund_amount));
            }
            save_invoice_funders(&env, invoice_id, &funders);

            // --- Update invoice state ---
            invoice.amount_funded += fund_amount;

            if invoice.amount_funded == invoice.amount {
                // Fully funded — pay out to freelancer
                let discount_amount = invoice
                    .amount
                    .checked_mul(discount_rate_as_i128(invoice.discount_rate))
                    .unwrap_or(0)
                    / 10_000;
                let freelancer_payout = invoice.amount - discount_amount;

                token.transfer(&contract_address, &invoice.freelancer, &freelancer_payout);

                invoice.status = InvoiceStatus::Funded;
                invoice.funded_at = Some(env.ledger().timestamp().try_into().unwrap());
                invoice.funder = Some(funder.clone());

                // Boost LP score on successful funding
                let current_lp_score = get_lp_score(&env, &funder);
                set_lp_score(&env, &funder, current_lp_score + 1);
            } else {
                invoice.status = InvoiceStatus::PartiallyFunded;
            }

            save_invoice(&env, &invoice);

            // Update LP index
            add_invoice_to_lp(&env, &funder, invoice_id);

            // Increment total funded counter if fully funded
            if invoice.status == InvoiceStatus::Funded {
                increment_total_funded(&env);
            }

            add_volume(&env, &invoice.token, fund_amount);

            notify_distribution_funding(&env, &funder, fund_amount);

            let now = env.ledger().timestamp();

            let seconds_to_due = if u64::from(invoice.due_date) > now {
                u64::from(invoice.due_date) - now
            } else {
                0
            };

            let days_to_due = seconds_to_due / (24 * 60 * 60);

            let effective_yield_bps = ((invoice.discount_rate as u64 * days_to_due) / 365) as u32;

            env.events().publish_event(&InvoiceFunded {
                invoice_id: invoice.id,
                funder: funder.clone(),
                freelancer: invoice.freelancer.clone(),
                payer: invoice.payer.clone(),
                token: invoice.token.clone(),
                fund_amount,
                amount_funded: invoice.amount_funded,
                invoice_amount: invoice.amount,
                due_date: u64::from(invoice.due_date),
                discount_rate: invoice.discount_rate,
                funded_at: invoice.funded_at.map(|ts| ts.into()),
                status: invoice.status.clone(),

                // NEW
                lp: funder.clone(),
                effective_yield_bps,
                timestamp: now,
            });

            Ok(())
        })
    }

    // ────────────────────────────────────────────────────────────
    // transfer_invoice
    // ────────────────────────────────────────────────────────────
    /// Access: Submitter only
    pub fn transfer_invoice(
        env: Env,
        invoice_id: u64,
        new_freelancer: Address,
    ) -> Result<(), ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }
        require_lp(&env, &funder)?;

        let mut invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;


        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);

        require_submitter_by_id(&env, &invoice.freelancer, invoice_id)?;

        match invoice.status {
            InvoiceStatus::Pending => {}
            InvoiceStatus::PartiallyFunded | InvoiceStatus::Funded => {
                return Err(ContractError::AlreadyFunded)
            }
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
        }

        let old_freelancer = invoice.freelancer.clone();
        invoice.freelancer = new_freelancer.clone();

        save_invoice(&env, &invoice);

        // Update submitter index
        remove_invoice_from_submitter(&env, &old_freelancer, invoice_id);
        add_invoice_to_submitter(&env, &new_freelancer, invoice_id);

        env.events().publish_event(&InvoiceTransferred {
            invoice_id,
            old_freelancer,
            new_freelancer,
            status: invoice.status.clone(),
        });

        Ok(())
    }

    // ────────────────────────────────────────────────────────────
    // cancel_invoice (Reentrancy Protected)
    // ────────────────────────────────────────────────────────────
    /// Access: Submitter only
    /// **Reentrancy Protected:** Yes - This function performs token transfers
    pub fn cancel_invoice(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        with_reentrancy_guard(&env, || {
            if is_paused(&env) {
                return Err(ContractError::ContractPaused);
            }

            if !invoice_exists(&env, invoice_id) {
                return Err(ContractError::InvoiceNotFound);
            }

            let mut invoice = load_invoice(&env, invoice_id);

            require_submitter_by_id(&env, &invoice.freelancer, invoice_id)?;

            match invoice.status {
                InvoiceStatus::Pending => {}
                InvoiceStatus::PartiallyFunded => {
                    let funders = get_invoice_funders(&env, invoice_id);
                    let token = token_client(&env, &invoice.token);
                    let contract_address = env.current_contract_address();
                    for i in 0..funders.len() {
                        let (funder_addr, fund_amt) = funders.get(i).unwrap();
                        let fund_discount = fund_amt
                            .checked_mul(discount_rate_as_i128(invoice.discount_rate))
                            .unwrap_or(0)
                            / 10_000;
                        let refund = fund_amt - fund_discount;
                        token.transfer(&contract_address, &funder_addr, &refund);
                    }
                }
                InvoiceStatus::Funded => return Err(ContractError::AlreadyFunded),
                InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
                InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
                InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
                InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
                InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
                InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
            }

            invoice.status = InvoiceStatus::Cancelled;

            save_invoice(&env, &invoice);

            env.events().publish_event(&InvoiceCancelled {
                invoice_id,
                freelancer: invoice.freelancer.clone(),
                status: invoice.status.clone(),
            });

            Ok(())
        })
    }

    // ────────────────────────────────────────────────────────────
    // expire_invoice
    // ────────────────────────────────────────────────────────────
    /// Access: Anyone
    pub fn expire_invoice(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);

        if env.ledger().timestamp() < u64::from(invoice.due_date) {
            return Err(ContractError::NotYetDefaulted);
        }

        match invoice.status {
            InvoiceStatus::Pending => {
                invoice.status = InvoiceStatus::Expired;
                save_invoice(&env, &invoice);
                Ok(())
            }
            InvoiceStatus::PartiallyFunded | InvoiceStatus::Funded => {
                Err(ContractError::AlreadyFunded)
            }
            InvoiceStatus::Paid => Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => Err(ContractError::AlreadyCancelled),
        }
    }

    // ────────────────────────────────────────────────────────────
    // ── Issue #34: priority queue check ──────────────────────
        // If a queue has been resolved, only the approved LP may fund.
        if let Some(approved) = get_queue_resolution(&env, invoice_id) {
            if approved != funder {
                return Err(ContractError::NotApprovedFunder);
            }
        }

        // Issue #122: LP whitelist check for private invoices
        // If the invoice has an LP whitelist, verify the funder is in it.
        if let Some(ref allowed_lps) = invoice.allowed_lps {
            let mut is_whitelisted = false;
            for i in 0..allowed_lps.len() {
                if allowed_lps.get(i).unwrap() == funder {
                    is_whitelisted = true;
                    break;
                }
            }
            if !is_whitelisted {
                return Err(ContractError::LPNotWhitelisted);
            }
        }

        // Issue #19: the invoice token must still be on the governance allowlist.
        if !is_approved_token(&env, &invoice.token) {
            return Err(ContractError::Unauthorized);
        }

        let min_rep = get_min_payer_reputation(&env);
        if min_rep > 0 && get_payer_score(&env, &invoice.payer) < min_rep {
            return Err(ContractError::PayerReputationTooLow);
        }

        if invoice.status == InvoiceStatus::Pending && env.ledger().timestamp() >= u64::from(invoice.due_date) {
        // Issues #92 + #93: optional oracle verification with data-freshness guard.
        // When require_oracle_verification is true, the oracle stored in config is
        // called. If no oracle is configured the flag is a no-op.
        if require_oracle_verification {
            if let Some(oracle_addr) =
                crate::storage::get_config(&env).and_then(|c| c.price_oracle)
            {
                let response: OracleVerificationResponse = env.invoke_contract(
                    &oracle_addr,
                    &Symbol::new(&env, "get_payer_data"),
                    vec![&env, invoice.payer.clone().into_val(&env)],
                );

                // Issue #93: reject stale oracle data.
                // Staleness = current_ledger_sequence - oracle.timestamp >= max_oracle_age_ledgers.
                // If max_oracle_age_ledgers == 0 the check is disabled (governance escape hatch).
                let max_age = crate::storage::get_config(&env)
                    .map(|c| c.max_oracle_age_ledgers)
                    .unwrap_or(DEFAULT_MAX_ORACLE_AGE_LEDGERS);
                if max_age > 0 {
                    let current_ledger = env.ledger().sequence() as u64;
                    let age = current_ledger.saturating_sub(response.timestamp as u64);
                    if age >= max_age {
                        return Err(ContractError::OracleDataStale);
                    }
                }

                // Issue #92: reject unverified payers.
                if !response.is_verified {
                    return Err(ContractError::PayerUnverified);
                }
            }
        }

        if invoice.status == InvoiceStatus::Pending
            && env.ledger().timestamp() > u64::from(invoice.due_date)
        {
            invoice.status = InvoiceStatus::Expired;
            save_invoice(&env, &invoice);
            env.events().publish_event(&InvoiceExpired {
                invoice_id: invoice.id,
                freelancer: invoice.freelancer.clone(),
                status: invoice.status.clone(),
            });
            return Err(ContractError::InvoiceExpired);
        }

        match invoice.status {
            InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {}
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Funded => return Err(ContractError::AlreadyFunded),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
        }

        if invoice.amount_funded + fund_amount > invoice.amount {
            return Err(ContractError::OverfundingRejected);
        }

        let token = token_client(&env, &invoice.token);
        let contract_address = env.current_contract_address();

        // Handle token precision if needed
        let normalized_fund_amount = if is_xlm_token(&env, &invoice.token) {
            normalize_xlm_amount(fund_amount)
        } else if is_eurc_token(&env, &invoice.token) {
            normalize_eurc_amount(fund_amount)
        } else {
            normalize_usdc_amount(fund_amount)
        };

        // --- Calculate the effective rate ---
        // For auction invoices, calculate the current auction rate
        let effective_rate = if invoice.is_auction {
            let current_time = env.ledger().timestamp();
            let auction_started_at = invoice.auction_started_at.unwrap_or(0) as u64;
            let start_rate = invoice.auction_start_rate.unwrap_or(0);
            let min_rate = invoice.auction_min_rate.unwrap_or(0);
            let decay_per_hour = invoice.auction_rate_decay_per_hour.unwrap_or(0);

            calculate_auction_rate(current_time, auction_started_at, start_rate, min_rate, decay_per_hour)
        } else {
            invoice.discount_rate
        };

        let fund_discount = normalized_fund_amount
            .checked_mul(discount_rate_as_i128(invoice.discount_rate))
            .unwrap_or(0) / 10_000;
            .checked_mul(discount_rate_as_i128(effective_rate))
            .unwrap_or(0)
            / 10_000;
        let cost = normalized_fund_amount - fund_discount;

        token.transfer(&funder, &contract_address, &cost);

        let mut funders = get_invoice_funders(&env, invoice_id);
        let mut found = false;
        for i in 0..funders.len() {
            let (addr, amt) = funders.get(i).unwrap();
            if addr == funder {
                funders.set(i, (addr, amt + fund_amount));
                found = true;
                break;
            }
        }
        if !found {
            funders.push_back((funder.clone(), fund_amount));
        }
        save_invoice_funders(&env, invoice_id, &funders);

        invoice.amount_funded += fund_amount;

        if invoice.amount_funded == invoice.amount {
            let discount_amount = invoice.amount.checked_mul(discount_rate_as_i128(invoice.discount_rate)).unwrap_or(0) / 10_000;
            // Fully funded — pay out to freelancer
            let discount_amount = invoice
                .amount
                .checked_mul(discount_rate_as_i128(effective_rate))
                .unwrap_or(0)
                / 10_000;
            let freelancer_payout = invoice.amount - discount_amount;
            token.transfer(&contract_address, &invoice.freelancer, &freelancer_payout);
            invoice.status = InvoiceStatus::Funded;
            invoice.funded_at = Some(env.ledger().timestamp().try_into().unwrap());
            invoice.funder = Some(funder.clone());
            set_lp_score(&env, &funder, get_lp_score(&env, &funder) + 1);
        } else {
            invoice.status = InvoiceStatus::PartiallyFunded;
        }

        save_invoice(&env, &invoice);
        add_invoice_to_lp(&env, &funder, invoice_id);

        // OPTIMIZATION: Batch stat updates
        let mut stats_delta = StatsDelta::default();

        // Issue #119: Transfer NFT to the LP when invoice is fully funded
        if invoice.status == InvoiceStatus::Funded {
            crate::nft::transfer_invoice_nft(&env, invoice_id, invoice.freelancer.clone(), funder.clone())?;
        }

        // Update LP index
        add_invoice_to_lp(&env, &funder, invoice_id);

        // ── Issue #116: Maintain LP portfolio stats ───────────────
        // We track a new position only on the first fund for this LP on this
        // invoice (guarded by the `!found` flag set above in the funders loop).
        // partial top-ups by the same LP are already merged into the funders
        // entry — adding a position again would double-count.
        {
            let mut lp_stats = storage_get_lp_portfolio_stats(&env, &funder);
            if !found {
                // New position: accumulate capital and update the running
                // average yield (simple mean of discount_rate_bps values).
                lp_stats.total_funded = lp_stats
                    .total_funded
                    .checked_add(fund_amount)
                    .unwrap_or(lp_stats.total_funded);
                let old_total = lp_stats.total_positions as u64;
                lp_stats.total_positions = lp_stats.total_positions.saturating_add(1);
                let new_total = lp_stats.total_positions as u64;
                // Weighted recalculation: avg = (old_avg * old_n + rate) / new_n
                lp_stats.avg_yield_bps = if new_total > 0 {
                    (((lp_stats.avg_yield_bps as u64) * old_total
                        + invoice.discount_rate as u64)
                        / new_total) as u32
                } else {
                    invoice.discount_rate
                };
            } else {
                // Top-up on an existing position — only grow total_funded.
                lp_stats.total_funded = lp_stats
                    .total_funded
                    .checked_add(fund_amount)
                    .unwrap_or(lp_stats.total_funded);
            }
            // A position becomes "active" when the invoice is fully Funded.
            if invoice.status == InvoiceStatus::Funded {
                lp_stats.active_positions = lp_stats.active_positions.saturating_add(1);
            }
            save_lp_portfolio_stats(&env, &funder, &lp_stats);
        }

        // Increment total funded counter if fully funded
        if invoice.status == InvoiceStatus::Funded {
            stats_delta.total_funded = 1;
        }

        stats_delta.add_volume(&env, &invoice.token, fund_amount);
        stats_delta.apply(&env);

        notify_distribution_funding(&env, &funder, fund_amount);

        let now = env.ledger().timestamp();
        let days_to_due = if u64::from(invoice.due_date) > now { (u64::from(invoice.due_date) - now) / (24*60*60) } else { 0 };
        let effective_yield_bps = ((invoice.discount_rate as u64 * days_to_due) / 365) as u32;

        env.events().publish_event(&InvoiceFunded {
            invoice_id,
            funder: funder.clone(),
            freelancer: invoice.freelancer,
            payer: invoice.payer,
            token: invoice.token,
            fund_amount,
            amount_funded: invoice.amount_funded,
            invoice_amount: invoice.amount,
            due_date: u64::from(invoice.due_date),
            discount_rate: invoice.discount_rate,
            funded_at: invoice.funded_at.map(|ts| ts.into()),
            status: invoice.status,
            lp: funder,
            effective_yield_bps,
            timestamp: now,
        });

        let seconds_to_due = if u64::from(invoice.due_date) > now {
            u64::from(invoice.due_date) - now
        } else {
            0
        };

        let days_to_due = seconds_to_due / (24 * 60 * 60);

        let effective_yield_bps = ((effective_rate as u64 * days_to_due) / 365) as u32;

        // --- Emit appropriate event ---
        if invoice.is_auction {
            let hours_elapsed = if let Some(started_at) = invoice.auction_started_at {
                ((now as u32 - started_at) / 3600) as u32
            } else {
                0
            };

            env.events().publish_event(&AuctionFunded {
                invoice_id: invoice.id,
                funder: funder.clone(),
                freelancer: invoice.freelancer.clone(),
                payer: invoice.payer.clone(),
                token: invoice.token.clone(),
                fund_amount,
                effective_rate,
                hours_elapsed,
                funded_at: now,
            });
        } else {
            env.events().publish_event(&InvoiceFunded {
                invoice_id: invoice.id,
                funder: funder.clone(),
                freelancer: invoice.freelancer.clone(),
                payer: invoice.payer.clone(),
                token: invoice.token.clone(),
                fund_amount,
                amount_funded: invoice.amount_funded,
                invoice_amount: invoice.amount,
                due_date: u64::from(invoice.due_date),
                discount_rate: invoice.discount_rate,
                funded_at: invoice.funded_at.map(|ts| ts.into()),
                status: invoice.status.clone(),

                // NEW
                lp: funder.clone(),
                effective_yield_bps,
                timestamp: now,
            });
        }

            Ok(())
        })
    }

    pub fn expire_invoice(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);

        if env.ledger().timestamp() < u64::from(invoice.due_date) {
            return Err(ContractError::NotYetDefaulted);
        }

        match invoice.status {
            InvoiceStatus::Pending => {
                invoice.status = InvoiceStatus::Expired;
                save_invoice(&env, &invoice);
                env.events().publish_event(&InvoiceExpired {
                    invoice_id,
                    freelancer: invoice.freelancer,
                    status: invoice.status,
                });
                Ok(())
            }
            _ => Err(ContractError::Unauthorized),
        }
    }

    pub fn mark_paid(env: Env, invoice_id: u64, amount: i128) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        if amount <= 0 { return Err(ContractError::InvalidAmount); }

        let mut invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;
        require_payer_by_id(&env, invoice_id)?;

        match invoice.status {
            InvoiceStatus::Funded => {}
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
            InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => return Err(ContractError::NotFunded),
        }

        let remaining = invoice.amount - invoice.amount_paid;
        if amount > remaining { return Err(ContractError::OverpaymentRejected); }

        let funders = get_invoice_funders(&env, invoice_id);
        if funders.is_empty() { return Err(ContractError::NotFunded); }

        let token = token_client(&env, &invoice.token);
        let contract_address = env.current_contract_address();

        let normalized_amount = if is_xlm_token(&env, &invoice.token) { normalize_xlm_amount(amount) } else { normalize_usdc_amount(amount) };
        // Handle token precision if needed
        let normalized_amount = if is_xlm_token(&env, &invoice.token) {
            normalize_xlm_amount(amount)
        } else if is_eurc_token(&env, &invoice.token) {
            normalize_eurc_amount(amount)
        } else {
            normalize_usdc_amount(amount)
        };

        // Payer sends partial/full amount to the contract
        token.transfer(&invoice.payer, &contract_address, &normalized_amount);

        invoice.amount_paid += amount;
    // ------------------------------------------------------------
    // transfer_lp_position
    /// Access: Current LP only
    pub fn transfer_lp_position(
        env: Env,
        invoice_id: u64,
        new_lp: Address,
    ) -> Result<(), ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }

        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);
        match invoice.status {
            InvoiceStatus::Funded => {}
            InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {
                return Err(ContractError::NotFunded)
            }
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
        }

        let current_lp = invoice
            .funder
            .clone()
            .ok_or(ContractError::Unauthorized)?;

        current_lp.require_auth();

        if current_lp == new_lp {
            return Err(ContractError::Unauthorized);
        }

        let mut funders = get_invoice_funders(&env, invoice_id);
        for i in 0..funders.len() {
            let (addr, amt) = funders.get(i).unwrap();
            if addr == current_lp {
                funders.set(i, (new_lp.clone(), amt));
            }
        }
        save_invoice_funders(&env, invoice_id, &funders);

        invoice.funder = Some(new_lp.clone());
        save_invoice(&env, &invoice);

        // Issue #119: Transfer NFT to the new LP when LP position is transferred
        // The NFT represents the LP's claim on the invoice
        crate::nft::transfer_invoice_nft(&env, invoice_id, current_lp.clone(), new_lp.clone())?;

        remove_invoice_from_lp(&env, &current_lp, invoice_id);
        add_invoice_to_lp(&env, &new_lp, invoice_id);

        env.events().publish_event(&LPPositionTransferred {
            invoice_id,
            old_lp: current_lp,
            new_lp,
            status: invoice.status.clone(),
        });

        Ok(())
    }

    // ------------------------------------------------------------
    // cancel_invoice
    // ------------------------------------------------------------
    /// Access: Submitter only
    pub fn cancel_invoice(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        if is_paused(&env) {
            return Err(ContractError::ContractPaused);
        }

        if invoice.amount_paid < invoice.amount {
            save_invoice(&env, &invoice);
            env.events().publish_event(&InvoicePartiallyPaid {
                invoice_id,
                payer: invoice.payer,
                amount_paid_now: amount,
                total_amount_paid: invoice.amount_paid,
                remaining_amount: invoice.amount - invoice.amount_paid,
            });
            return Ok(());
        }

        let fee_rate: u32 = env.storage().instance().get(&StorageKey::FeeRate).unwrap_or(0);
        let protocol_fee = invoice.amount.checked_mul(fee_rate as i128).unwrap_or(0) / 10_000;
        if protocol_fee > 0 {
            let admin: Address = env.storage().instance().get(&StorageKey::Admin).unwrap();
            token.transfer(&contract_address, &admin, &protocol_fee);
        }

        let distribute_amount = invoice.amount - protocol_fee;
        
        let primary_lp = funders.get(0).unwrap().0.clone();
        let primary_lp_funded = funders.get(0).unwrap().1;
        let primary_lp_payout = distribute_amount.checked_mul(primary_lp_funded).unwrap_or(0) / invoice.amount;
        let lp_earned = primary_lp_payout - primary_lp_funded;

        for i in 0..funders.len() {
            let (addr, amt) = funders.get(i).unwrap();
            let share = distribute_amount.checked_mul(amt).unwrap_or(0) / invoice.amount;
            if share > 0 { token.transfer(&contract_address, &addr, &share); }
        }

        invoice.status = InvoiceStatus::Paid;
        match invoice.status {
            InvoiceStatus::Pending => {}
            InvoiceStatus::PartiallyFunded => {
                let funders = get_invoice_funders(&env, invoice_id);
                let token = token_client(&env, &invoice.token);
                let contract_address = env.current_contract_address();
                for i in 0..funders.len() {
                    let (funder_addr, fund_amt) = funders.get(i).unwrap();
                    let fund_discount = fund_amt
                        .checked_mul(discount_rate_as_i128(invoice.discount_rate))
                        .unwrap_or(0)
                        / 10_000;
                    let refund = fund_amt - fund_discount;
                    token.transfer(&contract_address, &funder_addr, &refund);
                }
            }
            InvoiceStatus::Funded => return Err(ContractError::AlreadyFunded),
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
        }

        invoice.status = InvoiceStatus::Cancelled;

        save_invoice(&env, &invoice);

        env.events().publish_event(&InvoiceCancelled {
            invoice_id,
            freelancer: invoice.freelancer.clone(),
            status: invoice.status.clone(),
        });

        Ok(())
    }

    // ------------------------------------------------------------
    // expire_invoice
    // ------------------------------------------------------------
    /// Access: Anyone
    pub fn expire_invoice(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        if !invoice_exists(&env, invoice_id) {
            return Err(ContractError::InvoiceNotFound);
        }

        let mut invoice = load_invoice(&env, invoice_id);

        if env.ledger().timestamp() <= u64::from(invoice.due_date) {
            return Err(ContractError::NotYetDefaulted);
        }

        match invoice.status {
            InvoiceStatus::Pending => {
                invoice.status = InvoiceStatus::Expired;
                save_invoice(&env, &invoice);
                env.events().publish_event(&InvoiceExpired {
                    invoice_id: invoice.id,
                    freelancer: invoice.freelancer.clone(),
                    status: invoice.status.clone(),
                });
                Ok(())
            }
            InvoiceStatus::PartiallyFunded | InvoiceStatus::Funded => {
                Err(ContractError::AlreadyFunded)
            }
            InvoiceStatus::Paid => Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => Err(ContractError::InvoiceDefaulted),
            InvoiceStatus::Appealed => Err(ContractError::InvoiceAppealed),
            InvoiceStatus::Disputed => Err(ContractError::InvoiceDisputed),
            InvoiceStatus::Expired => Err(ContractError::InvoiceExpired),
            InvoiceStatus::Cancelled => Err(ContractError::AlreadyCancelled),
        }
    }

    // ────────────────────────────────────────────────────────────
    // mark_paid (USES invoice.token) — reentrancy-guarded
    // ────────────────────────────────────────────────────────────
    /// Access: Payer only
    pub fn mark_paid(env: Env, invoice_id: u64, amount: i128) -> Result<(), ContractError> {
        with_reentrancy_guard(&env, || {
            if is_paused(&env) {
                return Err(ContractError::ContractPaused);
            }

            if amount <= 0 {
                return Err(ContractError::InvalidAmount);
            }

            // Issue #71: single load instead of `invoice_exists` + `load_invoice`.
            let mut invoice =
                try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;

            require_payer_by_id(&env, invoice_id)?;

            match invoice.status {
                InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {
                    return Err(ContractError::NotFunded)
                }
                InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
                InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
                InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
                InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
                InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
                InvoiceStatus::Funded => {}
                InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
            }

            let remaining = invoice.amount - invoice.amount_paid;
            if amount > remaining {
                return Err(ContractError::OverpaymentRejected);
            }

            let funders = get_invoice_funders(&env, invoice_id);
            if funders.len() == 0 {
                return Err(ContractError::NotFunded);
            }
        let funders = get_invoice_funders(&env, invoice_id);
        if funders.is_empty() {
            return Err(ContractError::NotFunded);
        }

            let token = token_client(&env, &invoice.token);
            let contract_address = env.current_contract_address();

            // Handle XLM precision if needed (SAC wrapper handles conversion internally)
            let normalized_amount = if is_xlm_token(&env, &invoice.token) {
                normalize_xlm_amount(amount)
            } else {
                normalize_usdc_amount(amount)
            };
        // Handle token precision if needed
        let normalized_amount = if is_xlm_token(&env, &invoice.token) {
            normalize_xlm_amount(amount)
        } else if is_eurc_token(&env, &invoice.token) {
            normalize_eurc_amount(amount)
        } else {
            normalize_usdc_amount(amount)
        };

        // Payer sends partial/full amount to the contract
        token.transfer(&invoice.payer, &contract_address, &normalized_amount);

            // Payer sends partial/full amount to the contract
            token.transfer(&invoice.payer, &contract_address, &normalized_amount);

            invoice.amount_paid += amount;

            // If not fully paid, save and emit partial event
            if invoice.amount_paid < invoice.amount {
                save_invoice(&env, &invoice);
                env.events().publish_event(&InvoicePartiallyPaid {
                    invoice_id: invoice.id,
                    payer: invoice.payer.clone(),
                    amount_paid_now: amount,
                    total_amount_paid: invoice.amount_paid,
                    remaining_amount: invoice.amount - invoice.amount_paid,
                });
                return Ok(());
            }

            // --- FULL PAYMENT LOGIC ---
            // Calculate protocol fee and deduct it
            let fee_rate: u32 = env
                .storage()
                .instance()
                .get(&crate::storage::DataKey::FeeRate)
                .unwrap_or(0);
            let protocol_fee = invoice.amount.checked_mul(fee_rate as i128).unwrap_or(0) / 10_000;

            if protocol_fee > 0 {
                let admin: Address = env
                    .storage()
                    .instance()
                    .get(&crate::storage::DataKey::Admin)
                    .unwrap();
                token.transfer(&contract_address, &admin, &protocol_fee);
            }

            let distribute_amount = invoice.amount - protocol_fee;

            // Legacy compatibility: use first LP for event emission
            let primary_lp = funders.get(0).unwrap().0.clone();

            // Total amount funded by primary LP
            let primary_lp_funded = funders.get(0).unwrap().1;

            // LP payout after settlement distribution
            let primary_lp_payout = distribute_amount
                .checked_mul(primary_lp_funded)
                .unwrap_or(0)
                / invoice.amount;

            // LP earnings
            let lp_earned = primary_lp_payout - primary_lp_funded;

            // Distribute proportionally to funders
            for i in 0..funders.len() {
                let (funder_addr, fund_amt) = funders.get(i).unwrap();
                let funder_share =
                    distribute_amount.checked_mul(fund_amt).unwrap_or(0) / invoice.amount;
                if funder_share > 0 {
                    token.transfer(&contract_address, &funder_addr, &funder_share);
                }
        let protocol_fee_bps = env
            .storage()
            .instance()
            .get(&crate::storage::DataKey::ProtocolFeeBps)
            .unwrap_or(0_u32);

        let treasury = env
            .storage()
            .instance()
            .get::<_, Address>(&crate::storage::DataKey::TreasuryAddress);

        // LP payout after settlement distribution
        let mut primary_lp_payout = distribute_amount
            .checked_mul(primary_lp_funded)
            .unwrap_or(0)
            / invoice.amount;

        // LP earnings
        let mut lp_earned = primary_lp_payout - primary_lp_funded;

        if lp_earned > 0 && protocol_fee_bps > 0 && treasury.is_some() {
            let fee = lp_earned.checked_mul(protocol_fee_bps as i128).unwrap_or(0) / 10_000;
            primary_lp_payout -= fee;
            lp_earned -= fee;
        }

        // Distribute proportionally to funders
        for i in 0..funders.len() {
            let (funder_addr, fund_amt) = funders.get(i).unwrap();
            let mut funder_share =
                distribute_amount.checked_mul(fund_amt).unwrap_or(0) / invoice.amount;
            
            let earned = funder_share.saturating_sub(fund_amt);
            if earned > 0 && protocol_fee_bps > 0 {
                if let Some(treasury_addr) = treasury.clone() {
                    let fee = earned.checked_mul(protocol_fee_bps as i128).unwrap_or(0) / 10_000;
                    if fee > 0 {
                        funder_share -= fee;
                        token.transfer(&contract_address, &treasury_addr, &fee);
                        
                        env.events().publish_event(&crate::events::FeesCollected {
                            invoice_id,
                            fee_amount: fee,
                            treasury: treasury_addr,
                        });
                    }
                }
            }

            if funder_share > 0 {
                token.transfer(&contract_address, &funder_addr, &funder_share);
            }

            // ---- Update invoice ----
            invoice.status = InvoiceStatus::Paid;

            save_invoice(&env, &invoice);
        // ── Issue #116: Update each LP's portfolio stats on settlement ────
        for i in 0..funders.len() {
            let (funder_addr, fund_amt) = funders.get(i).unwrap();
            let funder_share =
                distribute_amount.checked_mul(fund_amt).unwrap_or(0) / invoice.amount;
            let mut earned = funder_share.saturating_sub(fund_amt);
            
            if earned > 0 && protocol_fee_bps > 0 && treasury.is_some() {
                let fee = earned.checked_mul(protocol_fee_bps as i128).unwrap_or(0) / 10_000;
                earned -= fee;
            }

            let mut lp_stats = storage_get_lp_portfolio_stats(&env, &funder_addr);
            lp_stats.total_earned = lp_stats
                .total_earned
                .checked_add(earned)
                .unwrap_or(lp_stats.total_earned);
            lp_stats.active_positions = lp_stats.active_positions.saturating_sub(1);
            save_lp_portfolio_stats(&env, &funder_addr, &lp_stats);
        }

        // ---- Update invoice ----
        invoice.status = InvoiceStatus::Paid;

        // Issue #119: Burn the NFT when invoice is marked as paid
        // Get the current NFT owner (should be the LP who funded it)
        if let Some(nft_owner) = crate::nft::get_invoice_nft_owner(&env, invoice_id) {
            crate::nft::burn_invoice_nft(&env, invoice_id, nft_owner)?;
        }

        save_invoice(&env, &invoice);

        // OPTIMIZATION: Batch stat update
        let mut stats_delta = StatsDelta::default();
        stats_delta.total_paid = 1;
        stats_delta.apply(&env);

        let paid_on_time = env.ledger().timestamp() <= u64::from(invoice.due_date);
        notify_distribution_settlement(&env, &invoice.freelancer, &invoice.payer, paid_on_time);
        
        // Update payer reputation
        let current_score = get_payer_score(&env, &invoice.payer);
        set_payer_score(&env, &invoice.payer, current_score + 1);
            // Increment total paid counter
            increment_total_paid(&env);

            let paid_on_time = env.ledger().timestamp() <= u64::from(invoice.due_date);
            notify_distribution_settlement(&env, &invoice.freelancer, &invoice.payer, paid_on_time);

            // --- Update payer reputation ---
            let current_score = get_payer_score(&env, &invoice.payer);
            set_payer_score(&env, &invoice.payer, current_score + 1);

            env.events().publish_event(&InvoicePaid {
                invoice_id: invoice.id,
                payer: invoice.payer.clone(),
                lp: primary_lp,
                freelancer: invoice.freelancer.clone(),
                token: invoice.token.clone(),
                amount_paid: invoice.amount,
                lp_earned,
                lp_payout: primary_lp_payout,
                settlement_timestamp: env.ledger().timestamp(),
                paid_on_time,
                status: invoice.status.clone(),
            });
        // Increment detailed reputation invoices_paid count for both payer and freelancer
        increment_invoices_paid(&env, &invoice.payer);
        increment_invoices_paid(&env, &invoice.freelancer);

        env.events().publish_event(&InvoicePaid {
            invoice_id,
            payer: invoice.payer,
            lp: primary_lp,
            freelancer: invoice.freelancer,
            token: invoice.token,
            amount_paid: invoice.amount,
            lp_earned,
            lp_payout: primary_lp_payout,
            settlement_timestamp: env.ledger().timestamp(),
            paid_on_time,
            status: invoice.status,
        });
        Ok(())

            Ok(())
        })
    }

    pub fn claim_yield(env: Env, invoice_id: u64) -> Result<i128, ContractError> {
        let invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;
        if let Some(ref funder) = invoice.funder {
            require_lp_by_id(&env, funder, invoice_id)?;
        } else {
            return Err(ContractError::NothingToClaim);
        }

        match invoice.status {
            InvoiceStatus::Paid => {
                let yield_amount = invoice.amount.checked_mul(discount_rate_as_i128(invoice.discount_rate)).unwrap_or(0) / 10_000;
                Ok(yield_amount)
            }
            _ => Ok(0),
        }
    }

    pub fn claim_default(env: Env, funder: Address, invoice_id: u64) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        require_lp(&env, &funder)?;
        
        let mut invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;

        let funders = get_invoice_funders(&env, invoice_id);
        if !funders.iter().any(|f| f.0 == funder) {
            return Err(ContractError::Unauthorized);
        }

        if env.ledger().timestamp() < u64::from(invoice.due_date) {
            return Err(ContractError::NotYetDefaulted);
        }

        match invoice.status {
            InvoiceStatus::Funded => {}
            InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
            InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
            _ => return Err(ContractError::NotFunded),
        }

        let token = token_client(&env, &invoice.token);
        let contract_address = env.current_contract_address();
        let mut total_refunded = 0;
        for i in 0..funders.len() {
            let (addr, amt) = funders.get(i).unwrap();
            let refund = amt - (amt * invoice.discount_rate as i128 / 10_000);
            
            // NOTE: This will fail if contract doesn't have balance. 
            // In a real scenario, this might be handled by a pool or insurance.
            token.transfer(&contract_address, &addr, &refund);
            total_refunded += refund;
        }
    // ----------------------------------------------------------------
    // claim_default
    // ----------------------------------------------------------------
    /// Access: LP only
    /// **Reentrancy Protected:** Yes - This function performs token transfers
    pub fn claim_default(env: Env, funder: Address, invoice_id: u64) -> Result<(), ContractError> {
        with_reentrancy_guard(&env, || {
            if is_paused(&env) {
                return Err(ContractError::ContractPaused);
            }

            require_lp(&env, &funder)?;

            if !invoice_exists(&env, invoice_id) {
                return Err(ContractError::InvoiceNotFound);
            }

            let mut invoice = load_invoice(&env, invoice_id);

            let funders = get_invoice_funders(&env, invoice_id);
            let mut is_funder = false;
            for i in 0..funders.len() {
                if funders.get(i).unwrap().0 == funder {
                    is_funder = true;
                    break;
                }
            }

            if !is_funder {
                return Err(ContractError::Unauthorized);
            }

            let now = env.ledger().timestamp();
            if now < u64::from(invoice.due_date) {
                return Err(ContractError::NotYetDefaulted);
            }

            match invoice.status {
                InvoiceStatus::Funded => {}
                InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {
                    return Err(ContractError::NotFunded)
                }
                InvoiceStatus::Paid => return Err(ContractError::AlreadyPaid),
                InvoiceStatus::Defaulted => return Err(ContractError::InvoiceDefaulted),
                InvoiceStatus::Appealed => return Err(ContractError::InvoiceAppealed),
                InvoiceStatus::Disputed => return Err(ContractError::InvoiceDisputed),
                InvoiceStatus::Expired => return Err(ContractError::InvoiceExpired),
                InvoiceStatus::Cancelled => return Err(ContractError::AlreadyCancelled),
            }

            let token = token_client(&env, &invoice.token);
            let contract_address = env.current_contract_address();

            let mut total_refunded = 0;

            for i in 0..funders.len() {
                let (funder_addr, fund_amt) = funders.get(i).unwrap();
                let fund_discount = fund_amt
                    .checked_mul(discount_rate_as_i128(invoice.discount_rate))
                    .unwrap_or(0)
                    / 10_000;
                let refund = fund_amt - fund_discount;
                token.transfer(&contract_address, &funder_addr, &refund);
                total_refunded += refund;
            }

            invoice.status = InvoiceStatus::Defaulted;
            save_invoice(&env, &invoice);

        let current_score = get_payer_score(&env, &invoice.payer);
        save_pre_default_payer_score(&env, invoice_id, current_score);
        set_payer_score(&env, &invoice.payer, current_score.saturating_sub(5));
        
            // --- Update payer reputation ---
            // Snapshot the score BEFORE applying the penalty so appeal_default()
            // can restore it exactly if the appeal is upheld.
            let current_score = get_payer_score(&env, &invoice.payer);
            save_pre_default_payer_score(&env, invoice_id, current_score);

            if current_score > 5 {
                set_payer_score(&env, &invoice.payer, current_score - 5);
            } else {
                set_payer_score(&env, &invoice.payer, 0);
            }

            env.events().publish_event(&InvoiceDefaulted {
                invoice_id: invoice.id,
                funder,
                freelancer: invoice.freelancer.clone(),
                payer: invoice.payer.clone(),
                token: invoice.token.clone(),
                amount: invoice.amount,
                due_date: u64::from(invoice.due_date),
                defaulted_at: now,
                discount_amount: total_refunded,
                status: invoice.status.clone(),
            });
        // Increment detailed reputation invoices_defaulted count for the payer
        increment_invoices_defaulted(&env, &invoice.payer);

        env.events().publish_event(&InvoiceDefaulted {
            invoice_id,
            funder,
            freelancer: invoice.freelancer,
            payer: invoice.payer,
            token: invoice.token,
            amount: invoice.amount,
            due_date: u64::from(invoice.due_date),
            defaulted_at: env.ledger().timestamp(),
            discount_amount: total_refunded,
            status: invoice.status,
        });
        Ok(())

            Ok(())
        })
    }

    pub fn appeal_default(env: Env, invoice_id: u64, evidence_hash: BytesN<32>) -> Result<(), ContractError> {
        let mut invoice = load_invoice(&env, invoice_id);
        require_payer_by_id(&env, invoice_id)?;
        if get_appeal(&env, invoice_id).is_some() { return Err(ContractError::AlreadyAppealed); }
        if invoice.status != InvoiceStatus::Defaulted { return Err(ContractError::NotDefaulted); }

        let now = env.ledger().timestamp();
        if now > u64::from(invoice.due_date) + APPEAL_WINDOW_SECONDS {
            return Err(ContractError::AppealWindowClosed);
        }

        save_appeal(&env, invoice_id, &AppealRecord {
            evidence_hash: evidence_hash.clone(),
            appealed_at: now.try_into().unwrap(),
            pre_default_score: get_pre_default_payer_score(&env, invoice_id).unwrap_or(50),
        });

        invoice.status = InvoiceStatus::Appealed;
        save_invoice(&env, &invoice);

        env.events().publish_event(&DefaultAppealed {
            invoice_id,
            payer: invoice.payer,
            evidence_hash,
            appealed_at: now,
        });
        Ok(())
    }

    pub fn resolve_appeal(env: Env, invoice_id: u64, upheld: bool) -> Result<(), ContractError> {
        require_admin(&env)?;
        let mut invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;
        if invoice.status != InvoiceStatus::Appealed { return Err(ContractError::NotDefaulted); }
        let appeal = get_appeal(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;

        if upheld {
            set_payer_score(&env, &invoice.payer, appeal.pre_default_score);
            
            // Decrement invoices_defaulted count
            let mut profile = get_reputation(&env, &invoice.payer);
            profile.invoices_defaulted = profile.invoices_defaulted.saturating_sub(1);
            set_reputation(&env, &profile);
        }
        invoice.status = InvoiceStatus::Defaulted;
        save_invoice(&env, &invoice);

        env.events().publish_event(&AppealResolved {
            invoice_id,
            payer: invoice.payer,
            upheld,
            resolved_at: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn dispute_invoice(env: Env, invoice_id: u64, reason_hash: BytesN<32>) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        let mut invoice = load_invoice(&env, invoice_id);
        require_payer_by_id(&env, invoice_id)?;
        if get_dispute(&env, invoice_id).is_some() { return Err(ContractError::AlreadyDisputed); }

        match invoice.status {
            InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded | InvoiceStatus::Funded => {}
            _ => return Err(ContractError::Unauthorized),
        }

        save_dispute(&env, invoice_id, &DisputeRecord {
            reason_hash: reason_hash.clone(),
            disputed_at: env.ledger().sequence(),
        });

        invoice.status = InvoiceStatus::Disputed;
        save_invoice(&env, &invoice);

        env.events().publish_event(&InvoiceDisputed {
            invoice_id,
            payer: invoice.payer,
            reason_hash,
            disputed_at: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn resolve_dispute(env: Env, invoice_id: u64, resolution_hash: BytesN<32>, resolution: u32) -> Result<(), ContractError> {
        require_admin(&env)?;
        let mut invoice = load_invoice(&env, invoice_id);
        if invoice.status != InvoiceStatus::Disputed { return Err(ContractError::NotDisputed); }

        match resolution {
            1 => { // Upheld (Payer)
                let funders = get_invoice_funders(&env, invoice_id);
                let token = token_client(&env, &invoice.token);
                let contract_address = env.current_contract_address();
                for i in 0..funders.len() {
                    let (addr, amt) = funders.get(i).unwrap();
                    let refund = amt - (amt * invoice.discount_rate as i128 / 10_000);
                    token.transfer(&contract_address, &addr, &refund);
                }
                invoice.status = InvoiceStatus::Cancelled;
            }
            2 => { // Rejected (Freelancer)
                if invoice.amount_funded == invoice.amount { invoice.status = InvoiceStatus::Funded; }
                else if invoice.amount_funded > 0 { invoice.status = InvoiceStatus::PartiallyFunded; }
                else { invoice.status = InvoiceStatus::Pending; }
            }
            _ => return Err(ContractError::Unauthorized),
        }

        save_invoice(&env, &invoice);
        env.events().publish_event(&DisputeResolved {
            invoice_id,
            resolution_hash,
            resolution,
            resolved_at: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn auto_resolve_dispute(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        let mut invoice = load_invoice(&env, invoice_id);
        if invoice.status != InvoiceStatus::Disputed { return Err(ContractError::NotDisputed); }
        let dispute = get_dispute(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;
        let config = get_config(&env).ok_or(ContractError::Unauthorized)?;
        
        if (env.ledger().sequence() as u64) < (dispute.disputed_at as u64) + config.dispute_timeout_ledgers {
             return Err(ContractError::Unauthorized);
        }

        if invoice.amount_funded == invoice.amount { invoice.status = InvoiceStatus::Funded; }
        else if invoice.amount_funded > 0 { invoice.status = InvoiceStatus::PartiallyFunded; }
        else { invoice.status = InvoiceStatus::Pending; }

        save_invoice(&env, &invoice);
        
        env.events().publish_event(&DisputeResolved {
            invoice_id,
            resolution_hash: BytesN::from_array(&env, &[0u8; 32]),
            resolution: 2, // Rejected
            resolved_at: env.ledger().timestamp(),
        });
        Ok(())
    }

    pub fn payer_score(env: Env, payer: Address) -> u32 { get_payer_score(&env, &payer) }
    pub fn lp_score(env: Env, lp: Address) -> u32 { get_lp_score(&env, &lp) }
    pub fn get_reputation(env: Env, address: Address) -> ReputationProfile { invoice::get_reputation(&env, &address) }
    pub fn min_payer_reputation(env: Env) -> u32 { get_min_payer_reputation(&env) }
    // ================================================================
    // Contract Configuration
    // ================================================================

    #[allow(clippy::too_many_arguments)]
    pub fn update_config(
        env: Env,
        caller: Address,
        high_rep_threshold: u32,
        bonus_bps: u32,
        min_discount_rate_bps: u32,
        decay_rate_bps: u32,
        decay_period_ledgers: u64,
        dispute_timeout_ledgers: u64,
        xlm_sac_address: Address,
        usdc_sac_address: Address,
        eurc_sac_address: Address,
    ) -> Result<(), ContractError> {
        crate::config::update_config(
            &env,
            &caller,
            high_rep_threshold,
            bonus_bps,
            min_discount_rate_bps,
            decay_rate_bps,
            decay_period_ledgers,
            dispute_timeout_ledgers,
            xlm_sac_address,
            usdc_sac_address,
            eurc_sac_address,
        )
        .map_err(|_| ContractError::Unauthorized)
    }

    pub fn get_config(env: Env) -> Result<Config, ContractError> {
        crate::storage::get_config(&env).ok_or(ContractError::Unauthorized)
    }
    // payer_score
    // ----------------------------------------------------------------
    /// Access: Anyone
    pub fn payer_score(env: Env, payer: Address) -> u32 {
        get_payer_score(&env, &payer)
    }

    // ----------------------------------------------------------------
    // lp_score  (Issue #34)
    // ----------------------------------------------------------------
    /// Access: Anyone
    pub fn lp_score(env: Env, lp: Address) -> u32 {
        get_lp_score(&env, &lp)
    }

    // ----------------------------------------------------------------
    // get_top_payers (Issue #77)
    // ----------------------------------------------------------------
    /// Return up to `limit` payers with the highest reputation scores.
    /// Reads from the maintained top-payers heap — no full-list sort required.
    /// Access: Anyone
    pub fn get_top_payers(env: Env, limit: u32) -> Vec<TopPayerEntry> {
        crate::top_payers::get_top_payers(&env, limit)
    }

    // ----------------------------------------------------------------
    // get_reputation (Issue #26)
    // ----------------------------------------------------------------
    /// Read an address's detailed reputation profile. Unknown addresses return
    /// a zeroed profile rather than panicking.
    /// Access: Anyone
    pub fn get_reputation(env: Env, address: Address) -> ReputationProfile {
        get_reputation(&env, &address)
    }

    // ----------------------------------------------------------------
    // min_payer_reputation config (Issue #28)
    // ----------------------------------------------------------------
    /// Current minimum payer reputation required to fund an invoice (0 = off).
    /// Access: Anyone
    pub fn min_payer_reputation(env: Env) -> u32 {
        get_min_payer_reputation(&env)
    }

    /// Update the minimum payer reputation threshold.
    /// Access: Admin only
    pub fn set_min_payer_reputation(env: Env, value: u32) -> Result<(), ContractError> {
        require_admin(&env)?;
        let updated_by = get_admin(&env).ok_or(ContractError::Unauthorized)?;
        let old_value = get_min_payer_reputation(&env);
        set_min_payer_reputation(&env, value);
        env.events().publish_event(&ParameterUpdated {
            param_name: Symbol::new(&env, "min_payer_reputation"),
            old_value: old_value as i128,
            new_value: value as i128,
            updated_by,
        });
        Ok(())
    }

    pub fn get_invoice(env: Env, invoice_id: u64) -> Result<Invoice, ContractError> {
        try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)
    }

    pub fn get_invoice_count(env: Env) -> u64 { read_next_invoice_id(&env) - 1 }

    pub fn cancel_invoice(env: Env, invoice_id: u64) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        let mut invoice = load_invoice(&env, invoice_id);
        require_submitter_by_id(&env, &invoice.freelancer, invoice_id)?;
        
        match invoice.status {
            InvoiceStatus::Pending => {}
            InvoiceStatus::PartiallyFunded => {
                let funders = get_invoice_funders(&env, invoice_id);
                let token = token_client(&env, &invoice.token);
                let contract_address = env.current_contract_address();
                for i in 0..funders.len() {
                    let (funder_addr, fund_amt) = funders.get(i).unwrap();
                    let fund_discount = fund_amt.checked_mul(discount_rate_as_i128(invoice.discount_rate)).unwrap_or(0) / 10_000;
                    let refund = fund_amt - fund_discount;
                    token.transfer(&contract_address, &funder_addr, &refund);
                }
            }
            _ => return Err(ContractError::AlreadyFunded),
        }
        
        invoice.status = InvoiceStatus::Cancelled;
        save_invoice(&env, &invoice);
        
        env.events().publish_event(&InvoiceCancelled {
            invoice_id,
            freelancer: invoice.freelancer,
            status: invoice.status,
        });
        Ok(())
    }

    // ----------------------------------------------------------------
    // NFT Query Functions (Issue #119)
    // ----------------------------------------------------------------
    /// Get the metadata of an invoice NFT
    /// 
    /// Returns the NFT metadata including invoice details and current owner.
    /// 
    /// # Arguments
    /// * `invoice_id` - The invoice ID whose NFT metadata to retrieve
    /// 
    /// # Errors
    /// Returns `ContractError::InvoiceNftNotFound` if no NFT exists for this invoice.
    /// 
    /// Access: Anyone
    pub fn get_invoice_nft_metadata(env: Env, invoice_id: u64) -> Result<InvoiceNftMetadata, ContractError> {
        crate::nft::get_invoice_nft_metadata(&env, invoice_id)
            .ok_or(ContractError::InvoiceNftNotFound)
    }

    /// Get the current owner of an invoice NFT
    /// 
    /// Returns the address that currently owns the NFT for this invoice.
    /// 
    /// # Arguments
    /// * `invoice_id` - The invoice ID whose NFT owner to retrieve
    /// 
    /// # Returns
    /// Option containing the owner address if the NFT exists, None otherwise.
    /// 
    /// Access: Anyone
    pub fn get_invoice_nft_owner(env: Env, invoice_id: u64) -> Option<Address> {
        crate::nft::get_invoice_nft_owner(&env, invoice_id)
    }

    /// Check if an invoice NFT exists
    /// 
    /// # Arguments
    /// * `invoice_id` - The invoice ID to check
    /// 
    /// # Returns
    /// true if the NFT exists, false otherwise.
    /// 
    /// Access: Anyone
    pub fn invoice_nft_exists(env: Env, invoice_id: u64) -> bool {
        crate::nft::invoice_nft_exists(&env, invoice_id)
    }
}

    pub fn join_fund_queue(env: Env, lp: Address, invoice_id: u64) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        require_lp(&env, &lp)?;
        let invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;
        
        match invoice.status {
            InvoiceStatus::Pending | InvoiceStatus::PartiallyFunded => {}
            _ => return Err(ContractError::AlreadyFunded),
        }

        if get_queue_resolution(&env, invoice_id).is_some() {
            return Err(ContractError::NotApprovedFunder);
        }

        let mut queue = get_fund_queue(&env, invoice_id);
        for item in queue.iter() {
            if item.lp == lp { return Err(ContractError::AlreadyInQueue); }
        }

        let score = get_lp_score(&env, &lp);
        queue.push_back(LpFundRequest {
            lp: lp.clone(),
            score,
        });
        save_fund_queue(&env, invoice_id, &queue);
        
        env.events().publish_event(&FundRequested {
            invoice_id,
            lp,
            score,
        });
        Ok(())
    }

    pub fn resolve_fund_queue(env: Env, invoice_id: u64) -> Result<Address, ContractError> {
        let queue = get_fund_queue(&env, invoice_id);
        if queue.is_empty() { return Err(ContractError::NotFunded); }

        if let Some(approved) = get_queue_resolution(&env, invoice_id) {
            return Ok(approved);
        }

        let mut best_lp = queue.get(0).unwrap().lp.clone();
        let mut best_score = queue.get(0).unwrap().score;
/// Convert amount from XLM precision (7 decimals) to contract precision
fn normalize_xlm_amount(amount: i128) -> i128 {
    amount
}

/// Check if a token address is the USDC address
fn is_usdc_token(env: &Env, token: &Address) -> bool {
    if let Some(config) = crate::storage::get_config(env) {
        token == &config.usdc_sac_address
    } else {
        false
    }
}

/// Convert amount from USDC precision (6 decimals) to contract precision
fn normalize_usdc_amount(amount: i128) -> i128 {
    amount
}

/// Check if a token address is the EURC address
fn is_eurc_token(env: &Env, token: &Address) -> bool {
    if let Some(config) = crate::storage::get_config(env) {
        token == &config.eurc_sac_address
    } else {
        false
    }
}

/// Convert amount from EURC precision (6 decimals) to contract precision
fn normalize_eurc_amount(amount: i128) -> i128 {
    amount
}

fn validate_invoice_terms(
    env: &Env,
    amount: i128,
    due_date: u64,
    discount_rate: u32,
) -> Result<(), ContractError> {
    if amount < 1_000_000 {
        return Err(ContractError::InvalidAmount);
    }

        for i in 1..queue.len() {
            let item = queue.get(i).unwrap();
            if item.score > best_score {
                best_score = item.score;
                best_lp = item.lp.clone();
            }
        }

        save_queue_resolution(&env, invoice_id, &best_lp);
        
        env.events().publish_event(&FundQueueResolved {
            invoice_id,
            approved_lp: best_lp.clone(),
            score: best_score,
        });
        Ok(best_lp)
    }

    pub fn suggested_discount_rate(env: Env, payer: Address) -> u32 {
        let score = get_payer_score(&env, &payer);
        let capped = score.min(100);
        let rate = 500 + (100 - capped) * 5;
        rate.max(50)
    }

    pub fn get_top_payers(env: Env, limit: u32) -> Vec<TopPayerEntry> {
        crate::top_payers::get_top_payers(&env, limit)
    }

    pub fn transfer_invoice(env: Env, invoice_id: u64, new_freelancer: Address) -> Result<(), ContractError> {
        if is_paused(&env) { return Err(ContractError::ContractPaused); }
        let mut invoice = try_load_invoice(&env, invoice_id).ok_or(ContractError::InvoiceNotFound)?;
        require_submitter_by_id(&env, &invoice.freelancer, invoice_id)?;
        
        match invoice.status {
            InvoiceStatus::Pending => {}
            _ => return Err(ContractError::AlreadyFunded),
        }

        let old_freelancer = invoice.freelancer.clone();
        invoice.freelancer = new_freelancer.clone();
        save_invoice(&env, &invoice);
        
        remove_invoice_from_submitter(&env, &old_freelancer, invoice_id);
        add_invoice_to_submitter(&env, &new_freelancer, invoice_id);

        env.events().publish_event(&InvoiceTransferred {
            invoice_id,
            old_freelancer,
            new_freelancer,
            status: invoice.status,
        });
        Ok(())
    }
}

fn token_client<'a>(env: &'a Env, token: &Address) -> TokenClient<'a> { TokenClient::new(env, token) }
fn discount_rate_as_i128(rate: u32) -> i128 { rate as i128 }
fn is_xlm_token(env: &Env, token: &Address) -> bool {
    storage::get_config(env).map_or(false, |c| token == &c.xlm_sac_address)
}
fn normalize_xlm_amount(amount: i128) -> i128 { amount }
fn normalize_usdc_amount(amount: i128) -> i128 { amount }

fn validate_invoice_terms(env: &Env, amount: i128, due_date: u64, discount_rate: u32) -> Result<(), ContractError> {
    if amount < 1_000_000 { return Err(ContractError::InvalidAmount); }
    let max_rate: u32 = env.storage().instance().get(&StorageKey::MaxDiscountRate).unwrap_or(5000);
    if discount_rate == 0 || discount_rate > max_rate { return Err(ContractError::InvalidDiscountRate); }
    if due_date > u64::from(u32::MAX) { return Err(ContractError::InvalidDueDate); }
    let now = env.ledger().timestamp();
    if due_date <= now { return Err(ContractError::InvalidDueDate); }
    if due_date < now + MIN_INVOICE_DURATION { return Err(ContractError::DueDateTooSoon); }
    if due_date > now + MAX_INVOICE_DURATION { return Err(ContractError::DueDateTooFar); }
    Ok(())
}

fn is_approved_token(env: &Env, token: &Address) -> bool {
    env.storage().persistent().get(&StorageKey::ApprovedToken(token.clone())).unwrap_or(false)
    // First check the explicit allowlist in storage
    if env.storage()
        .persistent()
        .get(&crate::storage::DataKey::ApprovedToken(token.clone()))
        .unwrap_or(false) {
        return true;
    }

    // Then check the wired tokens in Config
    if let Some(config) = crate::storage::get_config(env) {
        if token == &config.usdc_sac_address || token == &config.eurc_sac_address || token == &config.xlm_sac_address {
            return true;
        }
    }

    false
}

fn notify_distribution_funding(env: &Env, lp: &Address, amount: i128) {
    if let Some(dist) = env.storage().instance().get::<_, Address>(&StorageKey::DistributionContract) {
        env.invoke_contract::<()>(&dist, &Symbol::new(env, "accrue_lp"), vec![env, lp.clone().into_val(env), amount.into_val(env)]);
    }
}

fn notify_distribution_settlement(env: &Env, freelancer: &Address, payer: &Address, on_time: bool) {
    if let Some(dist) = env.storage().instance().get::<_, Address>(&StorageKey::DistributionContract) {
        env.invoke_contract::<()>(&dist, &Symbol::new(env, "accrue_settlement"), vec![env, freelancer.clone().into_val(env), payer.clone().into_val(env), on_time.into_val(env)]);
    }
}

mod test;
#[cfg(test)]
mod tests_access_control;
mod tests_appeal;
mod tests_arithmetic;
mod tests_auth;
mod tests_dispute;
mod tests_distribution;
mod tests_invariants;
#[cfg(test)]
mod tests_invoice_paid_event;
#[cfg(test)]
mod tests_lp_funding_details_event;
mod tests_lp_priority_queue;
mod tests_mutation;
#[cfg(test)]
mod tests_partial_payment;
mod tests_protocol_fee;
mod tests_security;
mod tests_state_machine;
mod tests_storage;
mod tests_storage_extra;
#[cfg(test)]
mod tests_governance_features;
mod tests_benchmarks;
#[cfg(test)]
mod tests_top_payers;
#[cfg(test)]
mod tests_lazy_storage;
#[cfg(test)]
mod tests_reputation_events;
#[cfg(test)]
mod tests_oracle_verification;
#[cfg(test)]
mod tests_oracle_freshness;
#[cfg(test)]
mod tests_referral;
mod tests_discount_invariants;
#[cfg(test)]
mod tests_token_switch;
