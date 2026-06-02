#![cfg(test)]

//! Property-based tests for discount calculation invariants.
//!
//! Issue #85 — verifies that the three invariants hold across all valid
//! inputs, using `proptest` with 10,000 random cases.
//!
//! Invariants tested:
//!   P1: amount_freelancer_receives + discount_amount == invoice_amount
//!   P2: lp_net_earnings > 0
//!   P3: lp_net_earnings == discount_rate * invoice_amount / 10_000
//!       (exact equality; integer division is consistent throughout)
//!
//! Math:
//!   discount_amount  = invoice_amount * discount_rate / 10_000
//!   LP pays (cost)   = invoice_amount - discount_amount
//!   freelancer gets  = invoice_amount - discount_amount  (= cost)
//!   payer pays       = invoice_amount  (full face value)
//!   LP receives back = invoice_amount  (fee_rate defaults to 0)
//!   LP net           = invoice_amount - cost = discount_amount  ✓

use super::*;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

// ── Constants ─────────────────────────────────────────────────────────────────

const LEDGER_TIMESTAMP: u64 = 1_700_000_000;

// Valid invoice amount range: ≥ MIN_INVOICE_AMOUNT (1_000_000) and safe for
// intermediate arithmetic (amount * discount_rate ≤ 10_000_000_000 * 5_000 = 5e13 << i128::MAX).
const AMOUNT_MIN: i128 = 1_000_000;
const AMOUNT_MAX: i128 = 10_000_000_000;

// Valid discount rate range (as enforced by validate_invoice_terms).
const RATE_MIN: u32 = 1;
const RATE_MAX: u32 = 5000;

// Valid due-date offset range:
//   MIN_INVOICE_DURATION = 86_400 s (24 h)
//   MAX_INVOICE_DURATION = 31_536_000 s (365 d)
const DUE_OFFSET_MIN: u64 = 86_400;
const DUE_OFFSET_MAX: u64 = 31_536_000;

// ── Test environment ──────────────────────────────────────────────────────────

#[allow(dead_code)]
struct InvariantEnv {
    env: Env,
    contract: InvoiceLiquidityContractClient<'static>,
    token: TokenClient<'static>,
    freelancer: Address,
    payer: Address,
    funder: Address,
}

/// Set up a fresh environment for one property-based test case.
///
/// Mints exactly `invoice_amount` to the payer and `invoice_amount` to the
/// funder (more than the cost they will pay, so any remainder stays in their
/// wallet and is captured by the balance-delta approach).
fn setup_invariant(invoice_amount: i128) -> InvariantEnv {
    let env = Env::default();
    env.mock_all_auths();

    let usdc_admin = Address::generate(&env);
    let usdc_id = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_address = usdc_id.address();

    let token = TokenClient::new(&env, &usdc_address);
    let token_admin = StellarAssetClient::new(&env, &usdc_address);

    let freelancer = Address::generate(&env);
    let payer = Address::generate(&env);
    let funder = Address::generate(&env);

    // Mint enough for the full invoice flow.
    token_admin.mint(&funder, &invoice_amount);
    token_admin.mint(&payer, &invoice_amount);

    let xlm_admin = Address::generate(&env);
    let xlm_id = env.register_stellar_asset_contract_v2(xlm_admin);
    let xlm_address = xlm_id.address();

    let contract_id = env.register(InvoiceLiquidityContract, ());
    let contract = InvoiceLiquidityContractClient::new(&env, &contract_id);
    let eurc_address = Address::generate(&env);
    contract.initialize(&usdc_admin, &usdc_address, &eurc_address, &xlm_address);

    let mut ledger = env.ledger().get();
    ledger.timestamp = LEDGER_TIMESTAMP;
    env.ledger().set(ledger);

    InvariantEnv {
        env,
        contract,
        token,
        freelancer,
        payer,
        funder,
    }
}

// ── Property tests ────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// Verifies all three discount invariants across the full valid input space.
    ///
    /// A single proptest block is used to avoid 3× the test-case overhead
    /// while still checking all invariants for every generated (amount, rate,
    /// due_date) triple.
    #[test]
    fn prop_discount_invariants_hold(
        invoice_amount  in AMOUNT_MIN..=AMOUNT_MAX,
        discount_rate   in RATE_MIN..=RATE_MAX,
        due_date_offset in DUE_OFFSET_MIN..=DUE_OFFSET_MAX,
    ) {
        let t = setup_invariant(invoice_amount);

        // ------------------------------------------------------------------
        // Submit invoice
        // ------------------------------------------------------------------
        let due_date = LEDGER_TIMESTAMP + due_date_offset;
        let invoice_id = t.contract.submit_invoice(
            &t.freelancer,
            &t.payer,
            &invoice_amount,
            &due_date,
            &discount_rate,
            &t.token.address,
        );

        // Expected discount: same integer division the contract uses.
        let discount_amount = invoice_amount * discount_rate as i128 / 10_000;
        let _cost = invoice_amount - discount_amount; // what LP pays

        // ------------------------------------------------------------------
        // Record balances before funding
        // ------------------------------------------------------------------
        let freelancer_before = t.token.balance(&t.freelancer);
        let funder_before     = t.token.balance(&t.funder);

        // ------------------------------------------------------------------
        // Fund invoice (LP funds full face value)
        // ------------------------------------------------------------------
        t.contract.fund_invoice(&t.funder, &invoice_id, &invoice_amount);

        let freelancer_after_fund = t.token.balance(&t.freelancer);

        // ------------------------------------------------------------------
        // Property 1: amount_freelancer_receives + discount == invoice_amount
        // ------------------------------------------------------------------
        let freelancer_received = freelancer_after_fund - freelancer_before;

        prop_assert_eq!(
            freelancer_received + discount_amount,
            invoice_amount,
            "P1 violated: freelancer_received({}) + discount({}) != invoice_amount({})",
            freelancer_received,
            discount_amount,
            invoice_amount,
        );

        // ------------------------------------------------------------------
        // Mark invoice paid (payer pays full face value)
        // ------------------------------------------------------------------
        t.contract.mark_paid(&invoice_id, &invoice_amount);

        let funder_after_paid = t.token.balance(&t.funder);

        // ------------------------------------------------------------------
        // Property 2: LP net earnings > 0
        // ------------------------------------------------------------------
        // lp_net = tokens_received_total - tokens_spent_total
        //        = (funder_after_paid - funder_before_fund)   [net delta]
        //          NB: funder_before == invoice_amount (minted above)
        //              funder pays cost, then receives invoice_amount back
        //              so delta = invoice_amount - cost = discount_amount
        let lp_net = funder_after_paid - funder_before;

        prop_assert!(
            lp_net > 0,
            "P2 violated: lp_net({}) is not positive (invoice={}, rate={})",
            lp_net,
            invoice_amount,
            discount_rate,
        );

        // ------------------------------------------------------------------
        // Property 3: lp_net == discount_rate * invoice_amount / 10_000
        //
        // Integer division is identical in both places, so equality is exact.
        // ------------------------------------------------------------------
        let expected_earnings = discount_rate as i128 * invoice_amount / 10_000;

        prop_assert_eq!(
            lp_net,
            expected_earnings,
            "P3 violated: lp_net({}) != expected_earnings({}) (invoice={}, rate={})",
            lp_net,
            expected_earnings,
            invoice_amount,
            discount_rate,
        );
    }
}
