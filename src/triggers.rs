//! Trigger-policy wire payloads shared with the client SDK.
//!
//! Only the client-facing wire structs live here: the admin
//! `SetTriggerMarketConfig` payload and the config newtypes it carries.
//! Trigger execution, stored state, and migration live in exchange-core's
//! `triggers` / `trigger_*` modules.

use serde::{Deserialize, Serialize};

use crate::types::{MarketId, TriggerSlippageBps};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TriggerConfigVersion(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TriggerMarkMaxAgeMs(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TriggerFutureSkewMs(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TriggerBracketLimit(pub u64);

/// Multisig-controlled replacement for one market's complete trigger policy.
/// The engine assigns the next version and schedules it for the next block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTriggerMarketConfig {
    pub market: MarketId,
    pub expected_current_version: Option<TriggerConfigVersion>,
    pub enabled: bool,
    pub max_trigger_slippage_bps: TriggerSlippageBps,
    pub max_mark_age_ms: TriggerMarkMaxAgeMs,
    pub max_future_publish_skew_ms: TriggerFutureSkewMs,
    pub max_active_brackets: TriggerBracketLimit,
}
