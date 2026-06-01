#![cfg(test)]

//! Tests for `get_lp_portfolio_stats` (Issue #116).
//!
//! Each test exercises one or more fields of `LPStats` to guarantee the
//! incremental storage is kept correct across the full invoice lifecycle.

use crate::test::setup;
use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address};

// ---------------------------------------------------------------------------
// Helper constants (mirrored from test.rs for readability)
// ---------------------------------------------------------------------------
const INVOICE_AMOUNT: i128 = 1_000_000_000; // 100 USDC (1 USDC = 10_000_000 stroops)
const DISCOUNT_RATE: u32 = 300; // 3.00 %
const DUE_DATE_OFFSET: u64 = 60 * 60 * 24 * 30; // 30 days

// ---------------------------------------------------------------------------
// 1. Fresh LP has zeroed-out stats
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_default_for_unknown_lp() {
    let t = setup();
    let unknown = Address::generate(&t.env);

    let stats = t.contract.get_lp_portfolio_stats(&unknown);

    assert_eq!(stats.total_funded, 0);
    assert_eq!(stats.total_earned, 0);
    assert_eq!(stats.active_positions, 0);
    assert_eq!(stats.total_positions, 0);
    assert_eq!(stats.avg_yield_bps, 0);
}

// ---------------------------------------------------------------------------
// 2. After a single full funding — total_funded, total_positions, active_positions
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_after_single_full_fund() {
    let t = setup();
    let token_admin = StellarAssetClient::new(&t.env, &t.token.address);

    let lp = Address::generate(&t.env);
    token_admin.mint(&lp, &(INVOICE_AMOUNT * 2));

    let freelancer = Address::generate(&t.env);
    let payer = Address::generate(&t.env);
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let id = t.contract.submit_invoice(
        &freelancer,
        &payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
        &None,
    );

    t.contract.fund_invoice(&lp, &id, &INVOICE_AMOUNT, &false);

    let stats = t.contract.get_lp_portfolio_stats(&lp);

    assert_eq!(stats.total_funded, INVOICE_AMOUNT, "total_funded mismatch");
    assert_eq!(stats.total_positions, 1, "total_positions should be 1");
    assert_eq!(stats.active_positions, 1, "active_positions should be 1");
    assert_eq!(stats.total_earned, 0, "no payment yet → earned = 0");
    assert_eq!(
        stats.avg_yield_bps, DISCOUNT_RATE,
        "avg yield should equal the single invoice discount rate"
    );
}

// ---------------------------------------------------------------------------
// 3. total_earned and active_positions after payment
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_total_earned_after_payment() {
    let t = setup();
    let token_admin = StellarAssetClient::new(&t.env, &t.token.address);

    let lp = Address::generate(&t.env);
    token_admin.mint(&lp, &(INVOICE_AMOUNT * 2));
    let payer = Address::generate(&t.env);
    token_admin.mint(&payer, &(INVOICE_AMOUNT * 2));

    let freelancer = Address::generate(&t.env);
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let id = t.contract.submit_invoice(
        &freelancer,
        &payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
        &None,
    );

    t.contract.fund_invoice(&lp, &id, &INVOICE_AMOUNT, &false);
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let stats = t.contract.get_lp_portfolio_stats(&lp);

    // Expected yield = INVOICE_AMOUNT * DISCOUNT_RATE / 10_000
    let expected_yield = INVOICE_AMOUNT * DISCOUNT_RATE as i128 / 10_000;

    assert_eq!(
        stats.total_earned, expected_yield,
        "total_earned should equal the LP's share of the discount"
    );
    assert_eq!(
        stats.active_positions, 0,
        "active_positions should drop to 0 after payment"
    );
    assert_eq!(
        stats.total_positions, 1,
        "total_positions is permanent and stays at 1"
    );
}

// ---------------------------------------------------------------------------
// 4. Multiple invoices — total_positions, avg_yield_bps
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_multiple_invoices_avg_yield() {
    let t = setup();
    let token_admin = StellarAssetClient::new(&t.env, &t.token.address);

    let lp = Address::generate(&t.env);
    // Mint enough for 3 full invoices
    token_admin.mint(&lp, &(INVOICE_AMOUNT * 6));

    let freelancer = Address::generate(&t.env);
    let payer1 = Address::generate(&t.env);
    let payer2 = Address::generate(&t.env);
    let payer3 = Address::generate(&t.env);
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    // 3 invoices with different discount rates: 200, 400, 600 bps
    // Expected running average: (200 + 400 + 600) / 3 = 400
    let rates: [u32; 3] = [200, 400, 600];
    let payers = [payer1, payer2, payer3];

    for (rate, payer) in rates.iter().zip(payers.iter()) {
        let id = t.contract.submit_invoice(
            &freelancer,
            &payer,
            &INVOICE_AMOUNT,
            &due_date,
            rate,
            &t.token.address,
            &None,
        );
        t.contract.fund_invoice(&lp, &id, &INVOICE_AMOUNT, &false);
    }

    let stats = t.contract.get_lp_portfolio_stats(&lp);

    assert_eq!(stats.total_positions, 3);
    assert_eq!(stats.active_positions, 3);
    assert_eq!(stats.total_funded, INVOICE_AMOUNT * 3);
    // Integer average: (0*0 + 200)/1 = 200; then (200+400)/2 = 300; then (300*2+600)/3 = 400
    assert_eq!(stats.avg_yield_bps, 400, "avg_yield_bps should equal (200+400+600)/3 = 400");
}

// ---------------------------------------------------------------------------
// 5. Partial funding by same LP on same invoice — no double-counting
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_partial_topup_no_duplicate_position() {
    let t = setup();
    let token_admin = StellarAssetClient::new(&t.env, &t.token.address);

    let lp = Address::generate(&t.env);
    token_admin.mint(&lp, &(INVOICE_AMOUNT * 2));

    let freelancer = Address::generate(&t.env);
    let payer = Address::generate(&t.env);
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let id = t.contract.submit_invoice(
        &freelancer,
        &payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
        &None,
    );

    // Two partial fund calls by the same LP
    let half = INVOICE_AMOUNT / 2;
    t.contract.fund_invoice(&lp, &id, &half, &false);
    t.contract.fund_invoice(&lp, &id, &half, &false);

    let stats = t.contract.get_lp_portfolio_stats(&lp);

    // The second call is a top-up — position count must NOT double
    assert_eq!(stats.total_positions, 1, "a top-up must not add a second position");
    assert_eq!(stats.active_positions, 1);
    // But total_funded should accumulate both transfers
    assert_eq!(stats.total_funded, INVOICE_AMOUNT, "total_funded should be full amount");
}

// ---------------------------------------------------------------------------
// 6. Two different LPs funding the same invoice — independent stats
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_two_lps_independent() {
    let t = setup();
    let token_admin = StellarAssetClient::new(&t.env, &t.token.address);

    let lp1 = Address::generate(&t.env);
    let lp2 = Address::generate(&t.env);
    token_admin.mint(&lp1, &(INVOICE_AMOUNT * 2));
    token_admin.mint(&lp2, &(INVOICE_AMOUNT * 2));

    let payer = Address::generate(&t.env);
    token_admin.mint(&payer, &(INVOICE_AMOUNT * 2));

    let freelancer = Address::generate(&t.env);
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let id = t.contract.submit_invoice(
        &freelancer,
        &payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
        &None,
    );

    let half = INVOICE_AMOUNT / 2;
    t.contract.fund_invoice(&lp1, &id, &half, &false);
    t.contract.fund_invoice(&lp2, &id, &half, &false);

    let stats1 = t.contract.get_lp_portfolio_stats(&lp1);
    let stats2 = t.contract.get_lp_portfolio_stats(&lp2);

    assert_eq!(stats1.total_positions, 1);
    assert_eq!(stats2.total_positions, 1);
    assert_eq!(stats1.total_funded, half);
    assert_eq!(stats2.total_funded, half);
    assert_eq!(stats1.active_positions, 1);
    assert_eq!(stats2.active_positions, 1);

    // After payment both LPs should see earned > 0 and active_positions = 0
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let stats1_after = t.contract.get_lp_portfolio_stats(&lp1);
    let stats2_after = t.contract.get_lp_portfolio_stats(&lp2);

    assert_eq!(stats1_after.active_positions, 0);
    assert_eq!(stats2_after.active_positions, 0);
    assert!(stats1_after.total_earned > 0, "lp1 should have earnings");
    assert!(stats2_after.total_earned > 0, "lp2 should have earnings");
}

// ---------------------------------------------------------------------------
// 7. Cumulative totals across multiple paid invoices
// ---------------------------------------------------------------------------

#[test]
fn test_lp_stats_cumulative_across_paid_invoices() {
    let t = setup();
    let token_admin = StellarAssetClient::new(&t.env, &t.token.address);

    let lp = Address::generate(&t.env);
    token_admin.mint(&lp, &(INVOICE_AMOUNT * 10));

    let freelancer = Address::generate(&t.env);
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let expected_yield_per_invoice = INVOICE_AMOUNT * DISCOUNT_RATE as i128 / 10_000;

    for _ in 0..3 {
        let payer = Address::generate(&t.env);
        token_admin.mint(&payer, &(INVOICE_AMOUNT * 2));

        let id = t.contract.submit_invoice(
            &freelancer,
            &payer,
            &INVOICE_AMOUNT,
            &due_date,
            &DISCOUNT_RATE,
            &t.token.address,
            &None,
        );
        t.contract.fund_invoice(&lp, &id, &INVOICE_AMOUNT, &false);
        t.contract.mark_paid(&id, &INVOICE_AMOUNT);
    }

    let stats = t.contract.get_lp_portfolio_stats(&lp);

    assert_eq!(stats.total_positions, 3);
    assert_eq!(stats.active_positions, 0);
    assert_eq!(stats.total_funded, INVOICE_AMOUNT * 3);
    assert_eq!(
        stats.total_earned,
        expected_yield_per_invoice * 3,
        "total_earned should be sum of yields across all paid invoices"
    );
}
