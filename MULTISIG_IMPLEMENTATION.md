# Multi-Signature Admin Implementation (Issue #124)

## Overview

This document describes the implementation of threshold-based multi-signature (M-of-N) admin approval for the Invoice Liquidity Network contract. This feature enables secure governance by requiring multiple authorized signers to approve critical operations such as pausing the contract.

## Architecture

### Core Components

#### 1. **Multisig Module** ([multisig.rs](contracts/invoice_liquidity/src/multisig.rs))

The module defines the data structures and helper functions for multisig operations:

**Key Types:**

- **`MultisigAdmin`**: Configuration holding the list of authorized signers and the required threshold
  ```rust
  pub struct MultisigAdmin {
      pub signers: Vec<Address>,
      pub threshold: u32,
  }
  ```

- **`AdminAction`**: Enumeration of actions requiring multisig approval
  - `Pause`: Emergency stop of contract
  - `Unpause`: Resume operations
  - `RemoveToken(Address)`: Remove token from approved list
  - `SetFeeRate(u32)`: Change fee rate
  - `SetMaxDiscount(u32)`: Set maximum discount rate
  - `UpdateMultisig { ... }`: Update multisig configuration itself

- **`MultisigProposal`**: Represents a pending or executed proposal
  ```rust
  pub struct MultisigProposal {
      pub id: u64,                          // Unique proposal ID
      pub action: AdminAction,              // The proposed action
      pub signers_approved: Vec<Address>,   // Signers who approved
      pub state: ProposalState,             // Pending/Executed/Expired
      pub expires_at: u64,                  // Ledger sequence expiration
  }
  ```

- **`ProposalState`**: Three-state lifecycle
  - `Pending`: Awaiting signatures
  - `Executed`: Successfully executed
  - `Expired`: Exceeded the execution window

**Constants:**

- `MULTISIG_WINDOW_LEDGERS = 17_280`: Approximately 24 hours. Proposals expire if not executed within this window.

**Helper Functions:**

- `is_signer()`: Check if address is in signer list
- `has_signed()`: Check if signer already approved a proposal
- `threshold_reached()`: Check if approval threshold is met
- `is_expired()`: Check if proposal has expired

#### 2. **Contract Functions** ([lib.rs](contracts/invoice_liquidity/src/lib.rs))

Public contract functions for multisig operations:

##### `initialize_multisig_admin(env, signers, threshold)`

Initialize multi-signature admin functionality.

- **Parameters:**
  - `signers`: Vec of addresses authorized to sign
  - `threshold`: Number of signatures required (must be ≤ `signers.len()`)

- **Returns:**
  - `Ok(())` on success
  - `Err(InvalidMultisigConfig)` if validation fails

- **Access:** Admin only

- **Example:**
  ```
  let signers = [addr1, addr2, addr3]
  initialize_multisig_admin(env, signers, 2)  // 2-of-3 multisig
  ```

##### `propose_pause(env, proposer)` / `propose_unpause(env, proposer)`

Create a new pause/unpause proposal.

- **Parameters:**
  - `proposer`: Must be an authorized signer

- **Returns:**
  - `Ok(proposal_id)` on success
  - `Err(NotAuthorizedSigner)` if proposer is not in signer list

- **Access:** Multi-sig authorized signer

##### `sign_proposal(env, signer, proposal_id)`

Add signer's approval to an existing proposal.

- **Parameters:**
  - `signer`: Must be an authorized signer
  - `proposal_id`: ID of the proposal to sign

- **Returns:**
  - `Ok(())` on success
  - `Err(NotAuthorizedSigner)` if not an authorized signer
  - `Err(AlreadySigned)` if signer already approved this proposal
  - `Err(ProposalNotFound)` if proposal doesn't exist

- **Access:** Multi-sig authorized signer

##### `execute_proposal(env, executor, proposal_id)`

Execute a proposal that has reached the threshold.

- **Parameters:**
  - `executor`: Must be an authorized signer (triggers execution but doesn't need to be new signer)
  - `proposal_id`: ID of the proposal to execute

- **Returns:**
  - `Ok(())` on success
  - `Err(ThresholdNotReached)` if not enough signatures
  - `Err(ProposalNotFound)` if proposal doesn't exist
  - `Err(ProposalAlreadyExecuted)` if already executed
  - `Err(ProposalExpired)` if outside execution window

- **Access:** Multi-sig authorized signer

#### 3. **Storage Layer** ([storage.rs](contracts/invoice_liquidity/src/storage.rs))

Helper functions for persistent storage of multisig data:

```rust
pub fn get_multisig_admin(env: &Env) -> Option<MultisigAdmin>
pub fn set_multisig_admin(env: &Env, admin: &MultisigAdmin)
pub fn get_multisig_proposal(env: &Env, proposal_id: u64) -> Option<MultisigProposal>
pub fn save_multisig_proposal(env: &Env, proposal: &MultisigProposal)
pub fn get_next_proposal_id(env: &Env) -> u64
pub fn increment_proposal_id(env: &Env)
```

Uses the `DataKey` enum for type-safe storage:
- `DataKey::MultisigAdmin`: Instance storage for admin config
- `DataKey::MultisigProposalCounter`: Instance storage for proposal ID counter
- `DataKey::MultisigProposal(u64)`: Persistent storage for proposals by ID

## Usage Workflow

### Example: 2-of-3 Pause Proposal

```rust
// Step 1: Initialize with 3 signers, require 2 approvals
let signers = vec![alice, bob, carol];
contract.initialize_multisig_admin(&signers, 2);

// Step 2: Alice proposes a pause
let proposal_id = contract.propose_pause(&alice)?;

// Step 3: Bob signs the proposal
contract.sign_proposal(&bob, proposal_id)?;

// Step 4: Once threshold (2) is reached, anyone can execute
contract.execute_proposal(&carol, proposal_id)?;
// Contract is now paused
```

## Error Handling

| Error | Code | Condition |
|-------|------|-----------|
| `NotAuthorizedSigner` | 40 | Caller is not in the signer list |
| `ProposalNotFound` | 41 | Proposal doesn't exist |
| `AlreadySigned` | 42 | Signer has already approved this proposal |
| `ProposalExpired` | 43 | Outside the execution window (17,280 ledgers) |
| `ThresholdNotReached` | 44 | Not enough signatures collected |
| `ProposalAlreadyExecuted` | 45 | Proposal was already executed |
| `InvalidMultisigConfig` | 46 | Threshold > signer count or threshold is 0 |

## Testing

Comprehensive test suite in [tests_multisig_admin.rs](contracts/invoice_liquidity/src/tests_multisig_admin.rs):

**Test Coverage:**

1. ✅ **Initialization**: 2-of-3 and 3-of-3 threshold setup
2. ✅ **Proposal Creation**: Creating pause/unpause proposals
3. ✅ **Signing**: Adding signatures, preventing duplicates
4. ✅ **Threshold Validation**: Ensuring threshold is enforced
5. ✅ **Execution**: Executing proposals when threshold is met
6. ✅ **Authorization**: Non-signers cannot participate
7. ✅ **Idempotency**: No re-execution of completed proposals
8. ✅ **Signature Order**: Signatures can arrive in any order
9. ✅ **Config Validation**: Invalid threshold configurations rejected

**Sample Test:**
```rust
#[test]
fn test_sign_and_execute_threshold_met() {
    let t = setup_multisig();
    
    // Setup 2-of-3 multisig
    let signers = vec![t.admin1, t.admin2, t.admin3];
    t.contract.initialize_multisig_admin(&signers, 2).unwrap();
    
    // Propose pause
    let proposal_id = t.contract.propose_pause(&t.admin1).unwrap();
    
    // Collect signatures
    t.contract.sign_proposal(&t.admin1, &proposal_id).unwrap();
    t.contract.sign_proposal(&t.admin2, &proposal_id).unwrap();
    
    // Execute (threshold reached)
    t.contract.execute_proposal(&t.admin1, &proposal_id).unwrap();
    
    // Verify pause is active
    assert!(t.contract.is_paused());
}
```

## Security Considerations

1. **Threshold Safety**: Requires `threshold ≤ signer_count`, preventing impossible configurations
2. **Duplicate Prevention**: Each signer can only sign a proposal once
3. **Expiration Window**: Proposals expire after 17,280 ledgers (~24 hours) to prevent stale proposal execution
4. **State Transitions**: Prevents re-execution and modifies state atomically
5. **Authorization**: Only authorized signers can propose and sign
6. **Order Independence**: Signatures can arrive in any order

## Future Enhancements

1. **Multi-Action Batching**: Support batching multiple actions in a single proposal
2. **Weighted Voting**: Different signers with different voting weights
3. **Time Locks**: Additional delay between execution threshold and actual execution
4. **Conditional Execution**: Execute actions based on contract state conditions
5. **Signature Revocation**: Allow signers to revoke their approval before execution
6. **Multisig Upgrades**: Change signers/threshold via multisig proposal itself

## Integration Points

The multisig admin system integrates with:

- **Pause/Unpause**: Critical contract state management
- **Token Management**: Future support for removing approved tokens
- **Fee Updates**: Future support for changing fee rates
- **Contract Upgrades**: Future support for governance-approved upgrades

## Related Issues

- **Issue #124**: Multi-sig Admin (this implementation)
- **Issue #48**: Contract Upgrades (future multisig integration)
- **Issue #95**: Emergency Controls (pause/unpause via multisig)

## Files Modified

1. ✅ [contracts/invoice_liquidity/src/multisig.rs](contracts/invoice_liquidity/src/multisig.rs) - Already existed
2. ✅ [contracts/invoice_liquidity/src/lib.rs](contracts/invoice_liquidity/src/lib.rs) - Added contract functions
3. ✅ [contracts/invoice_liquidity/src/storage.rs](contracts/invoice_liquidity/src/storage.rs) - Already had helper functions
4. ✅ [contracts/invoice_liquidity/src/errors.rs](contracts/invoice_liquidity/src/errors.rs) - Already had error codes
5. ✅ [contracts/invoice_liquidity/src/tests_multisig_admin.rs](contracts/invoice_liquidity/src/tests_multisig_admin.rs) - NEW: Comprehensive test suite

## Compilation

To compile the contract with multisig support:

```bash
cd contracts/invoice_liquidity
cargo build --release
```

To run tests:

```bash
cargo test tests_multisig_admin --lib
```

## Deployment Notes

1. After contract deployment, call `initialize_multisig_admin()` to activate multisig governance
2. Store the initial signer list securely
3. Test the proposal workflow on testnet before mainnet deployment
4. Document the multisig configuration for operational teams
