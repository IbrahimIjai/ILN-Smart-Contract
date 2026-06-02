use soroban_sdk::{contracttype, Address, Env, BytesN};

use crate::config::Config;
use crate::invoice::{AppealRecord, Invoice, LpFundRequest, ReputationScore, ContractStats};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataKey {
    // Instance Storage
    Admin,
    Config,
    FeeRate,
    ProtocolFeeBps,
    TreasuryAddress,
    MaxDiscountRate,
    DistributionContract,
    Paused,
    /// Minimum payer reputation required to fund an invoice (Issue #28). Default 0.
    MinPayerReputation,
    /// auto-increment counter for IDs (moved to instance for optimization)
    /// Reentrancy guard lock flag
    ReentrancyLock,
    NextInvoiceId,
    /// ContractStats struct (Issue #optimization)
    Stats,
    /// Issue #124: Multi-sig admin configuration
    MultisigAdmin,
    /// Issue #124: Proposal counter for unique IDs
    MultisigProposalCounter,

    // Persistent Storage
    Invoice(u64),
    InvoiceCount, // Legacy, moved to NextInvoiceId in instance
    Token,
    PayerScore(Address),
    InvoiceFunders(u64),
    ApprovedToken(Address),
    TokenList,
    /// Detailed reputation profile per address (Issue #26).
    Reputation(Address),
    Appeal(u64),
    PreDefaultPayerScore(u64),
    LpScore(Address),
    FundQueue(u64),
    QueueResolution(u64),

    // Legacy Stats (Persistent)
    TotalInvoices,
    TotalFunded,
    TotalPaid,
    TotalVolumeUsdc,
    TotalVolumeEurc,
    TotalVolumeXlm,
    TokenVolume(Address),
    /// Referral counts keyed by fixed-size code
    ReferralCount(BytesN<32>),
    Dispute(u64),
    SubmitterInvoices(Address),
    LpInvoices(Address),
    /// Fixed-size min-heap of the top payers by reputation score (Issue #77).
    TopPayersHeap,
    /// Invoice NFT metadata storage (Issue #119)
    InvoiceNft(u64),
    /// Invoice NFT owner tracking (Issue #119)
    InvoiceNftOwner(u64),
    /// Issue #124: Multi-sig proposals by ID
    MultisigProposal(u64),
    /// Issue #116: Per-LP portfolio analytics snapshot
    LPPortfolioStats(Address),
    /// Issue #115: Count of invoices by state
    InvoiceStateCount(crate::invoice::InvoiceStatus),
}

// ----------------------------------------------------------------
// Config Helpers
// ----------------------------------------------------------------

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Admin)
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_config(env: &Env) -> Option<Config> {
    env.storage().instance().get(&DataKey::Config)
}

pub fn set_config(env: &Env, config: &Config) {
    env.storage().instance().set(&DataKey::Config, config);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn get_min_payer_reputation(env: &Env) -> u32 {
// ----------------------------------------------------------------
// Invoice Helpers
// ----------------------------------------------------------------

pub fn save_invoice(env: &Env, invoice: &Invoice) {
    let key = DataKey::Invoice(invoice.id);
    
    if let Some(old_invoice) = env.storage().persistent().get::<_, Invoice>(&key) {
        if old_invoice.status != invoice.status {
            decrement_state_count(env, &old_invoice.status);
            increment_state_count(env, &invoice.status);
        }
    } else {
        increment_state_count(env, &invoice.status);
    }
    
    env.storage().persistent().set(&key, invoice);
    env.storage()
        .instance()
        .get(&DataKey::MinPayerReputation)
        .unwrap_or(0)
}

pub fn set_min_payer_reputation(env: &Env, value: u32) {
    env.storage()
        .instance()
        .set(&DataKey::MinPayerReputation, &value);
}

// ----------------------------------------------------------------
// Invoice Helpers
// ----------------------------------------------------------------

pub fn read_next_invoice_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::NextInvoiceId)
        .unwrap_or(1)
}

pub fn write_next_invoice_id(env: &Env, id: u64) {
    env.storage().instance().set(&DataKey::NextInvoiceId, &id);
}

pub fn next_invoice_id(env: &Env) -> Result<u64, crate::errors::ContractError> {
    next_invoice_ids(env, 1)
}

pub fn next_invoice_ids(env: &Env, count: u32) -> Result<u64, crate::errors::ContractError> {
    let current_id = read_next_invoice_id(env);
    let next_id = current_id
        .checked_add(count as u64)
        .ok_or(crate::errors::ContractError::ArithmeticOverflow)?;

    write_next_invoice_id(env, next_id);

    Ok(current_id)
}

pub fn get_invoice_funders(env: &Env, id: u64) -> soroban_sdk::Vec<(Address, i128)> {
    env.storage()
        .persistent()
        .get(&DataKey::InvoiceFunders(id))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn save_invoice_funders(env: &Env, id: u64, funders: &soroban_sdk::Vec<(Address, i128)>) {
    env.storage()
        .persistent()
        .set(&DataKey::InvoiceFunders(id), funders);
}

pub fn get_fund_queue(env: &Env, invoice_id: u64) -> soroban_sdk::Vec<LpFundRequest> {
    env.storage()
        .persistent()
        .get(&DataKey::FundQueue(invoice_id))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn save_fund_queue(env: &Env, invoice_id: u64, queue: &soroban_sdk::Vec<LpFundRequest>) {
    env.storage()
        .persistent()
        .set(&DataKey::FundQueue(invoice_id), queue);
}

pub fn get_queue_resolution(env: &Env, invoice_id: u64) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::QueueResolution(invoice_id))
}

pub fn save_queue_resolution(env: &Env, invoice_id: u64, approved_lp: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::QueueResolution(invoice_id), approved_lp);
}

pub fn get_appeal(env: &Env, invoice_id: u64) -> Option<AppealRecord> {
    env.storage().persistent().get(&DataKey::Appeal(invoice_id))
}

pub fn save_appeal(env: &Env, invoice_id: u64, record: &AppealRecord) {
    env.storage()
        .persistent()
        .set(&DataKey::Appeal(invoice_id), record);
}

pub fn save_pre_default_payer_score(env: &Env, invoice_id: u64, score: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::PreDefaultPayerScore(invoice_id), &score);
}

pub fn get_pre_default_payer_score(env: &Env, invoice_id: u64) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::PreDefaultPayerScore(invoice_id))
}

pub fn get_contract_stats(env: &Env) -> ContractStats {
    env.storage()
        .instance()
        .get(&DataKey::Stats)
        .unwrap_or_else(|| ContractStats {
            total_invoices: 0,
            total_funded: 0,
            total_paid: 0,
            total_volume_usdc: 0,
            total_volume_eurc: 0,
            total_volume_xlm: 0,
            token_volumes: soroban_sdk::Vec::new(env),
            total_volume_usd_normalized: 0,
        })
}

pub fn save_contract_stats(env: &Env, stats: &ContractStats) {
    env.storage().instance().set(&DataKey::Stats, stats);
pub fn increment_total_paid(env: &Env) {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::TotalPaid)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::TotalPaid, &(current + 1));
}

pub fn get_state_count(env: &Env, state: &crate::invoice::InvoiceStatus) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::InvoiceStateCount(state.clone()))
        .unwrap_or(0)
}

pub fn increment_state_count(env: &Env, state: &crate::invoice::InvoiceStatus) {
    let current = get_state_count(env, state);
    env.storage()
        .persistent()
        .set(&DataKey::InvoiceStateCount(state.clone()), &(current + 1));
}

pub fn decrement_state_count(env: &Env, state: &crate::invoice::InvoiceStatus) {
    let current = get_state_count(env, state);
    if current > 0 {
        let new_val = current - 1;
        let key = DataKey::InvoiceStateCount(state.clone());
        if new_val == 0 {
            if env.storage().persistent().has(&key) {
                env.storage().persistent().remove(&key);
            }
        } else {
            env.storage().persistent().set(&key, &new_val);
        }
    }
}

pub fn add_volume(
    env: &Env,
    token: &Address,
    amount: i128,
    usdc_addr: &Address,
    eurc_addr: &Address,
    xlm_addr: &Address,
) {
    if token == usdc_addr {
        let current: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalVolumeUsdc)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalVolumeUsdc, &(current + amount));
    } else if token == eurc_addr {
        let current: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalVolumeEurc)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalVolumeEurc, &(current + amount));
    } else if token == xlm_addr {
        let current: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalVolumeXlm)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalVolumeXlm, &(current + amount));
    }
}

// ----------------------------------------------------------------
// Reentrancy Guard
// ----------------------------------------------------------------

use crate::errors::ContractError;

/// Calls the provided closure with a reentrancy lock set in instance storage.
/// Returns Error::Reentrancy if already locked.
pub fn with_reentrancy_guard<F, R>(env: &Env, f: F) -> Result<R, ContractError>
where
    F: FnOnce() -> Result<R, ContractError>,
{
    let locked: bool = env
        .storage()
        .instance()
        .get(&DataKey::ReentrancyLock)
        .unwrap_or(false);
    if locked {
        return Err(ContractError::Reentrancy);
    }
    env.storage()
        .instance()
        .set(&DataKey::ReentrancyLock, &true);
    let result = f();
    env.storage()
        .instance()
        .set(&DataKey::ReentrancyLock, &false);
    result
// Multi-sig Admin Helpers (Issue #124)
// ----------------------------------------------------------------

pub fn get_multisig_admin(env: &Env) -> Option<crate::multisig::MultisigAdmin> {
    env.storage().instance().get(&DataKey::MultisigAdmin)
}

pub fn set_multisig_admin(env: &Env, admin: &crate::multisig::MultisigAdmin) {
    env.storage().instance().set(&DataKey::MultisigAdmin, admin);
}

pub fn get_multisig_proposal(env: &Env, proposal_id: u64) -> Option<crate::multisig::MultisigProposal> {
    env.storage()
        .persistent()
        .get(&DataKey::MultisigProposal(proposal_id))
}

pub fn save_multisig_proposal(env: &Env, proposal: &crate::multisig::MultisigProposal) {
    env.storage()
        .persistent()
        .set(&DataKey::MultisigProposal(proposal.id), proposal);
}

pub fn get_next_proposal_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::MultisigProposalCounter)
        .unwrap_or(1)
}

pub fn increment_proposal_id(env: &Env) {
    let next_id = get_next_proposal_id(env) + 1;
    env.storage()
        .instance()
        .set(&DataKey::MultisigProposalCounter, &next_id);
}

// ----------------------------------------------------------------
// LP Portfolio Stats Helpers (Issue #116)
// ----------------------------------------------------------------

pub fn get_lp_portfolio_stats(env: &Env, lp: &Address) -> crate::invoice::LPStats {
    env.storage()
        .persistent()
        .get(&DataKey::LPPortfolioStats(lp.clone()))
        .unwrap_or(crate::invoice::LPStats {
            total_funded: 0,
            total_earned: 0,
            active_positions: 0,
            total_positions: 0,
            avg_yield_bps: 0,
        })
}

pub fn save_lp_portfolio_stats(env: &Env, lp: &Address, stats: &crate::invoice::LPStats) {
    let key = DataKey::LPPortfolioStats(lp.clone());
    env.storage().persistent().set(&key, stats);
    env.storage()
        .persistent()
        .extend_ttl(&key, 1_000_000, 2_000_000);
}
