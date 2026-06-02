pub use crate::storage::DataKey as StorageKey;
use soroban_sdk::{contracttype, Address, BytesN, Env, IntoVal, Symbol};

// ----------------------------------------------------------------
// Status enum — tracks lifecycle of invoice
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvoiceStatus {
    Pending,         // submitted, waiting for a liquidity provider to fund it
    Funded,          // LP has funded it, freelancer has been paid out
    PartiallyFunded, // partially funded by one or more LPs
    Paid,            // payer has settled in full, LP has been released
    Defaulted,       // past due_date and still unpaid
    Appealed,        // payer has contested the default ruling (issue #36)
    Disputed,        // payer has disputed the invoice before settlement
    Expired,         // past due_date with no funding
    Cancelled,       // freelancer cancelled the invoice before funding
}

// ----------------------------------------------------------------
// Invoice struct (UPDATED - token stays per invoice)
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Invoice {
    pub id: u64,
    pub freelancer: Address, // who submitted the invoice (receives liquidity)
    pub payer: Address,      // the client who owes the money
    pub token: Address,      // token used for this invoice lifecycle
    pub amount: i128,        // full invoice value in stroops (1 USDC = 10_000_000)
    pub due_date: u32,       // Unix timestamp — when the payer must settle by
    pub discount_rate: u32,  // basis points, e.g. 300 = 3.00%
    pub status: InvoiceStatus,
    pub funder: Option<Address>, // set when an LP funds the invoice (legacy for full funding)
    pub funded_at: Option<u32>,  // ledger timestamp when funding occurred
    pub amount_funded: i128,     // cumulative amount funded so far
    pub amount_paid: i128,       // cumulative amount paid by the payer
    pub referral_code: Option<BytesN<32>>, // optional referral code used at submission
    pub submitter_reputation: u32, // snapshot of freelancer's reputation at submission time
    // Dutch auction fields
    pub is_auction: bool,    // whether this invoice uses Dutch auction pricing
    pub auction_start_rate: Option<u32>, // starting rate in basis points
    pub auction_min_rate: Option<u32>,   // minimum rate in basis points
    pub auction_rate_decay_per_hour: Option<u32>, // decay in basis points per hour
    pub auction_started_at: Option<u32>, // timestamp when auction was started
    /// Issue #122: Optional whitelist of LP addresses allowed to fund this invoice.
    /// If empty/None, invoice is public. If Some, only whitelisted LPs can fund.
    /// Capped at 10 addresses to limit storage.
    pub allowed_lps: Option<soroban_sdk::Vec<Address>>,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InvoiceParams {
    pub freelancer: Address,
    pub payer: Address,
    pub amount: i128,
    pub due_date: u64,
    pub discount_rate: u32,
    pub token: Address,
    pub referral_code: Option<BytesN<32>>,
    /// Issue #122: Optional whitelist of allowed LPs for this invoice
    pub allowed_lps: Option<soroban_sdk::Vec<Address>>,
}

#[contracttype]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PayerStats {
    pub total_invoices: u64,
    pub paid_on_time: u64,
    pub defaults: u64,
    pub total_volume: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReputationScore {
    pub score: u32,
    pub last_activity_ledger: u32,
}

/// Detailed reputation profile for an address (Issue #26).
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReputationProfile {
    pub address: Address,
    pub invoices_submitted: u32,
    pub invoices_paid: u32,
    pub invoices_defaulted: u32,
    pub score: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ContractStats {
    pub total_invoices: u64,
    pub total_funded: u64,
    pub total_paid: u64,
    pub total_volume_usdc: i128,
    pub total_volume_eurc: i128,
    pub total_volume_xlm: i128,
    pub token_volumes: soroban_sdk::Vec<(Address, i128)>,
    pub total_volume_usd_normalized: i128,
}

impl Default for ContractStats {
    fn default() -> Self {
        panic!("Use ContractStats::empty(env) instead of Default")
    }
}

impl ContractStats {
    pub fn empty(env: &Env) -> Self {
        Self {
            total_invoices: 0,
            total_funded: 0,
            total_paid: 0,
            total_volume_usdc: 0,
            total_volume_eurc: 0,
            total_volume_xlm: 0,
            token_volumes: soroban_sdk::Vec::new(env),
            total_volume_usd_normalized: 0,
        }
    }
/// Per-LP analytics snapshot (Issue #116).
///
/// Updated incrementally on every `fund_invoice` and `mark_paid` call so the
/// dashboard can read a single storage slot instead of iterating all invoices.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LPStats {
    /// Cumulative token-amount sent as capital across all funded invoices.
    pub total_funded: i128,
    /// Cumulative yield earned (payout received minus capital deployed).
    pub total_earned: i128,
    /// Number of invoices currently in `Funded` state for this LP.
    pub active_positions: u32,
    /// Total number of invoice positions this LP has ever funded.
    pub total_positions: u32,
    /// Simple average discount rate in basis points across all positions
    /// (sum of discount_rate_bps / total_positions), or 0 when no positions.
    pub avg_yield_bps: u32,
}

// ----------------------------------------------------------------
// Issue #36: Appeal record stored per invoice
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AppealRecord {
    pub evidence_hash: BytesN<32>,
    pub appealed_at: u32,
    pub pre_default_score: u32,
}

// ----------------------------------------------------------------
// Dispute record stored per invoice
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DisputeRecord {
    pub reason_hash: BytesN<32>,
    pub disputed_at: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct TopPayerEntry {
    pub address: Address,
    pub score: u32,
}

// ----------------------------------------------------------------
// Issue #34: Single entry in the LP priority queue
// ----------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct LpFundRequest {
    pub lp: Address,
    pub score: u32,
}

// ----------------------------------------------------------------
// Storage helpers — core invoice CRUD
// ----------------------------------------------------------------

pub fn get_submitter_invoices(env: &Env, submitter: &Address) -> soroban_sdk::Vec<u64> {
    env.storage()
        .persistent()
        .get(&StorageKey::SubmitterInvoices(submitter.clone()))
        .unwrap_or(soroban_sdk::Vec::new(env))
}

pub fn add_invoice_to_submitter(env: &Env, submitter: &Address, invoice_id: u64) {
    let mut invoices = get_submitter_invoices(env, submitter);
    invoices.push_back(invoice_id);
    let key = StorageKey::SubmitterInvoices(submitter.clone());
    env.storage().persistent().set(&key, &invoices);
}

pub fn remove_invoice_from_submitter(env: &Env, submitter: &Address, invoice_id: u64) {
    let invoices = get_submitter_invoices(env, submitter);
    let mut new_invoices = soroban_sdk::Vec::new(env);
    for id in invoices.iter() {
        if id != invoice_id {
            new_invoices.push_back(id);
        }
    }
    let key = StorageKey::SubmitterInvoices(submitter.clone());
    if new_invoices.is_empty() {
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
        }
    } else {
        env.storage().persistent().set(&key, &new_invoices);
        env.storage()
            .persistent()
            .extend_ttl(&key, 1_000_000, 2_000_000);
    }
}

pub fn get_lp_invoices(env: &Env, lp: &Address) -> soroban_sdk::Vec<u64> {
    env.storage()
        .persistent()
        .get(&StorageKey::LpInvoices(lp.clone()))
        .unwrap_or(soroban_sdk::Vec::new(env))
}

pub fn add_invoice_to_lp(env: &Env, lp: &Address, invoice_id: u64) {
    let mut invoices = get_lp_invoices(env, lp);
    let mut exists = false;
    for id in invoices.iter() {
        if id == invoice_id {
            exists = true;
            break;
        }
    }
    if !exists {
        invoices.push_back(invoice_id);
        let key = StorageKey::LpInvoices(lp.clone());
        env.storage().persistent().set(&key, &invoices);
    }
}

pub fn remove_invoice_from_lp(env: &Env, lp: &Address, invoice_id: u64) {
    let invoices = get_lp_invoices(env, lp);
    let mut new_invoices = soroban_sdk::Vec::new(env);
    for id in invoices.iter() {
        if id != invoice_id {
            new_invoices.push_back(id);
        }
    }
    let key = StorageKey::LpInvoices(lp.clone());
    if new_invoices.is_empty() {
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
        }
    } else {
        env.storage().persistent().set(&key, &new_invoices);
        env.storage()
            .persistent()
            .extend_ttl(&key, 1_000_000, 2_000_000);
    }
}

pub fn save_invoice(env: &Env, invoice: &Invoice) {
    let key = StorageKey::Invoice(invoice.id);
    
    // Track state count changes
    if let Some(old_invoice) = env.storage().persistent().get::<_, Invoice>(&key) {
        if old_invoice.status != invoice.status {
            crate::storage::decrement_state_count(env, &old_invoice.status);
            crate::storage::increment_state_count(env, &invoice.status);
        }
    } else {
        // New invoice
        crate::storage::increment_state_count(env, &invoice.status);
    }
    
    env.storage().persistent().set(&key, invoice);
    env.storage()
        .persistent()
        .extend_ttl(&key, 1_000_000, 2_000_000);
}

pub fn load_invoice(env: &Env, id: u64) -> Invoice {
    env.storage()
        .persistent()
        .get(&StorageKey::Invoice(id))
        .expect("invoice not found")
}

pub fn try_load_invoice(env: &Env, id: u64) -> Option<Invoice> {
    env.storage().persistent().get(&StorageKey::Invoice(id))
}

pub fn invoice_exists(env: &Env, id: u64) -> bool {
    env.storage().persistent().has(&StorageKey::Invoice(id))
}

// ----------------------------------------------------------------
// Reputation Score
// ----------------------------------------------------------------

pub fn get_payer_score(env: &Env, payer: &Address) -> u32 {
    match env
        .storage()
        .persistent()
        .get::<StorageKey, ReputationScore>(&StorageKey::PayerScore(payer.clone()))
    {
        Some(mut rep) => {
            if let Some(decay_config) = crate::storage::get_config(env) {
                let current_ledger = env.ledger().sequence();
                let ledgers_since_activity =
                    current_ledger.saturating_sub(rep.last_activity_ledger);

                if u64::from(ledgers_since_activity) >= decay_config.decay_period_ledgers
                    && decay_config.decay_period_ledgers > 0
                    && decay_config.decay_rate_bps > 0
                {
                    let periods_passed =
                        u64::from(ledgers_since_activity) / decay_config.decay_period_ledgers;

                    let mut decayed_score = rep.score as u64;
                    for _ in 0..periods_passed {
                        let mut decay_amount =
                            (decayed_score * decay_config.decay_rate_bps as u64) / 10_000;
                        if decay_amount == 0 && decayed_score > 0 {
                            decay_amount = 1;
                        }
                        decayed_score = decayed_score.saturating_sub(decay_amount);
                    }

                    let new_score = (decayed_score.min(100)) as u32;
                    if new_score != rep.score {
                        rep.score = new_score;
                        rep.last_activity_ledger = current_ledger;
                        env.storage()
                            .persistent()
                            .set(&StorageKey::PayerScore(payer.clone()), &rep);

                        // Sync with ReputationProfile and trigger event
                        let mut profile = get_reputation(env, payer);
                        profile.score = new_score;
                        set_reputation(env, &profile);
                    }
                }
            }
            rep.score
        }
        None => crate::constants::DEFAULT_PAYER_SCORE,
    }
}

fn payer_score_key(payer: &Address) -> StorageKey {
    StorageKey::PayerScore(payer.clone())
}

pub fn set_payer_score(env: &Env, payer: &Address, score: u32) {
    let score = score.min(100);
    let key = payer_score_key(payer);
    let old_score = get_payer_score(env, payer);

    if score == crate::constants::DEFAULT_PAYER_SCORE {
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
        }
    } else {
        let rep = ReputationScore {
            score,
            last_activity_ledger: env.ledger().sequence(),
        };
        env.storage().persistent().set(&key, &rep);
    }

    // Sync with ReputationProfile so they are completely aligned
    let mut profile = get_reputation(env, payer);
    profile.score = score;
    set_reputation(env, &profile);

    if old_score != score {
        crate::top_payers::update_top_payers_on_score_change(env, payer, score);
    }
}

pub fn get_reputation(env: &Env, address: &Address) -> ReputationProfile {
    env.storage()
        .persistent()
        .get(&StorageKey::Reputation(address.clone()))
        .unwrap_or(ReputationProfile {
            address: address.clone(),
            invoices_submitted: 0,
            invoices_paid: 0,
            invoices_defaulted: 0,
            score: 0,
        })
}

pub fn set_reputation(env: &Env, profile: &ReputationProfile) {
    let key = StorageKey::Reputation(profile.address.clone());
    let old_profile = get_reputation(env, &profile.address);
    let old_score = old_profile.score;
    let new_score = profile.score;

    let is_empty = profile.invoices_submitted == 0
        && profile.invoices_paid == 0
        && profile.invoices_defaulted == 0
        && profile.score == 0;

    if is_empty {
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
        }
    } else {
        env.storage().persistent().set(&key, profile);
        env.storage()
            .persistent()
            .extend_ttl(&key, 1_000_000, 2_000_000);
    }

    if old_score != new_score
        || old_profile.invoices_submitted != profile.invoices_submitted
        || old_profile.invoices_paid != profile.invoices_paid
        || old_profile.invoices_defaulted != profile.invoices_defaulted
    {
        env.events().publish_event(&crate::events::ReputationUpdated {
            address: profile.address.clone(),
            old_score,
            new_score,
            invoices_submitted: profile.invoices_submitted,
            invoices_paid: profile.invoices_paid,
            invoices_defaulted: profile.invoices_defaulted,
        });
    }
}

pub fn increment_invoices_submitted(env: &Env, address: &Address) {
    let mut profile = get_reputation(env, address);
    profile.invoices_submitted += 1;
    set_reputation(env, &profile);
}

pub fn increment_invoices_paid(env: &Env, address: &Address) {
    let mut profile = get_reputation(env, address);
    profile.invoices_paid += 1;
    set_reputation(env, &profile);
}

pub fn increment_invoices_defaulted(env: &Env, address: &Address) {
    let mut profile = get_reputation(env, address);
    profile.invoices_defaulted += 1;
    set_reputation(env, &profile);
}

pub fn get_invoice_funders(env: &Env, id: u64) -> soroban_sdk::Vec<(Address, i128)> {
    env.storage()
        .persistent()
        .get(&StorageKey::InvoiceFunders(id))
        .unwrap_or(soroban_sdk::Vec::new(env))
}

pub fn save_invoice_funders(env: &Env, id: u64, funders: &soroban_sdk::Vec<(Address, i128)>) {
    let key = StorageKey::InvoiceFunders(id);
    if funders.is_empty() {
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
        }
    } else {
        env.storage().persistent().set(&key, funders);
    }
}

pub fn get_lp_score(env: &Env, lp: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&StorageKey::LpScore(lp.clone()))
        .unwrap_or(crate::constants::DEFAULT_LP_SCORE)
}

pub fn set_lp_score(env: &Env, lp: &Address, score: u32) {
    let score = score.min(100);
    let key = StorageKey::LpScore(lp.clone());

    if score == crate::constants::DEFAULT_LP_SCORE {
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
        }
    } else {
        env.storage().persistent().set(&key, &score);
    }
}

// ----------------------------------------------------------------
// LP Queue Helpers
// ----------------------------------------------------------------

pub fn get_fund_queue(env: &Env, invoice_id: u64) -> soroban_sdk::Vec<LpFundRequest> {
    env.storage()
        .persistent()
        .get(&StorageKey::FundQueue(invoice_id))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn save_fund_queue(env: &Env, invoice_id: u64, queue: &soroban_sdk::Vec<LpFundRequest>) {
    env.storage()
        .persistent()
        .set(&StorageKey::FundQueue(invoice_id), queue);
}

pub fn get_queue_resolution(env: &Env, invoice_id: u64) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&StorageKey::QueueResolution(invoice_id))
}

pub fn save_queue_resolution(env: &Env, invoice_id: u64, approved_lp: &Address) {
    env.storage()
        .persistent()
        .set(&StorageKey::QueueResolution(invoice_id), approved_lp);
}

// ----------------------------------------------------------------
// Appeal & Dispute Helpers
// ----------------------------------------------------------------

pub fn get_appeal(env: &Env, invoice_id: u64) -> Option<AppealRecord> {
    env.storage().persistent().get(&StorageKey::Appeal(invoice_id))
}

pub fn save_appeal(env: &Env, invoice_id: u64, record: &AppealRecord) {
    env.storage()
        .persistent()
        .set(&StorageKey::Appeal(invoice_id), record);
}

pub fn save_pre_default_payer_score(env: &Env, invoice_id: u64, score: u32) {
    env.storage()
        .persistent()
        .set(&StorageKey::PreDefaultPayerScore(invoice_id), &score);
}

pub fn get_pre_default_payer_score(env: &Env, invoice_id: u64) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&StorageKey::PreDefaultPayerScore(invoice_id))
}

pub fn get_dispute(env: &Env, invoice_id: u64) -> Option<DisputeRecord> {
    env.storage()
        .persistent()
        .get(&StorageKey::Dispute(invoice_id))
}

pub fn save_dispute(env: &Env, invoice_id: u64, record: &DisputeRecord) {
    env.storage()
        .persistent()
        .set(&StorageKey::Dispute(invoice_id), record);
}

// ----------------------------------------------------------------
// Contract stats helpers
// ----------------------------------------------------------------

/// Local accumulator for stat changes to be applied in a single write.
#[derive(Default)]
pub struct StatsDelta {
    pub total_invoices: u64,
    pub total_funded: u64,
    pub total_paid: u64,
    pub volume_usdc: i128,
    pub volume_eurc: i128,
    pub volume_xlm: i128,
}

impl StatsDelta {
    pub fn add_volume(&mut self, env: &Env, token: &Address, amount: i128) {
        if let Some(config) = crate::storage::get_config(env) {
            if token == &config.xlm_sac_address {
                self.volume_xlm += amount;
                return;
            }
        }
        
        let token_list: soroban_sdk::Vec<Address> = env
            .storage()
            .persistent()
            .get(&StorageKey::TokenList)
            .unwrap_or(soroban_sdk::Vec::new(env));

        if !token_list.is_empty() {
            if let Some(usdc_addr) = token_list.get(0) {
                if token == &usdc_addr {
                    self.volume_usdc += amount;
                }
            }
            if token_list.len() > 2 {
                if let Some(eurc_addr) = token_list.get(2) {
                    if token == &eurc_addr {
                        self.volume_eurc += amount;
                    }
                }
            }
        }
    }

    pub fn apply(&self, env: &Env) {
        if self.total_invoices == 0 && self.total_funded == 0 && self.total_paid == 0
           && self.volume_usdc == 0 && self.volume_eurc == 0 && self.volume_xlm == 0 {
            return;
        }

        let mut stats = get_contract_stats(env);
        stats.total_invoices += self.total_invoices;
        stats.total_funded += self.total_funded;
        stats.total_paid += self.total_paid;
        stats.total_volume_usdc += self.volume_usdc;
        stats.total_volume_eurc += self.volume_eurc;
        stats.total_volume_xlm += self.volume_xlm;

        save_contract_stats(env, &stats);
    }
}

pub fn get_contract_stats(env: &Env) -> ContractStats {
    env.storage()
        .instance()
        .get(&StorageKey::Stats)
        .unwrap_or_else(|| ContractStats::empty(env))
}

pub fn save_contract_stats(env: &Env, stats: &ContractStats) {
    env.storage().instance().set(&StorageKey::Stats, stats);
}
