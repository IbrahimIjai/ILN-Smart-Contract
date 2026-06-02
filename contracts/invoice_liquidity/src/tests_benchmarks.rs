#![cfg(test)]

//! Execution cost benchmarks for core contract instructions (Issue #76).
//! Emits machine-readable `BENCHMARK` lines for CI regression checks.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

const BENCH_INVOICE_AMOUNT: i128 = 1_000_000_000;
const BENCH_DISCOUNT_RATE: u32 = 300;

struct BaseBenchEnv {
    env: Env,
    contract: InvoiceLiquidityContractClient<'static>,
    token: Address,
    freelancer: Address,
    payer: Address,
    lp: Address,
}

fn setup_benchmark_env() -> BaseBenchEnv {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let mut ledger = env.ledger().get();
    ledger.timestamp = 1_700_000_000;
    env.ledger().set(ledger);

    let usdc_admin = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let xlm_admin = Address::generate(&env);
    let xlm = env.register_stellar_asset_contract_v2(xlm_admin);

    let contract_id = env.register(InvoiceLiquidityContract, ());
    let contract = InvoiceLiquidityContractClient::new(&env, &contract_id);
    let eurc_address = Address::generate(&env);
    contract.initialize(&usdc_admin, &usdc.address(), &eurc_address, &xlm.address());

    let freelancer = Address::generate(&env);
    let payer = Address::generate(&env);
    let lp = Address::generate(&env);

    let usdc_client = StellarAssetClient::new(&env, &usdc.address());
    usdc_client.mint(&lp, &1_000_000_000_000);
    usdc_client.mint(&payer, &1_000_000_000_000);

    BaseBenchEnv {
        env,
        contract,
        token: usdc.address(),
        freelancer,
        payer,
        lp,
    }
}

fn emit_benchmark(name: &str, cpu: u64, mem: u64) {
    std::println!("BENCHMARK {name} cpu={cpu} mem={mem}");
}

fn measure<F: FnOnce()>(env: &Env, name: &str, action: F) -> (u64, u64) {
    env.cost_estimate().budget().reset_unlimited();
    action();
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();
    emit_benchmark(name, cpu, mem);
    (cpu, mem)
}

#[test]
fn benchmark_submit_invoice() {
    let bench = setup_benchmark_env();
    let due_date = bench.env.ledger().timestamp() + 86_400;

    measure(&bench.env, "submit_invoice", || {
        bench.contract.submit_invoice(
            &bench.freelancer,
            &bench.payer,
            &BENCH_INVOICE_AMOUNT,
            &due_date,
            &BENCH_DISCOUNT_RATE,
            &bench.token,
        );
    });
}

#[test]
fn benchmark_fund_invoice() {
    let bench = setup_benchmark_env();
    let due_date = bench.env.ledger().timestamp() + 86_400;
    let id = bench.contract.submit_invoice(
        &bench.freelancer,
        &bench.payer,
        &BENCH_INVOICE_AMOUNT,
        &due_date,
        &BENCH_DISCOUNT_RATE,
        &bench.token,
    );

    measure(&bench.env, "fund_invoice", || {
        bench.contract.fund_invoice(&bench.lp, &id, &BENCH_INVOICE_AMOUNT, &false);
    });
}

#[test]
fn benchmark_mark_paid() {
    let bench = setup_benchmark_env();
    let due_date = bench.env.ledger().timestamp() + 86_400;
    let id = bench.contract.submit_invoice(
        &bench.freelancer,
        &bench.payer,
        &BENCH_INVOICE_AMOUNT,
        &due_date,
        &BENCH_DISCOUNT_RATE,
        &bench.token,
    );
    bench
        .contract
        .fund_invoice(&bench.lp, &id, &BENCH_INVOICE_AMOUNT, &false);

    measure(&bench.env, "mark_paid", || {
        bench
            .contract
            .mark_paid(&id, &BENCH_INVOICE_AMOUNT);
    });
}

#[test]
fn benchmark_all_functions_summary() {
    let mut results = std::vec::Vec::new();

    let bench = setup_benchmark_env();
    let due_date = bench.env.ledger().timestamp() + 86_400;

    results.push(measure(&bench.env, "submit_invoice", || {
        bench.contract.submit_invoice(
            &bench.freelancer,
            &bench.payer,
            &BENCH_INVOICE_AMOUNT,
            &due_date,
            &BENCH_DISCOUNT_RATE,
            &bench.token,
        );
    }));

    let id = bench.contract.submit_invoice(
        &bench.freelancer,
        &bench.payer,
        &BENCH_INVOICE_AMOUNT,
        &(due_date + 1),
        &BENCH_DISCOUNT_RATE,
        &bench.token,
    );
    results.push(measure(&bench.env, "fund_invoice", || {
        bench.contract.fund_invoice(&bench.lp, &id, &BENCH_INVOICE_AMOUNT, &false);
    }));
    results.push(measure(&bench.env, "mark_paid", || {
        bench
            .contract
            .mark_paid(&id, &BENCH_INVOICE_AMOUNT);
    }));

    std::println!("\n| Function       | CPU Instructions | Memory (bytes) |");
    std::println!("| -------------- | ---------------- | -------------- |");
    for (name, (cpu, mem)) in [
        ("submit_invoice", results[0]),
        ("fund_invoice", results[1]),
        ("mark_paid", results[2]),
    ] {
        std::println!("| {name:<14} | {cpu:>16} | {mem:>14} |");
    }
}
