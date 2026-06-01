use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum ContractError {
    InvoiceNotFound = 1,
    AlreadyFunded = 2,
    AlreadyPaid = 3,
    NotFunded = 4,
    Unauthorized = 5,
    InvalidAmount = 6,
    InvalidDiscountRate = 7,
    InvalidDueDate = 8,
    InvoiceDefaulted = 9,
    NothingToClaim = 10,
    NotYetDefaulted = 11,
    OverfundingRejected = 12,
    InvoiceExpired = 13,
    BatchTooLarge = 14,
    AlreadyCancelled = 15,
    AlreadyInitialized = 16,
    // ── Issue #36: appeal_default ──────────────────────────────────
    /// Payer attempted to appeal an invoice that is already in Appealed state.
    AlreadyAppealed = 17,
    /// Appeal window has closed; appeal can no longer be submitted.
    AppealWindowClosed = 18,
    /// Action requires the invoice to be in Defaulted state.
    NotDefaulted = 19,
    // ── Issue #34: LP priority queue ──────────────────────────────
    /// LP has already joined the fund queue for this invoice.
    AlreadyInQueue = 20,
    /// fund_invoice rejected because a different LP was selected by the priority queue.
    NotApprovedFunder = 21,
    /// Invoice is in Appealed state and cannot be acted upon yet.
    InvoiceAppealed = 22,
    AlreadyDisputed = 23,
    NotDisputed = 24,
    InvoiceDisputed = 25,
    ContractPaused = 26,
    DueDateTooSoon = 27,
    DueDateTooFar = 28,
    SelfInvoice = 29,
    OverpaymentRejected = 30,
    /// Issue #28: payer's reputation is below the configured minimum threshold.
    PayerReputationTooLow = 31,
    ArithmeticOverflow = 32,
    /// Token charges a fee during `transfer`, causing the received amount to differ
    /// from the amount sent and breaking ILN accounting.
    FeeOnTransferToken = 33,
    /// Issue #92: oracle returned unverified for the invoice payer when
    /// require_oracle_verification was set to true.
    PayerUnverified = 33,
    /// Issue #93: oracle data is older than max_oracle_age_ledgers and must
    /// be rejected to prevent stale-data attacks.
    OracleDataStale = 34,
    // ── Invoice NFT Operations ──────────────────────────────────
    /// NFT already exists for this invoice (should not happen in normal flow).
    InvoiceNftAlreadyExists = 35,
    /// NFT does not exist for the requested invoice.
    InvoiceNftNotFound = 36,
    /// Caller is not the owner of the NFT.
    InvoiceNftNotOwned = 37,
    // ── Issue #122: LP Whitelist ────────────────────────────────
    /// LP is not in the whitelist for this private invoice.
    LPNotWhitelisted = 38,
    /// Whitelist exceeds maximum size (10 addresses).
    WhitelistTooLarge = 39,
    // ── Issue #124: Multi-sig Admin ────────────────────────────
    /// Caller is not an authorized multisig signer.
    NotAuthorizedSigner = 40,
    /// Proposal does not exist.
    ProposalNotFound = 41,
    /// Signer has already approved this proposal.
    AlreadySigned = 42,
    /// Proposal has expired and cannot be executed.
    ProposalExpired = 43,
    /// Threshold of signatures has not been reached.
    ThresholdNotReached = 44,
    /// Proposal has already been executed.
    ProposalAlreadyExecuted = 45,
    /// Invalid multisig configuration (e.g., threshold > signer count).
    InvalidMultisigConfig = 46,
}
