#![cfg(test)]

use crate::invoice::InvoiceStatus;
use crate::test::setup;

#[test]
fn test_total_count() {
    let t = setup();

    // Initial count is 0
    assert_eq!(t.contract.get_invoice_count(&None), 0);

    let due_date = t.env.ledger().timestamp() + 86400;

    // Submit 1
    let id1 = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &1_000_000_000,
        &due_date,
        &300,
        &t.token.address,
        &None,
    );
    assert_eq!(t.contract.get_invoice_count(&None), 1);

    // Submit 2
    let id2 = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &2_000_000_000,
        &due_date,
        &300,
        &t.token.address,
        &None,
    );
    assert_eq!(t.contract.get_invoice_count(&None), 2);
}

#[test]
fn test_per_state_counts() {
    let t = setup();

    let due_date = t.env.ledger().timestamp() + 86400;

    // Initially 0 for Pending
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 0);

    // Submit an invoice -> Pending
    let id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &1_000_000_000,
        &due_date,
        &300,
        &t.token.address,
        &None,
    );

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 1);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 0);

    // Fund invoice -> Funded
    t.contract.fund_invoice(&t.funder, &id, &1_000_000_000, &false);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 0);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 1);
}

#[test]
fn test_state_transitions() {
    let t = setup();

    let due_date = t.env.ledger().timestamp() + 86400;

    let id = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &1_000_000_000,
        &due_date,
        &300,
        &t.token.address,
        &None,
    );

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 1);

    t.contract.fund_invoice(&t.funder, &id, &1_000_000_000, &false);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 0);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 1);

    // Paid
    t.contract.mark_paid(&id, &1_000_000_000);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 0);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Paid)), 1);
}

#[test]
fn test_multiple_sequential_transitions() {
    let t = setup();

    let due_date = t.env.ledger().timestamp() + 86400;

    let id1 = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &1_000_000_000,
        &due_date,
        &300,
        &t.token.address,
        &None,
    );
    let id2 = t.contract.submit_invoice(
        &t.freelancer,
        &t.payer,
        &2_000_000_000,
        &due_date,
        &300,
        &t.token.address,
        &None,
    );

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 2);

    // Fund one invoice
    t.contract.fund_invoice(&t.funder, &id1, &1_000_000_000, &false);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 1);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 1);

    // Fund the second invoice
    // We need another funder with balance, but t.funder has 10_000_000_000 so it can fund both
    t.contract.fund_invoice(&t.funder, &id2, &2_000_000_000, &false);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Pending)), 0);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 2);

    // Pay first invoice
    t.contract.mark_paid(&id1, &1_000_000_000);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 1);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Paid)), 1);
    
    // Pay second invoice
    // Make sure t.payer has enough balance
    // In setup, payer gets INVOICE_AMOUNT * 10 = 10_000_000_000, so they have enough for id2 (2_000_000_000).
    t.contract.mark_paid(&id2, &2_000_000_000);

    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Funded)), 0);
    assert_eq!(t.contract.get_invoice_count(&Some(InvoiceStatus::Paid)), 2);
    
    // Total count should remain 2
    assert_eq!(t.contract.get_invoice_count(&None), 2);
}
