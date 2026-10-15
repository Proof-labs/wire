//! Micro-USDC scalar — the only money scalar in the bridge. Spec §Collateral.

use borsh::{BorshDeserialize, BorshSerialize};

/// Why a [`MicroUsdc`] operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnitsError {
    /// The result exceeds `u64::MAX` micro-USDC.
    Overflow,
    /// The result would be negative; micro-USDC is unsigned.
    Underflow,
    /// A percentage outside `0..=100` was supplied.
    PercentOutOfRange,
}

/// USDC in micro-USDC (1 USDC = 1_000_000). Inner `u64` private; no
/// `Add`/`Sub` impls — checked arithmetic only.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct MicroUsdc(u64);

impl MicroUsdc {
    /// No money.
    pub const ZERO: Self = Self(0);

    /// The largest representable amount.
    pub const MAX: Self = Self(u64::MAX);

    /// One whole USDC.
    pub const ONE_USDC: Self = Self(1_000_000);

    /// Wrap a raw micro-USDC count.
    #[must_use]
    pub const fn new(micro: u64) -> Self {
        Self(micro)
    }

    /// The raw micro-USDC count, for encoding onto the wire only.
    #[must_use]
    pub const fn to_micro(self) -> u64 {
        self.0
    }

    /// Add, refusing to wrap.
    pub fn checked_add(self, other: Self) -> Result<Self, UnitsError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(UnitsError::Overflow)
    }

    /// Subtract, refusing to go negative.
    pub fn checked_sub(self, other: Self) -> Result<Self, UnitsError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(UnitsError::Underflow)
    }

    /// `percent`% of `self`, floored. Widens to `u128`; flooring shrinks
    /// limits, the safe direction.
    pub fn percent_floor(self, percent: u8) -> Result<Self, UnitsError> {
        match percent {
            0..=100 => {}
            _ => return Err(UnitsError::PercentOutOfRange),
        }
        // Cannot overflow: (u64::MAX as u128) * 100 < u128::MAX, and the
        // divisor is a non-zero constant.
        let scaled = (self.0 as u128)
            .checked_mul(percent as u128)
            .ok_or(UnitsError::Overflow)?
            .checked_div(100)
            .ok_or(UnitsError::Overflow)?;
        // Also cannot fail for percent <= 100, but assert rather than assume.
        u64::try_from(scaled)
            .map(Self)
            .map_err(|_| UnitsError::Overflow)
    }

    /// Lower of `ceiling` and `percent`% of `liabilities`.
    pub fn effective_limit(
        ceiling: Self,
        liabilities: Self,
        percent: u8,
    ) -> Result<Self, UnitsError> {
        Ok(ceiling.min(liabilities.percent_floor(percent)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_the_additive_identity() {
        let a = MicroUsdc::new(1_000_000);
        assert_eq!(
            a.checked_add(MicroUsdc::ZERO)
                .expect("adding zero never overflows"),
            a
        );
    }

    #[test]
    fn addition_that_would_wrap_is_an_error_not_a_wrap() {
        let a = MicroUsdc::new(u64::MAX);
        assert_eq!(a.checked_add(MicroUsdc::new(1)), Err(UnitsError::Overflow));
    }

    #[test]
    fn subtraction_below_zero_is_an_error_not_a_wrap() {
        let a = MicroUsdc::new(5);
        assert_eq!(a.checked_sub(MicroUsdc::new(6)), Err(UnitsError::Underflow));
    }

    #[test]
    fn subtraction_to_exactly_zero_is_allowed() {
        let a = MicroUsdc::new(5);
        assert_eq!(
            a.checked_sub(MicroUsdc::new(5))
                .expect("subtracting to exactly zero is allowed"),
            MicroUsdc::ZERO
        );
    }

    /// Tier limits round down to micro-USDC, so a limit never rounds *up* into
    /// a larger blast radius.
    #[test]
    fn percentage_of_rounds_down() {
        // 10% of 25 micro-USDC is 2.5 -> 2, never 3.
        let liabilities = MicroUsdc::new(25);
        assert_eq!(
            liabilities
                .percent_floor(10)
                .expect("10% is a valid percentage"),
            MicroUsdc::new(2)
        );
    }

    #[test]
    fn percentage_of_a_huge_balance_does_not_overflow_the_intermediate() {
        // u64::MAX * 10 overflows u64; the implementation must widen.
        let liabilities = MicroUsdc::new(u64::MAX);
        assert_eq!(
            liabilities
                .percent_floor(100)
                .expect("100% of u64::MAX fits after widening"),
            MicroUsdc::new(u64::MAX)
        );
    }

    #[test]
    fn percentage_above_one_hundred_is_rejected() {
        // A tier limit expressed as >100% of liabilities is a config bug, and
        // silently accepting it would let the effective limit exceed custody.
        assert_eq!(
            MicroUsdc::new(10).percent_floor(101),
            Err(UnitsError::PercentOutOfRange)
        );
    }

    /// Spec: "the effective limit is the lower value".
    #[test]
    fn effective_limit_is_the_lower_of_ceiling_and_percentage() {
        let liabilities = MicroUsdc::new(1_000_000); // 1 USDC
        let ceiling = MicroUsdc::new(25_000);
        // 10% of liabilities = 100_000 > ceiling, so the ceiling binds.
        assert_eq!(
            MicroUsdc::effective_limit(ceiling, liabilities, 10)
                .expect("10% is a valid percentage"),
            ceiling
        );
        // With tiny liabilities the percentage binds instead.
        assert_eq!(
            MicroUsdc::effective_limit(ceiling, MicroUsdc::new(1_000), 10)
                .expect("10% is a valid percentage"),
            MicroUsdc::new(100)
        );
    }
}
