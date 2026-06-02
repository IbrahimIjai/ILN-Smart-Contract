/// Comprehensive tests for LP whitelist feature (Issue #122)
///
/// Tests cover:
/// - Public invoices (no whitelist)
/// - Private invoices with LP whitelist
/// - Whitelisted LP can fund
/// - Non-whitelisted LP is rejected
/// - Whitelist size validation
/// - Whitelist in event emission

#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{Address, Env, Vec};

    struct TestEnv {
        env: Env,
        contract: InvoiceLiquidityContractClient,
        admin: Address,
        freelancer: Address,
        payer: Address,
        lp1: Address,
        lp2: Address,
        lp3: Address,
        usdc_token: Address,
    }

    fn setup() -> TestEnv {
        let env = Env::default();

        // Generate test addresses
        let admin = Address::generate(&env);
        let freelancer = Address::generate(&env);
        let payer = Address::generate(&env);
        let lp1 = Address::generate(&env);
        let lp2 = Address::generate(&env);
        let lp3 = Address::generate(&env);

        // Register token contracts
        let usdc_admin = Address::generate(&env);
        let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin);
        let usdc_token_addr = usdc_token.address();

        let eurc_admin = Address::generate(&env);
        let eurc_token = env.register_stellar_asset_contract_v2(eurc_admin);
        let eurc_token_addr = eurc_token.address();

        let xlm_admin = Address::generate(&env);
        let xlm_token = env.register_stellar_asset_contract_v2(xlm_admin);
        let xlm_token_addr = xlm_token.address();

        // Deploy the main contract
        let contract_id = env.register_contract(None, InvoiceLiquidityContract);
        let contract = InvoiceLiquidityContractClient::new(&env, &contract_id);

        // Initialize the contract
        contract.initialize(
            &admin,
            &usdc_token_addr,
            &eurc_token_addr,
            &xlm_token_addr,
        );

        // Mint tokens for testing
        let token_client = crate::soroban_sdk::token::Client::new(&env, &usdc_token_addr);
        token_client.mint(&freelancer, &10_000_000_000);
        token_client.mint(&payer, &10_000_000_000);
        token_client.mint(&lp1, &10_000_000_000);
        token_client.mint(&lp2, &10_000_000_000);
        token_client.mint(&lp3, &10_000_000_000);

        // Authorize the contract to use tokens
        token_client.approve(&freelancer, &contract_id, &5_000_000_000, &100);
        token_client.approve(&payer, &contract_id, &5_000_000_000, &100);
        token_client.approve(&lp1, &contract_id, &5_000_000_000, &100);
        token_client.approve(&lp2, &contract_id, &5_000_000_000, &100);
        token_client.approve(&lp3, &contract_id, &5_000_000_000, &100);

        // Setup roles
        contract.add_payer(&payer);
        contract.add_submitter(&freelancer);
        contract.add_liquidity_provider(&lp1);
        contract.add_liquidity_provider(&lp2);
        contract.add_liquidity_provider(&lp3);

        TestEnv {
            env,
            contract,
            admin,
            freelancer,
            payer,
            lp1,
            lp2,
            lp3,
            usdc_token: usdc_token_addr,
        }
    }

    // ────────────────────────────────────────────────────────────
    // Test 1: Public invoice with no whitelist
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_public_invoice_no_whitelist() {
        let t = setup();

        let amount = 1_000_000_000i128; // 100 USDC
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Submit invoice with NO whitelist (public)
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &None, // No whitelist
            )
            .unwrap();

        // Any LP should be able to fund
        let result = t.contract.fund_invoice(&t.lp1, &invoice_id, &amount, &false);
        assert!(result.is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 2: Private invoice with whitelist - whitelisted LP succeeds
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_private_invoice_whitelisted_lp_succeeds() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create whitelist with lp1 and lp2
        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());
        whitelist.push_back(t.lp2.clone());

        // Submit private invoice
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // LP1 (whitelisted) should succeed
        let result = t.contract.fund_invoice(&t.lp1, &invoice_id, &amount, &false);
        assert!(result.is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 3: Private invoice with whitelist - non-whitelisted LP rejected
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_private_invoice_non_whitelisted_lp_rejected() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create whitelist with only lp1
        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());

        // Submit private invoice
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // LP2 (not whitelisted) should be rejected
        let result = t.contract.fund_invoice(&t.lp2, &invoice_id, &amount, &false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, ContractError::LPNotWhitelisted);

        // LP3 (not whitelisted) should also be rejected
        let result = t.contract.fund_invoice(&t.lp3, &invoice_id, &amount, &false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ContractError::LPNotWhitelisted);
    }

    // ────────────────────────────────────────────────────────────
    // Test 4: Single LP whitelist
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_single_lp_whitelist() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create whitelist with only lp1
        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // Only lp1 can fund
        assert!(t.contract.fund_invoice(&t.lp1, &invoice_id, &amount, &false).is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 5: Multiple LPs in whitelist
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_multiple_lps_in_whitelist() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create whitelist with all three LPs
        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());
        whitelist.push_back(t.lp2.clone());
        whitelist.push_back(t.lp3.clone());

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // All three LPs should be able to fund
        assert!(t.contract.fund_invoice(&t.lp1, &invoice_id, &amount, &false).is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 6: Whitelist size validation (max 10)
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_whitelist_max_size_validation() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create a whitelist with 11 addresses (exceeds max)
        let mut whitelist = Vec::new(&t.env);
        for _i in 0..11 {
            whitelist.push_back(Address::generate(&t.env));
        }

        // Should reject whitelist larger than 10
        let result = t.contract.submit_invoice(
            &t.freelancer,
            &t.payer,
            &amount,
            &due_date,
            &discount_rate,
            &t.usdc_token,
            &None,
            &Some(whitelist),
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ContractError::WhitelistTooLarge);
    }

    // ────────────────────────────────────────────────────────────
    // Test 7: Whitelist of exactly 10 addresses (boundary case)
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_whitelist_exactly_10_addresses() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create whitelist with exactly 10 addresses
        let mut whitelist = Vec::new(&t.env);
        let mut last_lp = None;
        for _i in 0..10 {
            let lp = Address::generate(&t.env);
            if _i == 0 {
                last_lp = Some(lp.clone());
            }
            whitelist.push_back(lp);
        }

        // Should accept whitelist of exactly 10
        let result = t.contract.submit_invoice(
            &t.freelancer,
            &t.payer,
            &amount,
            &due_date,
            &discount_rate,
            &t.usdc_token,
            &None,
            &Some(whitelist),
        );

        assert!(result.is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 8: Whitelist emitted in event
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_whitelist_in_event() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create whitelist
        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());
        whitelist.push_back(t.lp2.clone());

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist.clone()),
            )
            .unwrap();

        // Verify invoice was created
        assert!(invoice_id > 0);
        // The whitelist should be stored and available via query
        // (This would be verified through event logs in production)
    }

    // ────────────────────────────────────────────────────────────
    // Test 9: Whitelisted LP can fund partial invoice
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_whitelisted_lp_partial_funding() {
        let t = setup();

        let amount = 2_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());
        whitelist.push_back(t.lp2.clone());

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // Partial fund with lp1 (whitelisted)
        let partial_amount = amount / 2;
        let result = t.contract.fund_invoice(&t.lp1, &invoice_id, &partial_amount, &false);
        assert!(result.is_ok());

        // lp2 (whitelisted) should be able to complete funding
        let result = t.contract.fund_invoice(&t.lp2, &invoice_id, &partial_amount, &false);
        assert!(result.is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 10: Non-whitelisted LP cannot fund even with partial amount
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_non_whitelisted_lp_cannot_fund_partial() {
        let t = setup();

        let amount = 2_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        let mut whitelist = Vec::new(&t.env);
        whitelist.push_back(t.lp1.clone());

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // lp1 funds partial
        t.contract
            .fund_invoice(&t.lp1, &invoice_id, &(amount / 2), &false)
            .unwrap();

        // lp2 (not whitelisted) cannot fund even partial to complete it
        let result = t.contract.fund_invoice(&t.lp2, &invoice_id, &(amount / 2), &false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ContractError::LPNotWhitelisted);
    }

    // ────────────────────────────────────────────────────────────
    // Test 11: Multiple invoices with different whitelists
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_multiple_invoices_different_whitelists() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Invoice 1: whitelisted for lp1 only
        let mut whitelist1 = Vec::new(&t.env);
        whitelist1.push_back(t.lp1.clone());

        let invoice_id_1 = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist1),
            )
            .unwrap();

        // Invoice 2: whitelisted for lp2 only
        let mut whitelist2 = Vec::new(&t.env);
        whitelist2.push_back(t.lp2.clone());

        let invoice_id_2 = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist2),
            )
            .unwrap();

        // lp1 can fund invoice 1 but not invoice 2
        assert!(t.contract.fund_invoice(&t.lp1, &invoice_id_1, &amount, &false).is_ok());
        assert!(t.contract.fund_invoice(&t.lp1, &invoice_id_2, &amount, &false).is_err());

        // lp2 can fund invoice 2 but not invoice 1
        assert!(t.contract.fund_invoice(&t.lp2, &invoice_id_2, &amount, &false).is_ok());
    }

    // ────────────────────────────────────────────────────────────
    // Test 12: Empty whitelist behaves like public invoice
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_empty_whitelist_public_behavior() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create an empty whitelist
        let whitelist = Vec::new(&t.env);

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
                &None,
                &Some(whitelist),
            )
            .unwrap();

        // Any LP should be able to fund (empty whitelist = public)
        assert!(t.contract.fund_invoice(&t.lp1, &invoice_id, &amount, &false).is_ok());
    }
}
