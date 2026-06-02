/// Comprehensive tests for Invoice NFT lifecycle (Issue #119)
///
/// Tests cover:
/// - NFT minting when invoice is submitted
/// - NFT ownership transfer when invoice is funded
/// - NFT burning when invoice is marked paid
/// - Metadata persistence and accuracy
/// - Error cases and edge cases

#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{Address, Env};

    struct TestEnv {
        env: Env,
        contract: InvoiceLiquidityContractClient,
        admin: Address,
        freelancer: Address,
        payer: Address,
        lp: Address,
        usdc_token: Address,
        eurc_token: Address,
        xlm_token: Address,
    }

    fn setup() -> TestEnv {
        let env = Env::default();

        // Generate test addresses
        let admin = Address::generate(&env);
        let freelancer = Address::generate(&env);
        let payer = Address::generate(&env);
        let lp = Address::generate(&env);

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
        token_client.mint(&lp, &10_000_000_000);

        // Authorize the contract to use tokens
        token_client.approve(&freelancer, &contract_id, &5_000_000_000, &100);
        token_client.approve(&payer, &contract_id, &5_000_000_000, &100);
        token_client.approve(&lp, &contract_id, &5_000_000_000, &100);

        // Setup for payer and submitter roles
        contract.add_payer(&payer);
        contract.add_submitter(&freelancer);
        contract.add_liquidity_provider(&lp);

        TestEnv {
            env,
            contract,
            admin,
            freelancer,
            payer,
            lp,
            usdc_token: usdc_token_addr,
            eurc_token: eurc_token_addr,
            xlm_token: xlm_token_addr,
        }
    }

    // ────────────────────────────────────────────────────────────
    // Test 1: NFT minting on invoice submission
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_minted_on_submit_invoice() {
        let t = setup();

        let amount = 1_000_000_000i128; // 100 USDC in stroops
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60; // 30 days
        let discount_rate = 300; // 3%

        // Submit an invoice
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Verify NFT was minted
        assert!(t.contract.invoice_nft_exists(&invoice_id));

        // Verify NFT metadata
        let metadata = t
            .contract
            .get_invoice_nft_metadata(&invoice_id)
            .unwrap();
        assert_eq!(metadata.invoice_id, invoice_id);
        assert_eq!(metadata.amount, amount);
        assert_eq!(metadata.discount_rate, discount_rate);
        assert_eq!(metadata.token, t.usdc_token);
        assert_eq!(metadata.owner, t.freelancer);

        // Verify NFT owner is the freelancer (invoice submitter)
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.freelancer);
    }

    // ────────────────────────────────────────────────────────────
    // Test 2: NFT transferred to LP on invoice funding
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_transferred_on_fund_invoice() {
        let t = setup();

        let amount = 1_000_000_000i128; // 100 USDC in stroops
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60; // 30 days
        let discount_rate = 300; // 3%

        // Submit an invoice
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Verify initial owner is freelancer
        let owner_before = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner_before, t.freelancer);

        // Fund the invoice completely
        t.contract.fund_invoice(&t.lp, &invoice_id, &amount, &false);

        // Verify NFT owner changed to the LP
        let owner_after = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner_after, t.lp);

        // Verify NFT still exists and metadata is intact
        assert!(t.contract.invoice_nft_exists(&invoice_id));
        let metadata = t
            .contract
            .get_invoice_nft_metadata(&invoice_id)
            .unwrap();
        assert_eq!(metadata.amount, amount);
    }

    // ────────────────────────────────────────────────────────────
    // Test 3: NFT burned when invoice is marked paid
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_burned_on_mark_paid() {
        let t = setup();

        let amount = 1_000_000_000i128; // 100 USDC in stroops
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60; // 30 days
        let discount_rate = 300; // 3%

        // Submit an invoice
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Verify NFT exists
        assert!(t.contract.invoice_nft_exists(&invoice_id));

        // Fund the invoice
        t.contract.fund_invoice(&t.lp, &invoice_id, &amount, &false);

        // Verify NFT is owned by LP
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.lp);

        // Mark invoice as paid
        t.contract.mark_paid(&t.payer, &invoice_id, &amount);

        // Verify NFT was burned (no longer exists)
        assert!(!t.contract.invoice_nft_exists(&invoice_id));

        // Verify metadata query returns None
        let result = t.contract.get_invoice_nft_metadata(&invoice_id);
        assert!(result.is_err() || result.unwrap_err() == ContractError::InvoiceNftNotFound);
    }

    // ────────────────────────────────────────────────────────────
    // Test 4: Full lifecycle - submit, fund, pay
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_full_lifecycle() {
        let t = setup();

        let amount = 1_000_000_000i128; // 100 USDC in stroops
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60; // 30 days
        let discount_rate = 300; // 3%

        // Step 1: Submit invoice - NFT minted to freelancer
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        assert!(t.contract.invoice_nft_exists(&invoice_id));
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.freelancer);

        // Step 2: Fund invoice - NFT transferred to LP
        t.contract.fund_invoice(&t.lp, &invoice_id, &amount, &false);

        assert!(t.contract.invoice_nft_exists(&invoice_id));
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.lp);

        // Step 3: Mark paid - NFT burned
        t.contract.mark_paid(&t.payer, &invoice_id, &amount);

        assert!(!t.contract.invoice_nft_exists(&invoice_id));
    }

    // ────────────────────────────────────────────────────────────
    // Test 5: Multiple invoices maintain separate NFTs
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_multiple_nfts_independent() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Create first invoice
        let invoice_id_1 = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Create second invoice
        let invoice_id_2 = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Verify both NFTs exist and are independent
        assert!(t.contract.invoice_nft_exists(&invoice_id_1));
        assert!(t.contract.invoice_nft_exists(&invoice_id_2));

        let metadata_1 = t
            .contract
            .get_invoice_nft_metadata(&invoice_id_1)
            .unwrap();
        let metadata_2 = t
            .contract
            .get_invoice_nft_metadata(&invoice_id_2)
            .unwrap();

        assert_eq!(metadata_1.invoice_id, invoice_id_1);
        assert_eq!(metadata_2.invoice_id, invoice_id_2);

        // Fund first invoice - second should remain unchanged
        t.contract.fund_invoice(&t.lp, &invoice_id_1, &amount, &false);

        let owner_1 = t.contract.get_invoice_nft_owner(&invoice_id_1).unwrap();
        let owner_2 = t.contract.get_invoice_nft_owner(&invoice_id_2).unwrap();

        assert_eq!(owner_1, t.lp);
        assert_eq!(owner_2, t.freelancer);
    }

    // ────────────────────────────────────────────────────────────
    // Test 6: NFT metadata accuracy across token types
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_metadata_different_tokens() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 500; // 5%

        // Submit invoice with USDC
        let invoice_id_usdc = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Submit invoice with EURC
        let invoice_id_eurc = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.eurc_token,
            )
            .unwrap();

        let metadata_usdc = t
            .contract
            .get_invoice_nft_metadata(&invoice_id_usdc)
            .unwrap();
        let metadata_eurc = t
            .contract
            .get_invoice_nft_metadata(&invoice_id_eurc)
            .unwrap();

        // Verify tokens are different
        assert_eq!(metadata_usdc.token, t.usdc_token);
        assert_eq!(metadata_eurc.token, t.eurc_token);
        assert_ne!(metadata_usdc.token, metadata_eurc.token);

        // Other metadata should be the same
        assert_eq!(metadata_usdc.amount, metadata_eurc.amount);
        assert_eq!(metadata_usdc.discount_rate, metadata_eurc.discount_rate);
    }

    // ────────────────────────────────────────────────────────────
    // Test 7: NFT metadata includes correct minting timestamp
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_minted_at_timestamp() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        let timestamp_before = t.env.ledger().timestamp();

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        let timestamp_after = t.env.ledger().timestamp();

        let metadata = t
            .contract
            .get_invoice_nft_metadata(&invoice_id)
            .unwrap();

        // Verify minted_at is within the expected range
        assert!(metadata.minted_at >= timestamp_before);
        assert!(metadata.minted_at <= timestamp_after);
    }

    // ────────────────────────────────────────────────────────────
    // Test 8: Partial funding doesn't transfer NFT
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_not_transferred_on_partial_funding() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Partially fund the invoice (50%)
        let partial_amount = amount / 2;
        t.contract.fund_invoice(&t.lp, &invoice_id, &partial_amount, &false);

        // NFT should still be owned by freelancer (not transferred on partial funding)
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.freelancer);

        // NFT should still exist
        assert!(t.contract.invoice_nft_exists(&invoice_id));
    }

    // ────────────────────────────────────────────────────────────
    // Test 9: Query functions handle non-existent NFTs gracefully
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_queries_non_existent() {
        let t = setup();

        let non_existent_id = 99999u64;

        // Verify NFT doesn't exist
        assert!(!t.contract.invoice_nft_exists(&non_existent_id));

        // Query owner returns None
        let owner = t.contract.get_invoice_nft_owner(&non_existent_id);
        assert!(owner.is_none());

        // Query metadata returns error
        let result = t.contract.get_invoice_nft_metadata(&non_existent_id);
        assert!(result.is_err());
    }

    // ────────────────────────────────────────────────────────────
    // Test 10: NFT metadata preserves all invoice parameters
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_metadata_comprehensive() {
        let t = setup();

        let amount = 5_000_000_000i128; // Large amount
        let due_date = t.env.ledger().timestamp() as u64 + 60 * 24 * 60 * 60; // 60 days
        let discount_rate = 1000; // 10%

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.eurc_token,
            )
            .unwrap();

        let metadata = t
            .contract
            .get_invoice_nft_metadata(&invoice_id)
            .unwrap();

        // Verify all fields match the input
        assert_eq!(metadata.invoice_id, invoice_id);
        assert_eq!(metadata.amount, amount);
        assert_eq!(metadata.due_date, due_date as u32);
        assert_eq!(metadata.discount_rate, discount_rate);
        assert_eq!(metadata.token, t.eurc_token);
        assert_eq!(metadata.owner, t.freelancer);
    }

    // ────────────────────────────────────────────────────────────
    // Test 11: NFT transferred when LP position is transferred
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_transferred_when_lp_position_transferred() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        // Setup a second LP
        let lp2 = Address::generate(&t.env);
        t.contract.add_liquidity_provider(&lp2);

        // Mint tokens for lp2
        let token_client = crate::soroban_sdk::token::Client::new(&t.env, &t.usdc_token);
        token_client.mint(&lp2, &10_000_000_000);
        token_client.approve(&lp2, &t.contract.contract_id, &5_000_000_000, &100);

        // Submit invoice
        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Fund invoice - NFT owned by original LP
        t.contract.fund_invoice(&t.lp, &invoice_id, &amount, &false);
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.lp);

        // Transfer LP position to lp2
        t.contract.transfer_lp_position(&t.lp, &invoice_id, &lp2);

        // Verify NFT owner changed to lp2
        let owner_after = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner_after, lp2);

        // Verify NFT still exists
        assert!(t.contract.invoice_nft_exists(&invoice_id));

        // Verify metadata is intact
        let metadata = t
            .contract
            .get_invoice_nft_metadata(&invoice_id)
            .unwrap();
        assert_eq!(metadata.amount, amount);
        assert_eq!(metadata.owner, lp2);
    }

    // ────────────────────────────────────────────────────────────
    // Test 12: Partial funding - NFT remains with freelancer
    // Then full funding - NFT transfers to LP
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_remains_with_freelancer_on_partial_then_transfers_on_full() {
        let t = setup();

        let amount = 2_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Partial fund - 50%
        let partial_amount = amount / 2;
        t.contract.fund_invoice(&t.lp, &invoice_id, &partial_amount, &false);

        // NFT should still be owned by freelancer
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.freelancer);

        // Complete funding - 50%
        t.contract.fund_invoice(&t.lp, &invoice_id, &partial_amount, &false);

        // Now NFT should be owned by LP
        let owner = t.contract.get_invoice_nft_owner(&invoice_id).unwrap();
        assert_eq!(owner, t.lp);
    }

    // ────────────────────────────────────────────────────────────
    // Test 13: NFT cannot be queried after burn
    // ────────────────────────────────────────────────────────────
    #[test]
    fn test_nft_query_after_burn() {
        let t = setup();

        let amount = 1_000_000_000i128;
        let due_date = t.env.ledger().timestamp() as u64 + 30 * 24 * 60 * 60;
        let discount_rate = 300;

        let invoice_id = t
            .contract
            .submit_invoice(
                &t.freelancer,
                &t.payer,
                &amount,
                &due_date,
                &discount_rate,
                &t.usdc_token,
            )
            .unwrap();

        // Fund
        t.contract.fund_invoice(&t.lp, &invoice_id, &amount, &false);

        // Mark paid (burns NFT)
        t.contract.mark_paid(&t.payer, &invoice_id, &amount);

        // Query should fail
        let result = t.contract.get_invoice_nft_metadata(&invoice_id);
        assert!(result.is_err());

        let owner = t.contract.get_invoice_nft_owner(&invoice_id);
        assert!(owner.is_none());

        // invoice_nft_exists should return false
        assert!(!t.contract.invoice_nft_exists(&invoice_id));
    }
}

