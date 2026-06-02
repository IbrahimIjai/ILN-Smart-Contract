#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    Address, Env, IntoVal,
};

fn setup_env() -> (
    Env,
    Address,
    Address,
    InvoiceLiquidityContractClient<'static>,
) {
    let env = Env::default();
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let usdc_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_address = usdc_contract.address();

    let xlm_admin = Address::generate(&env);
    let xlm_contract = env.register_stellar_asset_contract_v2(xlm_admin.clone());
    let xlm_address = xlm_contract.address();

    let contract_id = env.register(InvoiceLiquidityContract, ());
    let client = InvoiceLiquidityContractClient::new(&env, &contract_id);

    client.initialize(&admin, &token_address, &xlm_address);

    let mut ledger = env.ledger().get();
    ledger.timestamp = 1_700_000_000;
    env.ledger().set(ledger);

    (env, admin, token_address, client)
}

#[test]
fn test_admin_violations() {
    let (env, _admin, _, client) = setup_env();
    let imposter = Address::generate(&env);
    let new_admin = Address::generate(&env);

    env.mock_auths(&[MockAuth {
        address: &imposter,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "set_admin",
            args: (new_admin.clone(),).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let res = client.try_set_admin(&new_admin);
    assert!(res.is_err());

    env.mock_auths(&[MockAuth {
        address: &imposter,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "pause",
            args: ().into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let res2 = client.try_pause();
    assert!(res2.is_err());
}

#[test]
fn test_public_methods() {
    let (_env, _admin, _, client) = setup_env();

    // Anyone can read contract stats without mock auth
    let stats = client.get_contract_stats();
    assert_eq!(stats.total_invoices, 0);

    let count = client.get_invoice_count();
    assert_eq!(count, 0);
}
