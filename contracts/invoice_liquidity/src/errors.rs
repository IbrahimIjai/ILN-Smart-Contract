use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    Unauthorized = 1,
    AlreadyInitialized = 2,
    InvalidAmount = 3,
    InvalidDiscountRate = 4,
    InvalidDueDate = 5,
    InvoiceNotFound = 6,
    AlreadyPaid = 7,
    AlreadyFunded = 8,
    NotYetDefaulted = 9,
    InvoiceDefaulted = 10,
    OverfundingRejected = 11,
    OverpaymentRejected = 12,
    ArithmeticOverflow = 13,
    NothingToClaim = 14,
    SelfInvoice = 15,
    InvoiceExpired = 16,
    BatchTooLarge = 17,
    AlreadyCancelled = 18,
    ContractPaused = 19,
    DueDateTooSoon = 20,
    DueDateTooFar = 21,
    /// LP has already joined the fund queue for this invoice.
    AlreadyInQueue = 22,
    /// fund_invoice rejected because a different LP was selected by the priority queue.
    NotApprovedFunder = 23,
    /// payer's reputation is below the configured minimum threshold.
    PayerReputationTooLow = 24,
    /// Invoice is in Appealed state and cannot be acted upon yet.
    InvoiceAppealed = 25,
    /// Payer attempted to appeal an invoice that is already in Appealed state.
    AlreadyAppealed = 26,
    /// Appeal window has closed; appeal can no longer be submitted.
    AppealWindowClosed = 27,
    /// Action requires the invoice to be in Defaulted state.
    NotDefaulted = 28,
    AlreadyDisputed = 29,
    NotDisputed = 30,
    InvoiceDisputed = 31,
    NotFunded = 32,
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
    /// Reentrancy guard: function called while already executing.
    Reentrancy = 32,
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
    /// Dutch auction: invalid auction parameters (rates or decay)
    InvalidAuctionParams = 35,
    /// Dutch auction: auction has expired and cannot be funded
    AuctionExpired = 36,
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
