// Event emission tests to catch regressions where events are dropped.
// This file is placed per `fix.md` to provide a consolidated smoke-test
// that exercises common instructions and asserts events are emitted.

#![cfg(test)]

mod test_context;
use soroban_sdk::{testutils::Address as _, Address, Env};
use test_context::TestContext;

// Lightweight smoke tests for event emission. These mirror patterns used in
// the individual contract test suites and assert that calling key
// instructions results in an event being published for the contract.

#[test]
fn invoice_liquidity_submit_emits_event() {
    let ctx = TestContext::new();

    let id = ctx.submit_invoice(1_000_000, 100, 1000);

    let events = ctx.env.events().all().filter_by_contract(&ctx.contract.address);
    assert!(events.events().last().is_some(), "submit_invoice must emit an event");

    // Basic sanity: ensure the last event contains the invoice id as bytes.
    let last = events.events().last().unwrap();
    let s = format!("{:?}", last);
    assert!(s.contains(&format!("{}", id)), "event must reference invoice id");
}
