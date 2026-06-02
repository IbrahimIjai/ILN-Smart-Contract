#![cfg(test)]

//! Lazy storage initialisation tests (Issue #78).

use super::*;
use crate::test::setup;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

fn has_persistent_key(env: &soroban_sdk::Env, contract: &Address, key: &StorageKey) -> bool {
    env.as_contract(contract, || env.storage().persistent().has(key))
}

#[test]
fn test_payer_score_read_does_not_create_storage() {
    let t = setup();
    let unknown = Address::generate(&t.env);
    let key = StorageKey::PayerScore(unknown.clone());

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
    assert_eq!(t.contract.payer_score(&unknown), 50);
    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
}

#[test]
fn test_lp_score_read_does_not_create_storage() {
    let t = setup();
    let unknown = Address::generate(&t.env);
    let key = StorageKey::LpScore(unknown.clone());

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
    assert_eq!(t.contract.lp_score(&unknown), 50);
    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
}

#[test]
fn test_reputation_read_does_not_create_storage() {
    let t = setup();
    let unknown = Address::generate(&t.env);
    let key = StorageKey::Reputation(unknown.clone());

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
    let profile = t.contract.get_reputation(&unknown);
    assert_eq!(profile.invoices_submitted, 0);
    assert_eq!(profile.score, 0);
    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
}

#[test]
fn test_payer_score_write_on_non_default_value() {
    let t = setup();
    let payer = Address::generate(&t.env);
    let key = StorageKey::PayerScore(payer.clone());

    t.env.as_contract(&t.contract.address, || {
        invoice::set_payer_score(&t.env, &payer, 55);
    });

    assert!(has_persistent_key(&t.env, &t.contract.address, &key));
    assert_eq!(t.contract.payer_score(&payer), 55);
}

#[test]
fn test_payer_score_removes_storage_when_reset_to_default() {
    let t = setup();
    let payer = Address::generate(&t.env);
    let key = StorageKey::PayerScore(payer.clone());

    t.env.as_contract(&t.contract.address, || {
        invoice::set_payer_score(&t.env, &payer, 55);
        invoice::set_payer_score(&t.env, &payer, 50);
    });

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
    assert_eq!(t.contract.payer_score(&payer), 50);
}

#[test]
fn test_empty_invoice_funders_not_stored() {
    let t = setup();
    let key = StorageKey::InvoiceFunders(123);

    t.env.as_contract(&t.contract.address, || {
        let empty = soroban_sdk::Vec::new(&t.env);
        invoice::save_invoice_funders(&t.env, 123, &empty);
    });

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
}

#[test]
fn test_empty_submitter_index_removed_after_last_invoice() {
    let t = setup();
    let submitter = Address::generate(&t.env);
    let key = StorageKey::SubmitterInvoices(submitter.clone());

    t.env.as_contract(&t.contract.address, || {
        invoice::add_invoice_to_submitter(&t.env, &submitter, 1);
        assert!(t.env.storage().persistent().has(&key));
        invoice::remove_invoice_from_submitter(&t.env, &submitter, 1);
    });

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
}

#[test]
fn test_zero_reputation_profile_not_stored() {
    let t = setup();
    let address = Address::generate(&t.env);
    let key = StorageKey::Reputation(address.clone());

    t.env.as_contract(&t.contract.address, || {
        invoice::set_reputation(
            &t.env,
            &ReputationProfile {
                address: address.clone(),
                invoices_submitted: 0,
                invoices_paid: 0,
                invoices_defaulted: 0,
                score: 0,
            },
        );
    });

    assert!(!has_persistent_key(&t.env, &t.contract.address, &key));
}
