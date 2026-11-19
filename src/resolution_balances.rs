//! Exact cash observations around an event's resolution.

use serde::{Deserialize, Serialize};

/// Height of a committed block whose account state was observed.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, derive_more::Display,
)]
#[serde(transparent)]
pub struct BalanceHeight(pub u64);

/// An account's cash balance in whole micro-USDC, excluding unrealized value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
#[serde(transparent)]
pub struct AccountCash(pub u64);
