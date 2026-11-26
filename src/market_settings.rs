//! A live market's settings sequence, and the admin quorum's
//! `UpdateMarketFees` payload, which names the sequence it was proposed
//! against.
//!
//! A quorum change executes at its last approval, possibly days after it was
//! proposed, and writes absolute values. It names the market's settings
//! sequence at proposal, so a change made in between is never silently
//! reverted.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::types::{
    ExecError, FeeTier, FundingDeadBandBps, FundingInterestRate, MarkSourceMode, MarketId,
    UpdateMarketFees,
};

/// The settings changes recorded for a market since the sequence was
/// introduced: every `UpdateMarketFees` that changes at least one of its
/// settings, by one key or by the admin quorum, adds one. A market with none
/// is at 0, so a market that existed before reads 0 whatever its history.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct MarketSettingsSequence(pub u64);

impl MarketSettingsSequence {
    /// The sequence after one more settings change.
    pub fn next(self) -> Result<Self, ExecError> {
        self.0.checked_add(1).map(Self).ok_or(ExecError::Overflow)
    }
}

impl fmt::Display for MarketSettingsSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Payload of [`crate::types::AdminAction::UpdateMarketFees`]: applies
/// `change` to its market only while the market is at the settings sequence
/// `predecessor`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuorumUpdateMarketFees {
    /// The market's settings sequence when the change was proposed.
    pub predecessor: MarketSettingsSequence,
    /// The settings to change. The quorum is the authorization, so the
    /// change names no signer.
    pub change: MarketSettingsChange,
}

/// The settings one change sets on `market`: every field of
/// [`UpdateMarketFees`] but its `signer`, in the same order. Each field means
/// what the field of the same name there means; `None` leaves that setting as
/// it is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketSettingsChange {
    pub market: MarketId,
    #[serde(default)]
    pub taker_fee_bps: Option<u32>,
    #[serde(default)]
    pub maker_fee_bps: Option<u32>,
    #[serde(default)]
    pub max_funding_rate_bps: Option<u32>,
    #[serde(default)]
    pub funding_interval_ms: Option<u64>,
    #[serde(default)]
    pub max_position_size: Option<u64>,
    #[serde(default)]
    pub default_ttl_ms: Option<u64>,
    #[serde(default)]
    pub net_delta_margin: Option<bool>,
    #[serde(default)]
    pub tick_size: Option<u64>,
    #[serde(default)]
    pub lot_size: Option<u64>,
    #[serde(default)]
    pub primary_oracle_signer: Option<[u8; 20]>,
    #[serde(default)]
    pub oracle_staleness_ms: Option<u64>,
    #[serde(default)]
    pub mark_source_mode: Option<MarkSourceMode>,
    #[serde(default)]
    pub max_mark_spread_bps: Option<u32>,
    #[serde(default)]
    pub cex_composite_staleness_ms: Option<u64>,
    #[serde(default)]
    pub fee_tiers: Option<Vec<FeeTier>>,
    #[serde(default)]
    pub im_bps: Option<u32>,
    #[serde(default)]
    pub mm_bps: Option<u32>,
    #[serde(default)]
    pub max_open_interest: Option<u64>,
    #[serde(default)]
    pub impact_notional: Option<u64>,
    pub funding_dead_band_bps: Option<FundingDeadBandBps>,
    pub funding_interest_rate: Option<FundingInterestRate>,
}

impl UpdateMarketFees {
    /// The settings this action changes, without the key that signs it.
    pub fn into_change(self) -> MarketSettingsChange {
        // Every field is named, so a field added to `UpdateMarketFees` does
        // not compile until it is placed in the change.
        let UpdateMarketFees {
            market,
            signer: _,
            taker_fee_bps,
            maker_fee_bps,
            max_funding_rate_bps,
            funding_interval_ms,
            max_position_size,
            default_ttl_ms,
            net_delta_margin,
            tick_size,
            lot_size,
            primary_oracle_signer,
            oracle_staleness_ms,
            mark_source_mode,
            max_mark_spread_bps,
            cex_composite_staleness_ms,
            fee_tiers,
            im_bps,
            mm_bps,
            max_open_interest,
            impact_notional,
            funding_dead_band_bps,
            funding_interest_rate,
        } = self;
        MarketSettingsChange {
            market,
            taker_fee_bps,
            maker_fee_bps,
            max_funding_rate_bps,
            funding_interval_ms,
            max_position_size,
            default_ttl_ms,
            net_delta_margin,
            tick_size,
            lot_size,
            primary_oracle_signer,
            oracle_staleness_ms,
            mark_source_mode,
            max_mark_spread_bps,
            cex_composite_staleness_ms,
            fee_tiers,
            im_bps,
            mm_bps,
            max_open_interest,
            impact_notional,
            funding_dead_band_bps,
            funding_interest_rate,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// The encoding of a `signer` field.
    #[derive(Serialize)]
    struct Signer(#[serde(with = "crate::wire_bytes")] [u8; 20]);

    /// A MessagePack array16 header for `len` elements.
    fn array16(len: u16) -> Vec<u8> {
        let mut header = vec![0xdc];
        header.extend_from_slice(&len.to_be_bytes());
        header
    }

    /// A change encodes as its action with the `signer` element taken out:
    /// same fields, same order, same encoding. Every field holds a distinct
    /// value, so a field placed in the wrong slot changes the bytes.
    #[test]
    fn a_change_encodes_as_its_action_without_the_signer() {
        let update = UpdateMarketFees {
            market: 7,
            signer: [0xEE; 20],
            taker_fee_bps: Some(1),
            maker_fee_bps: Some(2),
            max_funding_rate_bps: Some(3),
            funding_interval_ms: Some(4),
            max_position_size: Some(5),
            default_ttl_ms: Some(6),
            net_delta_margin: Some(true),
            tick_size: Some(8),
            lot_size: Some(9),
            primary_oracle_signer: Some([0x0A; 20]),
            oracle_staleness_ms: Some(11),
            mark_source_mode: Some(MarkSourceMode::Median),
            max_mark_spread_bps: Some(13),
            cex_composite_staleness_ms: Some(14),
            fee_tiers: Some(vec![FeeTier {
                min_30d_volume_micro_usdc: 0,
                maker_fee_tenth_bps: -15,
                taker_fee_tenth_bps: 15,
            }]),
            im_bps: Some(16),
            mm_bps: Some(17),
            max_open_interest: Some(18),
            impact_notional: Some(19),
            funding_dead_band_bps: Some(FundingDeadBandBps(20)),
            funding_interest_rate: Some(FundingInterestRate(-21)),
        };
        let action = rmp_serde::to_vec(&update).expect("the action encodes");
        let change = rmp_serde::to_vec(&update.clone().into_change()).expect("the change encodes");

        let market = rmp_serde::to_vec(&update.market).expect("the market encodes");
        let signer = rmp_serde::to_vec(&Signer(update.signer)).expect("the signer encodes");
        let head = [array16(23), market.clone(), signer].concat();
        assert!(
            action.starts_with(&head),
            "the action is its 23 fields, market then signer first"
        );
        let rest = action
            .get(head.len()..)
            .expect("the action has fields after the signer");
        assert_eq!(change, [array16(22), market, rest.to_vec()].concat());
    }
}
