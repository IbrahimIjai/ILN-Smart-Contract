#![cfg(test)]

use crate::{InvoiceLiquidityContract, InvoiceLiquidityContractClient};

use soroban_sdk::{
    testutils::{Address as _, Events},
    token::{TokenClient, StellarAssetClient},
    Address, Env, IntoVal, Symbol,
};

fn setup_env<'a>() -> (
    Env,
    InvoiceLiquidityContractClient<'a>,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let payer = Address::generate(&env);
    let lp = Address::generate(&env);
    let treasury = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = token_contract.address();

    let asset_client = StellarAssetClient::new(&env, &token);
    asset_client.mint(&lp, &10_000_000);
    asset_client.mint(&payer, &10_000_000);

    let contract_id = env.register(InvoiceLiquidityContract, ());
    let client = InvoiceLiquidityContractClient::new(&env, &contract_id);
    client.initialize(&admin, &token, &token);

    // whitelist LP
    client.add_approved_funder(&lp);

    (env, client, admin, freelancer, payer, lp, treasury, token)
}

#[test]
fn test_zero_fee() {
    let (env, client, _, freelancer, payer, lp, treasury, token) = setup_env();

    client.set_treasury_address(&treasury);
    client.update_protocol_fee_bps(&0);

    let due_date = env.ledger().timestamp() + (7 * 24 * 60 * 60);
    let amount = 1_000_000_i128;

    let invoice_id = client.submit_invoice(
        &freelancer,
        &payer,
        &amount,
        &due_date,
        &1000_u32, // 10% discount
        &token,
    );

    client.fund_invoice(&lp, &invoice_id, &amount, &false);
    
    // Check initial LP balance
    let token_client = TokenClient::new(&env, &token);
    let lp_balance_before = token_client.balance(&lp);

    // Mark paid
    client.mark_paid(&invoice_id, &amount);

    let lp_balance_after = token_client.balance(&lp);
    
    // LP funded 900,000 (1_000_000 - 10% discount)
    // LP earns 100,000. No fee, so receives full 1_000_000 back.
    assert_eq!(lp_balance_after - lp_balance_before, amount);

    let treasury_balance = token_client.balance(&treasury);
    assert_eq!(treasury_balance, 0);

    // Verify no FeesCollected event
    let events = env.events().all();
    for (_contract, (topics, _data)) in events.iter() {
        let topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap_or(Symbol::new(&env, ""));
        assert_ne!(topic, Symbol::new(&env, "fees_collected"));
    }
}

#[test]
fn test_non_zero_fee() {
    let (env, client, _, freelancer, payer, lp, treasury, token) = setup_env();

    client.set_treasury_address(&treasury);
    client.update_protocol_fee_bps(&100); // 1% fee on earnings

    let due_date = env.ledger().timestamp() + (7 * 24 * 60 * 60);
    let amount = 1_000_000_i128;

    let invoice_id = client.submit_invoice(
        &freelancer,
        &payer,
        &amount,
        &due_date,
        &1000_u32, // 10% discount => 900,000 funded
        &token,
    );

    client.fund_invoice(&lp, &invoice_id, &amount, &false);

    let token_client = TokenClient::new(&env, &token);
    let lp_balance_before = token_client.balance(&lp);

    client.mark_paid(&invoice_id, &amount);

    // LP funded 900,000. Gross earnings = 100,000.
    // Fee = 100,000 * 100 / 10000 = 1,000.
    // Net earnings = 99,000.
    // LP receives 900,000 + 99,000 = 999,000.
    let lp_balance_after = token_client.balance(&lp);
    assert_eq!(lp_balance_after - lp_balance_before, 999_000);

    let treasury_balance = token_client.balance(&treasury);
    assert_eq!(treasury_balance, 1_000);

    // Verify FeesCollected event
    let events = env.events().all();
    let mut fee_event_found = false;
    for (contract, (topics, data)) in events.iter() {
        let topic_opt: Option<Symbol> = topics.get(0).unwrap().try_into_val(&env).ok();
        if let Some(topic) = topic_opt {
            if topic == Symbol::new(&env, "fees_collected") {
                fee_event_found = true;
                let fee_amount: i128 = data.try_into_val(&env).unwrap();
                assert_eq!(fee_amount, 1_000);
                let invoice_topic: u64 = topics.get(1).unwrap().try_into_val(&env).unwrap();
                assert_eq!(invoice_topic, invoice_id);
                let treasury_topic: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
                assert_eq!(treasury_topic, treasury);
            }
        }
    }
    assert!(fee_event_found);
}

#[test]
fn test_governance_update() {
    let (env, client, admin, _, _, _, _, _) = setup_env();

    // Max fee is 100.
    let result = client.try_update_protocol_fee_bps(&101);
    assert!(result.is_err());

    client.update_protocol_fee_bps(&50);
    
    // Verify event
    let events = env.events().all();
    let mut update_event_found = false;
    for (_contract, (topics, data)) in events.iter() {
        let topic_opt: Option<Symbol> = topics.get(0).unwrap().try_into_val(&env).ok();
        if let Some(topic) = topic_opt {
            if topic == Symbol::new(&env, "parameter_updated") {
                let param_name: Symbol = topics.get(1).unwrap().try_into_val(&env).unwrap();
                if param_name == Symbol::new(&env, "protocol_fee_bps") {
                    update_event_found = true;
                }
            }
        }
    }
    assert!(update_event_found);
}

#[test]
fn test_treasury_update() {
    let (env, client, _, freelancer, payer, lp, _, token) = setup_env();

    let new_treasury = Address::generate(&env);
    client.set_treasury_address(&new_treasury);
    client.update_protocol_fee_bps(&100);

    let due_date = env.ledger().timestamp() + (7 * 24 * 60 * 60);
    let amount = 1_000_000_i128;

    let invoice_id = client.submit_invoice(
        &freelancer,
        &payer,
        &amount,
        &due_date,
        &1000_u32,
        &token,
    );

    client.fund_invoice(&lp, &invoice_id, &amount, &false);
    client.mark_paid(&invoice_id, &amount);

    let token_client = TokenClient::new(&env, &token);
    let new_treasury_balance = token_client.balance(&new_treasury);
    assert_eq!(new_treasury_balance, 1_000);
}
