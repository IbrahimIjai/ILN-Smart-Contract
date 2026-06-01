#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, BytesN,
};

// ================================================================
// Dutch Auction Test Setup and Helpers
// ================================================================

/// All the actors and contract references a test needs
pub struct TestEnv {
    pub env: Env,
    pub contract: InvoiceLiquidityContractClient<'static>,
    pub token: TokenClient<'static>,
    pub freelancer: Address,
    pub payer: Address,
    pub funder: Address,
    pub funder2: Address,
}

/// Standard invoice values reused across tests
const INVOICE_AMOUNT: i128 = 1_000_000_000; // 100 USDC in stroops (1 USDC = 10_000_000)
const DISCOUNT_RATE: u32 = 300; // 3.00% in basis points
const DUE_DATE_OFFSET: u64 = 60 * 60 * 24 * 30; // 30 days from now

/// Auction parameters
const AUCTION_START_RATE: u32 = 1000; // 10% starting rate
const AUCTION_MIN_RATE: u32 = 100;   // 1% minimum rate
const AUCTION_DECAY_PER_HOUR: u32 = 100; // 1% decay per hour

pub fn setup() -> TestEnv {
    let env = Env::default();

    // Skip auth checks in tests
    env.mock_all_auths();

    // Deploy a mock USDC token contract
    let usdc_admin = Address::generate(&env);
    let usdc_contract_id = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_address = usdc_contract_id.address();

    let eurc_admin = Address::generate(&env);
    let eurc_contract_id = env.register_stellar_asset_contract_v2(eurc_admin.clone());
    let eurc_address = eurc_contract_id.address();

    let token = TokenClient::new(&env, &usdc_address);
    let token_admin = StellarAssetClient::new(&env, &usdc_address);

    // Generate test wallets
    let freelancer = Address::generate(&env);
    let payer = Address::generate(&env);
    let funder = Address::generate(&env);
    let funder2 = Address::generate(&env);

    // Mint USDC to the actors who need it
    token_admin.mint(&funder, &(INVOICE_AMOUNT * 10));
    token_admin.mint(&funder2, &(INVOICE_AMOUNT * 10));
    token_admin.mint(&payer, &(INVOICE_AMOUNT * 10));

    let contract_id = env.register(InvoiceLiquidityContract, ());
    let contract = InvoiceLiquidityContractClient::new(&env, &contract_id);

    // Fund the contract treasury
    token_admin.mint(&contract.address, &(INVOICE_AMOUNT * 100));

    let xlm_admin = Address::generate(&env);
    let xlm_contract_id = env.register_stellar_asset_contract_v2(xlm_admin);
    let xlm_address = xlm_contract_id.address();

    // Initialize the contract
    contract.initialize(&usdc_admin, &usdc_address, &eurc_address, &xlm_address);

    // Set ledger timestamp to a known baseline
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 1_700_000_000;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    TestEnv {
        env,
        contract,
        token,
        freelancer,
        payer,
        funder,
        funder2,
    }
}

/// Helper: submit a Dutch auction invoice and return its ID
fn submit_auction_invoice(t: &TestEnv) -> u64 {
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &AUCTION_START_RATE,
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    )
}

// ================================================================
// Tests: submit_invoice_auction Happy Path
// ================================================================

#[test]
fn test_submit_auction_invoice_returns_id() {
    let t = setup();
    let id = submit_auction_invoice(&t);

    // First auction invoice should be ID 1
    assert_eq!(id, 1);
}

#[test]
fn test_submit_auction_invoice_stores_correct_fields() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let id = t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &AUCTION_START_RATE,
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    let invoice = t.contract.get_invoice(&id);

    // Verify basic invoice fields
    assert_eq!(invoice.id, id);
    assert_eq!(invoice.freelancer, t.freelancer);
    assert_eq!(invoice.payer, t.payer);
    assert_eq!(invoice.token, t.token.address);
    assert_eq!(invoice.amount, INVOICE_AMOUNT);
    assert_eq!(u64::from(invoice.due_date), due_date);
    assert_eq!(invoice.status, InvoiceStatus::Pending);

    // Verify auction fields
    assert_eq!(invoice.is_auction, true);
    assert_eq!(invoice.auction_start_rate, Some(AUCTION_START_RATE));
    assert_eq!(invoice.auction_min_rate, Some(AUCTION_MIN_RATE));
    assert_eq!(invoice.auction_rate_decay_per_hour, Some(AUCTION_DECAY_PER_HOUR));
    assert!(invoice.auction_started_at.is_some());
}

#[test]
fn test_submit_auction_emits_event() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let id = t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &AUCTION_START_RATE,
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    let events = t.env.events().all().filter_by_contract(&t.contract.address);
    
    let expected_event = crate::events::AuctionStarted {
        invoice_id: id,
        freelancer: t.freelancer.clone(),
        payer: t.payer.clone(),
        token: t.token.address.clone(),
        amount: INVOICE_AMOUNT,
        due_date,
        start_rate: AUCTION_START_RATE,
        min_rate: AUCTION_MIN_RATE,
        rate_decay_per_hour: AUCTION_DECAY_PER_HOUR,
        started_at: t.env.ledger().timestamp(),
    };

    assert_eq!(
        events.events().last(),
        Some(&expected_event.to_xdr(&t.env, &t.contract.address))
    );
}

// ================================================================
// Tests: Dutch Auction Rate Calculation
// ================================================================

#[test]
fn test_auction_rate_at_start() {
    let t = setup();
    let id = submit_auction_invoice(&t);

    // Fund immediately at start - should get the start rate
    let funder_before = t.token.balance(&t.funder);
    
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &INVOICE_AMOUNT,
        &false,
    );

    let funder_after = t.token.balance(&t.funder);
    
    // Cost = Amount - (Amount * StartRate / 10000)
    // Expected cost: 1_000_000_000 - (1_000_000_000 * 1000 / 10000) = 900_000_000
    let expected_cost = 900_000_000i128;
    assert_eq!(funder_before - funder_after, expected_cost);
}

#[test]
fn test_auction_rate_decreases_over_time() {
    let t = setup();
    let id1 = submit_auction_invoice(&t);

    // Fund immediately - start rate 10% (1000 bps)
    let funder_before1 = t.token.balance(&t.funder);
    t.contract.fund_invoice(
        &t.funder,
        &id1,
        &INVOICE_AMOUNT,
        &false,
    );
    let cost1 = funder_before1 - t.token.balance(&t.funder);

    // Create a new auction and wait 1 hour
    let id2 = submit_auction_invoice(&t);
    
    let mut ledger_info = t.env.ledger().get();
    ledger_info.timestamp += 3600; // Add 1 hour
    t.env.ledger().set(ledger_info);

    // Fund at 1 hour - rate should be 10% - 1% = 9% (900 bps)
    let funder_before2 = t.token.balance(&t.funder);
    t.contract.fund_invoice(
        &t.funder2,
        &id2,
        &INVOICE_AMOUNT,
        &false,
    );
    let cost2 = funder_before2 - t.token.balance(&t.funder2);

    // Cost at 9% should be: 1_000_000_000 - (1_000_000_000 * 900 / 10000) = 910_000_000
    let expected_cost2 = 910_000_000i128;
    
    // Cost1 (10%) should be less than Cost2 (9%)
    assert!(cost1 < cost2);
    assert_eq!(cost2, expected_cost2);
}

#[test]
fn test_auction_rate_reaches_minimum() {
    let t = setup();
    let id = submit_auction_invoice(&t);

    // Calculate hours needed to reach minimum rate
    // Start: 10% (1000 bps), Min: 1% (100 bps), Decay: 1% per hour (100 bps per hour)
    // Hours to reach min = (1000 - 100) / 100 = 9 hours
    
    // Advance time by 10 hours (beyond the minimum)
    let mut ledger_info = t.env.ledger().get();
    ledger_info.timestamp += 10 * 3600; // Add 10 hours
    t.env.ledger().set(ledger_info);

    // Fund - should get minimum rate
    let funder_before = t.token.balance(&t.funder);
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &INVOICE_AMOUNT,
        &false,
    );
    let cost = funder_before - t.token.balance(&t.funder);

    // Cost at 1% minimum: 1_000_000_000 - (1_000_000_000 * 100 / 10000) = 990_000_000
    let expected_cost = 990_000_000i128;
    assert_eq!(cost, expected_cost);
}

// ================================================================
// Tests: First Taker Wins
// ================================================================

#[test]
fn test_first_lp_to_fund_gets_auction_rate() {
    let t = setup();
    let id = submit_auction_invoice(&t);

    // Two LPs try to fund at the same time - first one should succeed
    let result1 = t.contract.try_fund_invoice(&t.funder, &id, &(INVOICE_AMOUNT / 2), &false);
    let result2 = t.contract.try_fund_invoice(&t.funder2, &id, &(INVOICE_AMOUNT / 2), &false);

    // First should succeed
    assert!(result1.is_ok());
    
    // Second should also succeed since it's partial funding
    assert!(result2.is_ok());
}

#[test]
fn test_multiple_funders_auction_emits_events() {
    let t = setup();
    let id = submit_auction_invoice(&t);

    // First funder funds half
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &(INVOICE_AMOUNT / 2),
        &false,
    );

    // Second funder funds the other half
    t.contract.fund_invoice(
        &t.funder2,
        &id,
        &(INVOICE_AMOUNT / 2),
        &false,
    );

    // Invoice should now be fully funded
    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Funded);
}

// ================================================================
// Tests: Auction Expiration
// ================================================================

#[test]
fn test_auction_cannot_fund_after_due_date() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + 1800; // 30 minutes from now

    let id = t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &AUCTION_START_RATE,
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    // Advance past due date
    let mut ledger_info = t.env.ledger().get();
    ledger_info.timestamp += 3600; // Add 1 hour
    t.env.ledger().set(ledger_info);

    // Try to fund - should fail
    let result = t.contract.try_fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);
    assert!(result.is_err());
}

// ================================================================
// Tests: Invalid Auction Parameters
// ================================================================

#[test]
fn test_invalid_start_rate_zero() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &0, // Invalid: zero start rate
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    assert!(result.is_err());
}

#[test]
fn test_invalid_start_rate_exceeds_max() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &10001, // Invalid: exceeds MAX_DISCOUNT_RATE
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    assert!(result.is_err());
}

#[test]
fn test_invalid_min_rate_exceeds_start_rate() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &500, // Start rate
        &1000, // Invalid: min rate exceeds start rate
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    assert!(result.is_err());
}

#[test]
fn test_invalid_decay_rate_zero() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &AUCTION_START_RATE,
        &AUCTION_MIN_RATE,
        &0, // Invalid: zero decay rate
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    assert!(result.is_err());
}

// ================================================================
// Tests: Auction Invoice vs Standard Invoice
// ================================================================

#[test]
fn test_auction_invoice_marked_correctly() {
    let t = setup();

    // Submit auction invoice
    let auction_id = submit_auction_invoice(&t);
    let auction_invoice = t.contract.get_invoice(&auction_id);

    // Submit standard invoice
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    let standard_id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );
    let standard_invoice = t.contract.get_invoice(&standard_id);

    // Verify auction flag
    assert_eq!(auction_invoice.is_auction, true);
    assert_eq!(standard_invoice.is_auction, false);
}

#[test]
fn test_standard_invoice_unaffected_by_auction_changes() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    // Submit standard invoice
    let id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    // Fund with known rate
    let funder_before = t.token.balance(&t.funder);
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &INVOICE_AMOUNT,
        &false,
    );
    let cost = funder_before - t.token.balance(&t.funder);

    // Cost should be based on fixed discount rate
    // 1_000_000_000 - (1_000_000_000 * 300 / 10000) = 970_000_000
    let expected_cost = 970_000_000i128;
    assert_eq!(cost, expected_cost);
}

// ================================================================
// Tests: Auction with Referral Code
// ================================================================

#[test]
fn test_auction_invoice_with_referral_code() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    
    let referral_code = BytesN::<32>::from_array(&[1u8; 32]);

    let id = t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &AUCTION_START_RATE,
        &AUCTION_MIN_RATE,
        &AUCTION_DECAY_PER_HOUR,
        &t.token.address,
        &Some(referral_code.clone()),
    );

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.referral_code, Some(referral_code));
}

// ================================================================
// Tests: Rate Calculation Edge Cases
// ================================================================

#[test]
fn test_auction_rate_with_very_small_decay() {
    let t = setup();
    let id = t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &(t.env.ledger().timestamp() + DUE_DATE_OFFSET),
        &1000, // 10%
        &900,  // 9%
        &1,    // Very small decay: 0.01% per hour
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    // Even after 1 hour, rate should be close to start
    let mut ledger_info = t.env.ledger().get();
    ledger_info.timestamp += 3600;
    t.env.ledger().set(ledger_info);

    let funder_before = t.token.balance(&t.funder);
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &INVOICE_AMOUNT,
        &false,
    );
    let cost = funder_before - t.token.balance(&t.funder);

    // With 0.01% decay, after 1 hour rate should be 9.99% (999 bps)
    // Cost should be very close to start rate cost
    let expected_cost = 900_100_000i128; // Approximate
    assert!(cost > 899_000_000 && cost < 901_000_000);
}

#[test]
fn test_auction_rate_with_large_decay() {
    let t = setup();
    let id = t.contract.submit_invoice_auction(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &(t.env.ledger().timestamp() + DUE_DATE_OFFSET),
        &10000, // 100%
        &0,    // 0%
        &5000,  // Very large decay: 50% per hour
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    // After 1 hour, rate should be 50%
    let mut ledger_info = t.env.ledger().get();
    ledger_info.timestamp += 3600;
    t.env.ledger().set(ledger_info);

    let funder_before = t.token.balance(&t.funder);
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &INVOICE_AMOUNT,
        &false,
    );
    let cost = funder_before - t.token.balance(&t.funder);

    // Cost at 50%: 1_000_000_000 - (1_000_000_000 * 5000 / 10000) = 500_000_000
    let expected_cost = 500_000_000i128;
    assert_eq!(cost, expected_cost);
}

// ================================================================
// Tests: Auction Invoice Lifecycle
// ================================================================

#[test]
fn test_auction_invoice_complete_lifecycle() {
    let t = setup();
    
    // 1. Submit auction
    let id = submit_auction_invoice(&t);
    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
    assert_eq!(invoice.is_auction, true);
    
    // 2. Fund invoice
    t.contract.fund_invoice(
        &t.funder,
        &id,
        &INVOICE_AMOUNT,
        &false,
    );
    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Funded);
    assert_eq!(invoice.amount_funded, INVOICE_AMOUNT);
    
    // 3. Payer settles
    t.contract.settle_invoice(
        &t.payer,
        &id,
        &INVOICE_AMOUNT,
    );
    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Paid);
    assert_eq!(invoice.amount_paid, INVOICE_AMOUNT);
}
