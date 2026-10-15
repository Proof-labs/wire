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

/// Deprecated: every market has triggers and the engine refuses this action.
/// The type is unchanged so the admin action table and existing proposals
/// still decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTriggerMarketConfig {
    pub market: MarketId,
    /// Deprecated: ignored.
    pub expected_current_version: Option<TriggerConfigVersion>,
    /// Deprecated: ignored. Triggers are always on.
    pub enabled: bool,
    /// Deprecated: ignored. The cap is `TRIGGER_SLIPPAGE_CEILING_BPS`.
    pub max_trigger_slippage_bps: TriggerSlippageBps,
    /// Deprecated: ignored. The market's own oracle age guard applies.
    pub max_mark_age_ms: TriggerMarkMaxAgeMs,
    /// Deprecated: ignored.
    pub max_future_publish_skew_ms: TriggerFutureSkewMs,
    /// Deprecated: ignored. There is no per-market bracket limit.
    pub max_active_brackets: TriggerBracketLimit,
}
