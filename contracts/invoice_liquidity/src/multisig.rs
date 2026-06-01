/// Multi-signature Admin Module (Issue #124)
///
/// Implements a threshold-based multi-signature scheme for high-security admin operations.
/// Requires a configurable threshold of authorized signers to approve critical actions
/// such as pause, contract upgrade, or token removal.
///
/// Workflow:
/// 1. Any signer calls propose_admin_action() to create a proposal
/// 2. Signers call sign_admin_action() to approve the proposal
/// 3. Once threshold is reached, any signer calls execute_admin_action()
/// 4. Proposals expire after MULTISIG_WINDOW_LEDGERS if not executed

use soroban_sdk::{contracttype, Address, Env, Vec};
use crate::errors::ContractError;

/// Number of ledgers a multisig proposal remains valid (approximately 24 hours)
pub const MULTISIG_WINDOW_LEDGERS: u64 = 17_280;

/// Enumeration of admin actions that require multi-sig approval
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum AdminAction {
    /// Pause contract (emergency stop)
    Pause,
    /// Unpause contract (resume operations)
    Unpause,
    /// Remove a token from approved tokens list
    RemoveToken(Address),
    /// Change the fee rate
    SetFeeRate(u32),
    /// Set maximum discount rate
    SetMaxDiscount(u32),
    /// Update multisig configuration itself (change signers or threshold)
    UpdateMultisig {
        new_signers: Vec<Address>,
        new_threshold: u32,
    },
}

/// Multi-signature admin configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MultisigAdmin {
    /// List of authorized signers
    pub signers: Vec<Address>,
    /// Number of signatures required to execute an action
    pub threshold: u32,
}

/// A proposal for an admin action awaiting signatures
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalState {
    Pending,
    Executed,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MultisigProposal {
    /// Unique proposal ID
    pub id: u64,
    /// The proposed action
    pub action: AdminAction,
    /// List of signers who have approved this proposal
    pub signers_approved: Vec<Address>,
    /// Current state of the proposal
    pub state: ProposalState,
    /// Ledger sequence number when this proposal expires
    pub expires_at: u64,
}

/// Validate that an address is in the signer list
pub fn is_signer(env: &Env, signers: &Vec<Address>, address: &Address) -> bool {
    for i in 0..signers.len() {
        if signers.get(i).unwrap() == *address {
            return true;
        }
    }
    false
}

/// Check if a signer has already approved a proposal
pub fn has_signed(proposal: &MultisigProposal, signer: &Address) -> bool {
    for i in 0..proposal.signers_approved.len() {
        if proposal.signers_approved.get(i).unwrap() == *signer {
            return true;
        }
    }
    false
}

/// Check if proposal has reached the approval threshold
pub fn threshold_reached(proposal: &MultisigProposal, threshold: u32) -> bool {
    proposal.signers_approved.len() as u32 >= threshold
}

/// Check if proposal has expired
pub fn is_expired(env: &Env, proposal: &MultisigProposal) -> bool {
    env.ledger().sequence() >= proposal.expires_at
}
