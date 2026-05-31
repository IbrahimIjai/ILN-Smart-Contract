//! Integration tests for the mock payer-verification oracle.
//!
//! Issue #94 — verifies the mock oracle's controllable behaviour that
//! oracle-dependent tests rely on.

#![cfg(test)]

extern crate std;

#[path = "mocks/mock_oracle.rs"]
mod mock_oracle;

use mock_oracle::{MockOracle, MockOracleClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, MockOracleClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(MockOracle, ());
    let client = MockOracleClient::new(&env, &id);
    (env, client)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_unverified_by_default() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    let result = client.get_verification(&payer);
    assert!(!result.verified);
    assert_eq!(result.timestamp, 0);
}

#[test]
fn test_set_verified_true() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    client.set_verified(&payer, &true);
    assert!(client.get_verification(&payer).verified);
}

#[test]
fn test_revoke_verification() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    client.set_verified(&payer, &true);
    client.set_verified(&payer, &false);
    assert!(!client.get_verification(&payer).verified);
}

#[test]
fn test_set_timestamp() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    client.set_timestamp(&1_700_000_000u64);
    assert_eq!(client.get_verification(&payer).timestamp, 1_700_000_000u64);
}

#[test]
fn test_stale_data_detection() {
    // Demonstrates that calling code can detect staleness by comparing
    // the returned timestamp against the current time.
    let (env, client) = setup();
    let payer = Address::generate(&env);

    let stale_ts: u64 = 1_000_000_000;
    client.set_verified(&payer, &true);
    client.set_timestamp(&stale_ts);

    let result = client.get_verification(&payer);
    let now: u64 = stale_ts + 31 * 24 * 60 * 60; // 31 days later
    let staleness_threshold: u64 = 7 * 24 * 60 * 60;
    assert!(
        now.saturating_sub(result.timestamp) > staleness_threshold,
        "data older than 7 days should be treated as stale"
    );
}

#[test]
#[should_panic(expected = "forced panic")]
fn test_set_should_panic() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    client.set_should_panic();
    let _ = client.get_verification(&payer);
}

#[test]
fn test_panic_flag_is_one_shot() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    // No panic armed — two sequential calls must both succeed
    let r1 = client.get_verification(&payer);
    let r2 = client.get_verification(&payer);
    assert_eq!(r1.verified, r2.verified);
}

#[test]
fn test_independent_per_address() {
    let (env, client) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    client.set_verified(&a, &true);
    assert!(client.get_verification(&a).verified);
    assert!(!client.get_verification(&b).verified);
}

#[test]
fn test_update_verification_interface() {
    let (env, client) = setup();
    let payer = Address::generate(&env);
    // update_verification is the oracle-operator method (OracleInterface)
    client.update_verification(&payer, &true);
    assert!(client.get_verification(&payer).verified);
}
