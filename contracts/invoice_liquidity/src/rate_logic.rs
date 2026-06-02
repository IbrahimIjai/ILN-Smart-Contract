#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RateError {
    ArithmeticUnderflow,
    ArithmeticOverflow,
}

pub fn calculate_effective_rate(
    base_rate_bps: u32,
    reputation_score: u8,
    high_rep_threshold: u8,
    bonus_bps: u32,
    min_discount_rate_bps: u32,
) -> Result<u32, RateError> {
    if reputation_score >= high_rep_threshold {
        let reduced_rate = base_rate_bps.saturating_sub(bonus_bps);
        let effective_rate = core::cmp::max(reduced_rate, min_discount_rate_bps);
        Ok(effective_rate)
    } else {
        let effective_rate = core::cmp::max(base_rate_bps, min_discount_rate_bps);
        Ok(effective_rate)
    }
}

/// Calculate the current Dutch auction rate based on time elapsed.
/// 
/// The rate decreases linearly from start_rate to min_rate over time.
/// Formula: current_rate = start_rate - (hours_elapsed * decay_per_hour)
/// The rate is capped at min_rate.
/// 
/// # Arguments
/// * `current_time` - Current timestamp in seconds
/// * `auction_started_at` - Timestamp when auction started in seconds
/// * `start_rate` - Starting rate in basis points
/// * `min_rate` - Minimum rate in basis points
/// * `decay_per_hour` - Rate decay per hour in basis points
/// 
/// # Returns
/// The current auction rate in basis points
pub fn calculate_auction_rate(
    current_time: u64,
    auction_started_at: u64,
    start_rate: u32,
    min_rate: u32,
    decay_per_hour: u32,
) -> u32 {
    // Calculate hours elapsed (rounded down)
    let seconds_elapsed = current_time.saturating_sub(auction_started_at);
    let hours_elapsed = seconds_elapsed / 3600; // 1 hour = 3600 seconds
    
    // Calculate total decay
    let total_decay = hours_elapsed.saturating_mul(decay_per_hour as u64) as u32;
    
    // Calculate current rate, but not below min_rate
    let current_rate = start_rate.saturating_sub(total_decay);
    core::cmp::max(current_rate, min_rate)
}
