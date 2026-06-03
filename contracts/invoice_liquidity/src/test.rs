#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, Vec, IntoVal, Event,
    token::{StellarAssetClient, Client as TokenClient},
    testutils::{Address as _, Events as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, Event, BytesN,
};

pub(crate) struct TestContext<'a> {
    pub(crate) env: Env,
    pub(crate) admin: Address,
    pub(crate) freelancer: Address,
    pub(crate) payer: Address,
    pub(crate) funder: Address,
    pub(crate) token: Address,
    pub(crate) xlm_token: Address,
    pub(crate) contract_id: Address,
    pub(crate) contract: InvoiceLiquidityContractClient<'a>,
}

pub(crate) fn setup() -> TestContext<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    // ---- Deploy a mock USDC token contract ----
    let usdc_admin = Address::generate(&env);
    let usdc_contract_id = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_address = usdc_contract_id.address();

    let eurc_admin = Address::generate(&env);
    let eurc_contract_id = env.register_stellar_asset_contract_v2(eurc_admin.clone());
    let eurc_address = eurc_contract_id.address();

    let token = TokenClient::new(&env, &usdc_address);
    let token_admin = StellarAssetClient::new(&env, &usdc_address);

    // ---- Generate test wallets ----
    let freelancer = Address::generate(&env);
    let payer = Address::generate(&env);
    let funder = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = token_id.address();
    
    let xlm_admin = Address::generate(&env);
    let xlm_id = env.register_stellar_asset_contract_v2(xlm_admin.clone());
    let xlm_token = xlm_id.address();

    // Mint some tokens to funder and payer
    let token_client = StellarAssetClient::new(&env, &token);
    token_client.mint(&funder, &1_000_000_000_000);
    token_client.mint(&payer, &1_000_000_000_000);

    let xlm_client = StellarAssetClient::new(&env, &xlm_token);
    xlm_client.mint(&funder, &1_000_000_000_000);
    xlm_client.mint(&payer, &1_000_000_000_000);

    let contract_id = env.register(InvoiceLiquidityContract, ());
    let client = InvoiceLiquidityContractClient::new(&env, &contract_id);

    client.initialize(&admin, &token, &xlm_token);
    // Initialize with mock USDC, EURC and mock XLM SAC addresses
    contract.initialize(&usdc_admin, &usdc_address, &eurc_address, &xlm_address);

    // Setup initial ledger time
    let mut info = env.ledger().get();
    info.timestamp = 1_000_000;
    env.ledger().set(info);

    TestContext {
        env,
        admin,
        freelancer,
        payer,
        funder,
        token,
        xlm_token,
        contract_id,
        contract: client,
    }
}

const INVOICE_AMOUNT: i128 = 10_000_000;
const DISCOUNT_RATE: u32 = 300;
const DUE_DATE_OFFSET: u64 = 86400 * 30;

fn submit_standard_invoice(t: &TestContext) -> u64 {
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    )
}

#[test]
fn test_submit_invoice_happy_path() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    let id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    assert_eq!(id, 1);
    let invoice = t.contract.get_invoice(&id);

    assert_eq!(invoice.id, id);
    assert_eq!(invoice.freelancer, t.freelancer);
    assert_eq!(invoice.payer, t.payer);
    assert_eq!(invoice.token, t.token);
    assert_eq!(invoice.amount, INVOICE_AMOUNT);
    assert_eq!(u64::from(invoice.due_date), due_date);
    assert_eq!(invoice.discount_rate, DISCOUNT_RATE);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
}

#[test]
fn test_get_invoice_returns_existing_invoice() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    let id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    let invoice = t.contract.get_invoice(&id);

    assert_eq!(invoice.id, id);
    assert_eq!(invoice.freelancer, t.freelancer);
    assert_eq!(invoice.payer, t.payer);
    assert_eq!(invoice.token, t.token);
    assert_eq!(invoice.amount, INVOICE_AMOUNT);
    assert_eq!(u64::from(invoice.due_date), due_date);
    assert_eq!(invoice.discount_rate, DISCOUNT_RATE);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
    assert_eq!(invoice.amount_funded, 0);
    assert!(invoice.funder.is_none());
    assert!(invoice.funded_at.is_none());
}

#[test]
fn test_submitter_reputation_snapshot_at_submission() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    // Default reputation for a new freelancer should be 50
    let id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &DISCOUNT_RATE,
        &t.token,
        &t.token.address,
        &Option::<BytesN<32>>::None,
    );

    let invoice = t.contract.get_invoice(&id);

    // Verify that the submitter_reputation matches the freelancer's reputation at submission
    // For a new freelancer, this should be the default value of 50
    assert_eq!(invoice.submitter_reputation, 50);
    assert_eq!(invoice.freelancer, t.freelancer);
}

#[test]
fn test_get_invoice_returns_invoice_not_found_for_missing_id() {
    let t = setup();

    let result = t.contract.try_get_invoice(&999);

    assert_eq!(result, Err(Ok(ContractError::InvoiceNotFound)));
}

#[test]
fn test_submit_multiple_invoices_increment_ids() {
    let t = setup();

    let id1 = submit_standard_invoice(&t);
    let id2 = submit_standard_invoice(&t);
    let id3 = submit_standard_invoice(&t);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

// ----------------------------------------------------------------
// submit_invoices_batch
// ----------------------------------------------------------------

/// Build a fully-populated, valid `InvoiceParams` for batch tests (Issue #120).
fn batch_params(t: &TestEnv, due_date: u64) -> InvoiceParams {
    InvoiceParams {
        freelancer: t.freelancer.clone(),
        payer: t.payer.clone(),
        amount: INVOICE_AMOUNT,
        due_date,
        discount_rate: DISCOUNT_RATE,
        token: t.token.address.clone(),
        referral_code: None,
        allowed_lps: None,
    }
}

#[test]
fn test_submit_invoices_batch_happy_path() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    
    let params = batch_params(&t, due_date);

    let mut batch = Vec::new(&t.env);
    for _ in 0..3 {
        batch.push_back(InvoiceParams {
            freelancer: t.freelancer.clone(),
            payer: t.payer.clone(),
            amount: INVOICE_AMOUNT,
            due_date,
            discount_rate: DISCOUNT_RATE,
            token: t.token.clone(),
        });
    }

    let ids = t.contract.submit_invoices_batch(&batch);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), 1);
    assert_eq!(ids.get(1).unwrap(), 2);
    assert_eq!(ids.get(2).unwrap(), 3);

    let stats = t.contract.get_contract_stats();
    assert_eq!(stats.total_invoices, 3);
}

#[test]
fn test_update_invoice_happy_path() {
    assert_eq!(t.contract.get_invoice_count(&None), 3);
}

#[test]
fn test_submit_invoices_batch_max_size_succeeds() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    let params = batch_params(&t, due_date);

    // Exactly MAX_BATCH_SIZE (50) invoices must succeed.
    let mut batch = Vec::new(&t.env);
    for _ in 0..50 {
        batch.push_back(params.clone());
    }

    let ids = t.contract.submit_invoices_batch(&batch);
    assert_eq!(ids.len(), 50);
    assert_eq!(ids.get(0).unwrap(), 1);
    assert_eq!(ids.get(49).unwrap(), 50);
    assert_eq!(t.contract.get_invoice_count(&None), 50);
}

#[test]
fn test_submit_invoices_batch_rejects_over_limit() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;
    let params = batch_params(&t, due_date);

    // One past MAX_BATCH_SIZE (51) must be rejected.
    let mut batch = Vec::new(&t.env);
    for _ in 0..51 {
        batch.push_back(params.clone());
    }

    let result = t.contract.try_submit_invoices_batch(&batch);
    assert_eq!(result, Err(Ok(ContractError::BatchTooLarge)));

    // Nothing was created.
    assert_eq!(t.contract.get_invoice_count(&None), 0);
}

#[test]
fn test_submit_invoices_batch_atomicity_fail() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let mut batch = Vec::new(&t.env);

    // Valid invoice...
    batch.push_back(batch_params(&t, due_date));

    // ...followed by an invalid one (amount = 0) -> whole batch must revert.
    let mut bad = batch_params(&t, due_date);
    bad.amount = 0;
    batch.push_back(bad);

    let result = t.contract.try_submit_invoices_batch(&batch);
    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));

    // Atomicity: the valid invoice before the failure was NOT persisted.
    assert_eq!(t.contract.get_invoice_count(&None), 0);
}

// ----------------------------------------------------------------
// submit_invoice — validation errors
// ----------------------------------------------------------------

#[test]
fn test_submit_rejects_zero_amount() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice(
        &t.freelancer,
        &t.payer,
        &0,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
}

#[test]
fn test_submit_rejects_negative_amount() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice(
        &t.freelancer,
        &t.payer,
        &-1,
        &due_date,
        &DISCOUNT_RATE,
        &t.token.address,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
}

#[test]
fn test_submit_rejects_past_due_date() {
    let t = setup();
    let past_due_date = t.env.ledger().timestamp() - 1; // 1 second in the past

    let result = t.contract.try_submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &past_due_date,
        &DISCOUNT_RATE,
        &t.token.address,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidDueDate)));
}

#[test]
fn test_submit_rejects_zero_discount_rate() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &0,
        &t.token.address,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidDiscountRate)));
}

#[test]
fn test_submit_rejects_discount_rate_above_50_percent() {
    let t = setup();
    let due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET;

    let result = t.contract.try_submit_invoice(
        &t.freelancer,
        &t.payer,
        &INVOICE_AMOUNT,
        &due_date,
        &5_001, // 50.01% — just over the cap
        &t.token.address,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidDiscountRate)));
}

// ----------------------------------------------------------------
// update_invoice
// ----------------------------------------------------------------

#[test]
fn test_update_invoice_updates_pending_invoice_fields() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let updated_amount = INVOICE_AMOUNT + 5_000_000;
    let updated_due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET * 2;
    let updated_discount_rate = DISCOUNT_RATE + 100;

    t.contract.update_invoice(
        &t.freelancer,
        &id,
        &updated_amount,
        &updated_due_date,
        &updated_discount_rate,
    );

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.amount, updated_amount);
    assert_eq!(u64::from(invoice.due_date), updated_due_date);
    assert_eq!(invoice.discount_rate, updated_discount_rate);
    assert_eq!(invoice.payer, t.payer);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
}

#[test]
fn test_update_invoice_emits_updated_event() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let updated_amount = INVOICE_AMOUNT + 5_000_000;
    let updated_due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET * 2;
    let updated_discount_rate = DISCOUNT_RATE + 100;

    t.contract.update_invoice(
        &t.freelancer,
        &id,
        &updated_amount,
        &updated_due_date,
        &updated_discount_rate,
    );

    let expected_event = InvoiceUpdated {
        invoice_id: id,
        freelancer: t.freelancer.clone(),
        payer: t.payer.clone(),
        token: t.token.clone(),
        amount: updated_amount,
        due_date: updated_due_date,
        discount_rate: updated_discount_rate,
        status: InvoiceStatus::Pending,
    };

    let events = t.env.events().all().filter_by_contract(&t.contract_id);
    assert_eq!(
        events.events().last(),
        Some(&expected_event.to_xdr(&t.env, &t.contract_id))
    );
}

#[test]
fn test_update_invoice_rejects_non_freelancer() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let impostor = Address::generate(&t.env);
    let updated_due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET * 2;

    let result = t.contract.try_update_invoice(
        &impostor,
        &id,
        &INVOICE_AMOUNT,
        &updated_due_date,
        &DISCOUNT_RATE,
    );

    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_update_funded_invoice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let updated_due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET * 2;

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let result = t.contract.try_update_invoice(
        &t.freelancer,
        &id,
        &INVOICE_AMOUNT,
        &updated_due_date,
        &DISCOUNT_RATE,
    );

    assert_eq!(result, Err(Ok(ContractError::AlreadyFunded)));
}

#[test]
fn test_update_invoice_rejects_invalid_amount() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let updated_due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET * 2;

    let result =
        t.contract
            .try_update_invoice(&t.freelancer, &id, &0, &updated_due_date, &DISCOUNT_RATE);

    assert_eq!(result, Err(Ok(ContractError::InvalidAmount)));
}

#[test]
fn test_update_invoice_rejects_invalid_due_date() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let past_due_date = t.env.ledger().timestamp();

    let result = t.contract.try_update_invoice(
        &t.freelancer,
        &id,
        &INVOICE_AMOUNT,
        &past_due_date,
        &DISCOUNT_RATE,
    );

    assert_eq!(result, Err(Ok(ContractError::InvalidDueDate)));
}

#[test]
fn test_update_invoice_rejects_invalid_discount_rate() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let updated_due_date = t.env.ledger().timestamp() + DUE_DATE_OFFSET * 2;

    let result =
        t.contract
            .try_update_invoice(&t.freelancer, &id, &INVOICE_AMOUNT, &updated_due_date, &0);

    assert_eq!(result, Err(Ok(ContractError::InvalidDiscountRate)));
}

#[test]
fn test_transfer_invoice_updates_freelancer() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    let new_freelancer = Address::generate(&t.env);

    t.contract.transfer_invoice(&id, &new_freelancer);

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.freelancer, new_freelancer);
}

#[test]
fn test_transfer_nonexistent_invoice_fails() {
    let t = setup();
    let new_freelancer = Address::generate(&t.env);

    let result = t.contract.try_transfer_invoice(&999, &new_freelancer);
    assert_eq!(result, Err(Ok(ContractError::InvoiceNotFound)));
}

#[test]
fn test_transfer_funded_invoice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let new_freelancer = Address::generate(&t.env);
    let result = t.contract.try_transfer_invoice(&id, &new_freelancer);
    assert_eq!(result, Err(Ok(ContractError::AlreadyFunded)));
}

#[test]
fn test_transfer_lp_position_updates_funder_and_lp_index() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT);

    let new_lp = Address::generate(&t.env);
    t.contract.transfer_lp_position(&id, &new_lp);

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.funder, Some(new_lp.clone()));

    let old_lp_invoices = t.contract.list_invoices_by_lp(&t.funder, &0, &50);
    assert!(!old_lp_invoices.iter().any(|invoice| invoice.id == id));

    let new_lp_invoices = t.contract.list_invoices_by_lp(&new_lp, &0, &50);
    assert!(new_lp_invoices.iter().any(|invoice| invoice.id == id));
}

#[test]
fn test_transfer_lp_position_pays_new_lp_on_settlement() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT);

    let new_lp = Address::generate(&t.env);
    t.contract.transfer_lp_position(&id, &new_lp);

    let old_lp_balance_before = t.token.balance(&t.funder);
    let new_lp_balance_before = t.token.balance(&new_lp);

    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let old_lp_balance_after = t.token.balance(&t.funder);
    let new_lp_balance_after = t.token.balance(&new_lp);

    assert_eq!(old_lp_balance_after, old_lp_balance_before);
    assert_eq!(new_lp_balance_after - new_lp_balance_before, INVOICE_AMOUNT);
}

#[test]
fn test_transfer_lp_position_can_transfer_twice() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT);

    let new_lp = Address::generate(&t.env);
    let second_lp = Address::generate(&t.env);

    t.contract.transfer_lp_position(&id, &new_lp);
    t.contract.transfer_lp_position(&id, &second_lp);

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.funder, Some(second_lp.clone()));
    let invoices = t.contract.list_invoices_by_lp(&second_lp, &0, &50);
    assert!(invoices.iter().any(|invoice| invoice.id == id));
}

// ----------------------------------------------------------------
// fund_invoice — happy path
// ----------------------------------------------------------------

#[test]
fn test_fund_invoice_transfers_correct_amounts() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    let token = TokenClient::new(&t.env, &t.token);
    let funder_balance_before = token.balance(&t.funder);
    let freelancer_balance_before = token.balance(&t.freelancer);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let funder_balance_after = token.balance(&t.funder);
    let freelancer_balance_after = token.balance(&t.freelancer);

    let discount_amount = INVOICE_AMOUNT * DISCOUNT_RATE as i128 / 10_000;
    let freelancer_payout = INVOICE_AMOUNT - discount_amount;

    assert_eq!(
        funder_balance_before - funder_balance_after,
        freelancer_payout,
        "LP should have sent the cost amount"
    );

    assert_eq!(
        freelancer_balance_after - freelancer_balance_before,
        freelancer_payout,
        "Freelancer should receive amount minus discount"
    );
}

#[test]
fn test_fund_invoice_updates_status_to_funded() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let invoice = t.contract.get_invoice(&id);

    assert_eq!(invoice.status, InvoiceStatus::Funded);
    assert_eq!(invoice.funder, Some(t.funder.clone()));
    assert!(invoice.funded_at.is_some());
}

#[test]
fn test_fund_invoice_sets_funded_at_timestamp() {
    let t = setup();
    let id = submit_standard_invoice(&t);
    let now = t.env.ledger().timestamp();

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.funded_at, Some(now.try_into().expect("timestamp")));
}

#[test]
fn test_fund_nonexistent_invoice_fails() {
    let t = setup();

    let result = t
        .contract
        .try_fund_invoice(&t.funder, &999, &INVOICE_AMOUNT);
    assert_eq!(result, Err(Ok(ContractError::InvoiceNotFound)));
}

#[test]
fn test_fund_already_funded_invoice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let second_funder = Address::generate(&t.env);
    let result = t
        .contract
        .try_fund_invoice(&second_funder, &id, &INVOICE_AMOUNT);

    assert_eq!(result, Err(Ok(ContractError::AlreadyFunded)));
}

#[test]
fn test_mark_paid_releases_full_amount_to_lp() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let token = TokenClient::new(&t.env, &t.token);
    let funder_balance_before = token.balance(&t.funder);

    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let funder_balance_after = token.balance(&t.funder);

    assert_eq!(
        funder_balance_after - funder_balance_before,
        INVOICE_AMOUNT,
        "LP should receive the full invoice amount when invoice is paid"
    );
}

#[test]
fn test_mark_paid_updates_status() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Paid);
}

#[test]
fn test_full_lifecycle_lp_earns_correct_yield() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    let token = TokenClient::new(&t.env, &t.token);
    let lp_start = token.balance(&t.funder);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT);
    // LP funds the invoice
    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    // Payer settles
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let lp_end = token.balance(&t.funder);

    let expected_yield = INVOICE_AMOUNT * DISCOUNT_RATE as i128 / 10_000;

    assert_eq!(
        lp_end - lp_start,
        expected_yield,
        "LP net yield should equal the discount amount"
    );
}

#[test]
fn test_full_lifecycle_payer_balance_reduces_correctly() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    let token = TokenClient::new(&t.env, &t.token);
    let payer_start = token.balance(&t.payer);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let payer_end = token.balance(&t.payer);

    assert_eq!(
        payer_start - payer_end,
        INVOICE_AMOUNT,
        "Payer should have paid the full invoice amount"
    );
}

#[test]
fn test_fund_invoice_full_lifecycle_with_stats() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT);
    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Funded);

    t.contract.mark_paid(&id, &INVOICE_AMOUNT);
    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Paid);
    
    let stats = t.contract.get_contract_stats();
    assert_eq!(stats.total_paid, 1);
    assert_eq!(stats.total_funded, 1);
}

#[test]
fn test_mark_paid_on_pending_invoice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    let result = t.contract.try_mark_paid(&id, &INVOICE_AMOUNT);
    assert_eq!(result, Err(Ok(ContractError::NotFunded)));
}

#[test]
fn test_mark_paid_twice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let result = t.contract.try_mark_paid(&id, &INVOICE_AMOUNT);
    assert_eq!(result, Err(Ok(ContractError::AlreadyPaid)));
}

#[test]
fn test_mark_paid_nonexistent_invoice_fails() {
    let t = setup();

    let result = t.contract.try_mark_paid(&999, &INVOICE_AMOUNT);
    assert_eq!(result, Err(Ok(ContractError::InvoiceNotFound)));
}

#[test]
fn test_claim_default_success() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    // Move time forward past due date
    let mut ledger = t.env.ledger().get();
    ledger.timestamp += DUE_DATE_OFFSET + 1;
    t.env.ledger().set(ledger);

    let token = TokenClient::new(&t.env, &t.token);
    let token_admin = StellarAssetClient::new(&t.env, &t.token);
    token_admin.mint(&t.contract_id, &INVOICE_AMOUNT);
    let funder_before = token.balance(&t.funder);

    t.contract.claim_default(&t.funder, &id);

    let funder_after = token.balance(&t.funder);

    let discount_amount = INVOICE_AMOUNT * DISCOUNT_RATE as i128 / 10_000;

    assert_eq!(
        funder_after - funder_before,
        INVOICE_AMOUNT - discount_amount,
        "LP should recover their contributed principal after default"
    );

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Defaulted);
}

#[test]
fn test_claim_default_before_due_date_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let result = t.contract.try_claim_default(&t.funder, &id);
    assert_eq!(result, Err(Ok(ContractError::NotYetDefaulted)));
}

#[test]
fn test_claim_default_non_funder_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let mut ledger = t.env.ledger().get();
    ledger.timestamp += DUE_DATE_OFFSET + 1;
    t.env.ledger().set(ledger);

    let attacker = Address::generate(&t.env);

    let result = t.contract.try_claim_default(&attacker, &id);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_claim_default_on_paid_invoice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let mut ledger = t.env.ledger().get();
    ledger.timestamp += DUE_DATE_OFFSET + 1;
    t.env.ledger().set(ledger);

    let result = t.contract.try_claim_default(&t.funder, &id);
    assert_eq!(result, Err(Ok(ContractError::AlreadyPaid)));
}

#[test]
fn test_claim_default_twice_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let mut ledger = t.env.ledger().get();
    ledger.timestamp += DUE_DATE_OFFSET + 1;
    t.env.ledger().set(ledger);

    let token = TokenClient::new(&t.env, &t.token);
    let token_admin = StellarAssetClient::new(&t.env, &t.token);
    token_admin.mint(&t.contract_id, &INVOICE_AMOUNT);

    t.contract.claim_default(&t.funder, &id);

    let result = t.contract.try_claim_default(&t.funder, &id);
    assert_eq!(result, Err(Ok(ContractError::InvoiceDefaulted)));
}

#[test]
fn test_expire_invoice_success() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    // Move time forward past due date
    let mut ledger = t.env.ledger().get();
    ledger.timestamp += DUE_DATE_OFFSET + 1;
    t.env.ledger().set(ledger);

    t.contract.expire_invoice(&id);

    let invoice = t.contract.get_invoice(&id);
    assert_eq!(invoice.status, InvoiceStatus::Expired);
}

#[test]
fn test_expire_invoice_before_due_date_fails() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    let result = t.contract.try_expire_invoice(&id);
    assert_eq!(result, Err(Ok(ContractError::NotYetDefaulted)));
}

#[test]
fn test_new_payer_score_is_neutral() {
    let t = setup();

    let score = t.contract.payer_score(&t.payer);

    assert_eq!(score, 50);
}

#[test]
fn test_perfect_payer_score() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);
    t.contract.mark_paid(&id, &INVOICE_AMOUNT);

    let score = t.contract.payer_score(&t.payer);

    assert_eq!(score, 51);
}

#[test]
fn test_payer_with_default() {
    let t = setup();
    let id = submit_standard_invoice(&t);

    t.contract.fund_invoice(&t.funder, &id, &INVOICE_AMOUNT, &false);

    let mut ledger = t.env.ledger().get();
    ledger.timestamp += DUE_DATE_OFFSET + 1;
    t.env.ledger().set(ledger);

    let token_admin = StellarAssetClient::new(&t.env, &t.token);
    token_admin.mint(&t.contract_id, &INVOICE_AMOUNT);

    t.contract.claim_default(&t.funder, &id);

    let score = t.contract.payer_score(&t.payer);

    assert!(score < 50);
}

// ----------------------------------------------------------------
// Reputation decay tests
// ----------------------------------------------------------------

#[test]
#[ignore]
fn test_reputation_decay_inactive_score() {
    let t = setup();

    t.env.as_contract(&t.contract_id, || {
        set_payer_score(&t.env, &t.payer, 80);
    });

    let config = Config {
        high_rep_threshold: 80,
        bonus_bps: 200,
        min_discount_rate_bps: 100,
        decay_rate_bps: 100, // 1% per period
        decay_period_ledgers: 1000,
        dispute_timeout_ledgers: 100,
        xlm_sac_address: t.xlm_token.clone(),
        price_oracle: None,
    };
    t.env.as_contract(&t.contract_id, || {
        set_config(&t.env, &config);
    });

    let mut ledger = t.env.ledger().get();
    ledger.sequence_number += 2100;
    t.env.ledger().set(ledger);

    let score = t.contract.payer_score(&t.payer);

    assert!(score < 80, "Score should decay from 80, got {}", score);
    assert!(score >= 78, "Score should decay to ~78, got {}", score);
}

#[test]
#[ignore]
fn test_reputation_no_decay_when_inactive() {
    let t = setup();

    t.env.as_contract(&t.contract_id, || {
        set_payer_score(&t.env, &t.payer, 80);
    });

    let config = Config {
        high_rep_threshold: 80,
        bonus_bps: 200,
        min_discount_rate_bps: 100,
        decay_rate_bps: 100,
        decay_period_ledgers: 10_000_000, // Very long period
        dispute_timeout_ledgers: 100,
        xlm_sac_address: t.xlm_token.clone(),
        price_oracle: None,
    };
    t.env.as_contract(&t.contract_id, || {
        set_config(&t.env, &config);
    });

    let mut ledger = t.env.ledger().get();
    ledger.sequence_number += 1000;
    t.env.ledger().set(ledger);

    let score = t.contract.payer_score(&t.payer);

    assert_eq!(score, 80, "Score should not decay when period not reached");
}

#[test]
#[ignore]
fn test_reputation_decay_activity_resets() {
    let t = setup();

    t.env.as_contract(&t.contract_id, || {
        set_payer_score(&t.env, &t.payer, 80);
    });

    let config = Config {
        high_rep_threshold: 80,
        bonus_bps: 200,
        min_discount_rate_bps: 100,
        decay_rate_bps: 100,
        decay_period_ledgers: 1000,
        dispute_timeout_ledgers: 100,
        xlm_sac_address: t.xlm_token.clone(),
        price_oracle: None,
    };

    t.env.as_contract(&t.contract_id, || {
        set_config(&t.env, &config);
    });

    let mut ledger = t.env.ledger().get();
    ledger.sequence_number += 500;
    t.env.ledger().set(ledger);

    t.env.as_contract(&t.contract_id, || {
        set_payer_score(&t.env, &t.payer, 85);
    });

    ledger = t.env.ledger().get();
    ledger.sequence_number += 500;
    t.env.ledger().set(ledger);

    let score = t.contract.payer_score(&t.payer);

    assert_eq!(score, 85, "Score should not decay shortly after activity");
}

#[test]
#[ignore]
fn test_reputation_score_never_goes_below_zero() {
    let t = setup();

    t.env.as_contract(&t.contract_id, || {
        set_payer_score(&t.env, &t.payer, 5);
    });

    let config = Config {
        high_rep_threshold: 80,
        bonus_bps: 200,
        min_discount_rate_bps: 100,
        decay_rate_bps: 5000, // Very aggressive decay: 50% per period
        decay_period_ledgers: 100,
        dispute_timeout_ledgers: 100,
        xlm_sac_address: t.xlm_token.clone(),
        price_oracle: None,
    };
    t.env.as_contract(&t.contract_id, || {
        set_config(&t.env, &config);
    });

    let mut ledger = t.env.ledger().get();
    ledger.sequence_number += 1000;
    t.env.ledger().set(ledger);

    let score = t.contract.payer_score(&t.payer);

    assert_eq!(score, 0, "Score should floor at 0, not go negative");
}

#[test]
fn test_reputation_score_never_exceeds_100() {
    let t = setup();

    t.env.as_contract(&t.contract_id, || {
        set_payer_score(&t.env, &t.payer, 150);
    });

    let score = t.contract.payer_score(&t.payer);

    assert_eq!(score, 100, "Score should be capped at 100");
}

#[test]
fn test_upgrade_emits_correct_event() {
    let t = setup();

    let wasm_hash = soroban_sdk::BytesN::from_array(&t.env, &[1u8; 32]);

    t.contract.upgrade(&wasm_hash);

    let events = t.env.events().all().filter_by_contract(&t.contract_id);

    let expected_event = ContractUpgraded {
        admin: t.admin.clone(),
        new_wasm_hash: wasm_hash,
        timestamp: t.env.ledger().timestamp(),
    };

    assert_eq!(
        events.events().last(),
        Some(&expected_event.to_xdr(&t.env, &t.contract_id)),
        "ContractUpgraded event should be emitted"
    );
}

#[test]
fn test_upgrade_does_not_affect_existing_invoices() {
    let t = setup();

    let id = submit_standard_invoice(&t);
    let invoice_before = t.contract.get_invoice(&id);

    let wasm_hash = soroban_sdk::BytesN::from_array(&t.env, &[3u8; 32]);
    t.contract.upgrade(&wasm_hash);

    let invoice_after = t.contract.get_invoice(&id);

    assert_eq!(
        invoice_before.id, invoice_after.id,
        "Invoice ID should be preserved"
    );
    assert_eq!(
        invoice_before.freelancer, invoice_after.freelancer,
        "Freelancer address should be preserved"
    );
    assert_eq!(
        invoice_before.payer, invoice_after.payer,
        "Payer address should be preserved"
    );
    assert_eq!(
        invoice_before.amount, invoice_after.amount,
        "Amount should be preserved"
    );
    assert_eq!(
        invoice_before.status, invoice_after.status,
        "Status should be preserved"
    );
}

#[test]
fn test_upgrade_snapshot_before_after() {
    let t = setup();

    let _id1 = submit_standard_invoice(&t);
    let stats_with_data = t.contract.get_contract_stats();

    let wasm_hash = soroban_sdk::BytesN::from_array(&t.env, &[4u8; 32]);
    t.contract.upgrade(&wasm_hash);

    let stats_after = t.contract.get_contract_stats();

    assert_eq!(
        stats_with_data.total_invoices, stats_after.total_invoices,
        "Total invoices should be preserved after upgrade"
    );
}
