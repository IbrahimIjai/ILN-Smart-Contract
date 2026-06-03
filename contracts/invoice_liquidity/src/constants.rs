pub const MAX_DISCOUNT_RATE: u32 = 5000;
pub const DEFAULT_PAYER_SCORE: u32 = 50;
pub const DEFAULT_LP_SCORE: u32 = 50;
pub const TOP_PAYERS_CAPACITY: u32 = 50;
/// Maximum number of invoices accepted in a single `submit_invoices_batch`
/// call (Issue #120). Bounds per-transaction storage/compute work.
pub const MAX_BATCH_SIZE: u32 = 50;
