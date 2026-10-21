//! Explicit, governed partial-margin offsets between a perpetual and its
//! conditional.
//!
//! A policy is one stored record with a compare-and-set sequence: a quorum
//! writes a complete replacement naming the sequence it replaces. An absent
//! record means no offsets, and nothing is credited.
//!
//! One pair lists the two books that may offset, the market whose certified
//! price is their shared settlement reference, the fraction of the matched
//! legs' combined margin charge the offset credits, and what happens at
//! resolution. Every field is engine-verifiable except the economic rate,
//! which governance owns.
//!
//! The pair record carries the resolution treatment; lookups return it, so no
//! consumer infers it. Pair identity and order are canonical, and a decoded
//! collection is re-validated, so a stored record has exactly one accepted
//! shape.

use core::{fmt, mem::size_of, num::NonZeroU64};

use serde::{Deserialize, Serialize};

use crate::types::{MarketId, MAX_ADMIN_ACTION_BYTES};

// Array marker, market ids, rate, and the resolution-treatment string.
const MAX_ENCODED_PAIR_BYTES: usize =
    1 + 3 * (1 + size_of::<MarketId>()) + (1 + size_of::<u16>()) + 1 + "RemoveOnResolution".len();
// Enum map/tag, payload array, predecessor, and pair-list array16 header.
const MAX_ENCODED_SET_OVERHEAD: usize =
    1 + 1 + "SetMarginOffsetPolicy".len() + 1 + 1 + size_of::<u64>() + 3;

/// Pair capacity that fits the governance action budget at maximum field widths.
pub const MAX_MARGIN_OFFSET_PAIRS: usize =
    (MAX_ADMIN_ACTION_BYTES - MAX_ENCODED_SET_OVERHEAD) / MAX_ENCODED_PAIR_BYTES;

/// The smallest partial rate, in basis points. Below it an offset credits
/// nothing.
const MIN_PARTIAL_OFFSET_BPS: u16 = 1;

/// Full netting, in basis points. It is not a partial rate.
const FULL_NETTING_BPS: u16 = 10_000;

/// Fraction of the matched legs' combined margin charge an offset credits.
///
/// Strictly between zero and full netting: 0 credits nothing and 10,000
/// credits everything, so neither is a partial rate. The bound is enforced
/// when the value is built and again when it is decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct PartialOffsetBps(u16);

impl PartialOffsetBps {
    /// The raw basis points, for charge arithmetic.
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for PartialOffsetBps {
    type Error = InvalidPartialOffsetBps;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if (MIN_PARTIAL_OFFSET_BPS..FULL_NETTING_BPS).contains(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidPartialOffsetBps(value))
        }
    }
}

impl From<PartialOffsetBps> for u16 {
    fn from(value: PartialOffsetBps) -> Self {
        value.0
    }
}

/// Why a value is not a usable partial offset rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidPartialOffsetBps(pub u16);

impl core::fmt::Display for InvalidPartialOffsetBps {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "partial offset rate {} must be in {MIN_PARTIAL_OFFSET_BPS}..{FULL_NETTING_BPS} basis points",
            self.0
        )
    }
}

/// Position of a policy in its update sequence.
///
/// Every stored policy has one and every update names the one it replaces.
/// Absence is represented by `Option<MarginOffsetSequence>` at the storage
/// boundary, never by a zero sequence, so a value at rest is always a real
/// position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct MarginOffsetSequence(NonZeroU64);

impl MarginOffsetSequence {
    /// The raw sequence number.
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// The sequence that replaces this one, when the counter can advance.
    pub fn checked_next(self) -> Option<Self> {
        self.0
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
    }
}

impl TryFrom<u64> for MarginOffsetSequence {
    type Error = InvalidMarginOffsetSequence;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(InvalidMarginOffsetSequence(value))
    }
}

impl From<MarginOffsetSequence> for u64 {
    fn from(value: MarginOffsetSequence) -> Self {
        value.0.get()
    }
}

/// Why a value is not a usable sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMarginOffsetSequence(pub u64);

impl core::fmt::Display for InvalidMarginOffsetSequence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "margin offset sequence {} must be non-zero", self.0)
    }
}

/// Position of a pair in a policy's canonical list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OffsetPairIndex(pub u16);

impl OffsetPairIndex {
    /// The index of the pair at `position`, when the position fits the index.
    pub fn from_position(position: usize) -> Option<Self> {
        u16::try_from(position).ok().map(Self)
    }
}

impl core::fmt::Display for OffsetPairIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Display for MarginOffsetSequence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get().fmt(f)
    }
}

impl core::fmt::Display for PartialOffsetBps {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get().fmt(f)
    }
}

/// Number of pairs in a policy replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct MarginOffsetPairCount(u16);

impl TryFrom<u16> for MarginOffsetPairCount {
    type Error = MarginOffsetPolicyError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if usize::from(value) > MAX_MARGIN_OFFSET_PAIRS {
            return Err(MarginOffsetPolicyError::TooManyPairs {
                actual: usize::from(value),
                max: MAX_MARGIN_OFFSET_PAIRS,
            });
        }
        Ok(Self(value))
    }
}

impl From<MarginOffsetPairCount> for u16 {
    fn from(count: MarginOffsetPairCount) -> Self {
        count.0
    }
}

impl core::fmt::Display for MarginOffsetPairCount {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

/// The perpetual side of one eligible pair, so pair identities cannot be
/// confused with the conditional or reference market at a call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PerpetualMarket(pub MarketId);

/// The conditional side of one eligible pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConditionalMarket(pub MarketId);

/// The market whose engine-certified price is both legs' shared settlement
/// reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SettlementReferenceMarket(pub MarketId);

impl core::fmt::Display for PerpetualMarket {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self(market) = self;
        market.fmt(f)
    }
}

impl core::fmt::Display for ConditionalMarket {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self(market) = self;
        market.fmt(f)
    }
}

impl core::fmt::Display for SettlementReferenceMarket {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self(market) = self;
        market.fmt(f)
    }
}

/// What happens to a pair's credit when its conditional resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffsetResolutionTreatment {
    /// The credit is removed: the offset prices the pre-resolution hedge, and
    /// a resolved conditional's margin collapses under the existing rule.
    RemoveOnResolution,
}

impl core::fmt::Display for OffsetResolutionTreatment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RemoveOnResolution => f.write_str("remove_on_resolution"),
        }
    }
}

/// One eligible perpetual-and-conditional pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarginOffsetPair {
    pub perpetual: PerpetualMarket,
    pub conditional: ConditionalMarket,
    pub settlement_reference: SettlementReferenceMarket,
    pub rate: PartialOffsetBps,
    pub resolution: OffsetResolutionTreatment,
}

/// A policy's eligible pairs in canonical order.
///
/// The only way to build one is [`TryFrom<Vec<MarginOffsetPair>>`], which
/// enforces the bound and the order: conditionals strictly ascend and never
/// repeat, and every pair names its own perpetual as the settlement
/// reference. Ascending conditional market id is the allocation priority when
/// several pairs could match, so the order is part of the contract and a
/// decoded list that is not canonical is refused.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(into = "Vec<MarginOffsetPair>")]
pub struct MarginOffsetPairs(Vec<MarginOffsetPair>);

struct MarginOffsetPairsVisitor;

impl<'de> serde::de::Visitor<'de> for MarginOffsetPairsVisitor {
    type Value = MarginOffsetPairs;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_MARGIN_OFFSET_PAIRS} canonical offset pairs"
        )
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut sequence: A,
    ) -> Result<Self::Value, A::Error> {
        if sequence
            .size_hint()
            .is_some_and(|length| length > MAX_MARGIN_OFFSET_PAIRS)
        {
            return Err(serde::de::Error::custom(
                "margin offset pair count exceeds the policy bound",
            ));
        }
        let mut pairs = Vec::with_capacity(MAX_MARGIN_OFFSET_PAIRS);
        while let Some(pair) = sequence.next_element::<MarginOffsetPair>()? {
            if pairs.len() == MAX_MARGIN_OFFSET_PAIRS {
                return Err(serde::de::Error::custom(
                    "margin offset pair count exceeds the policy bound",
                ));
            }
            pairs.push(pair);
        }
        MarginOffsetPairs::try_from(pairs).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for MarginOffsetPairs {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(MarginOffsetPairsVisitor)
    }
}

impl MarginOffsetPairs {
    /// The pairs in canonical order, ascending by conditional market.
    pub fn iter(&self) -> core::slice::Iter<'_, MarginOffsetPair> {
        self.0.iter()
    }

    /// The number of pairs.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the policy lists no pairs.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The pair at `index` in canonical order.
    pub fn get(&self, index: OffsetPairIndex) -> Option<&MarginOffsetPair> {
        self.0.get(usize::from(index.0))
    }

    /// The pair offsetting `conditional`, when the policy lists one.
    pub fn pair_for(&self, conditional: MarketId) -> Option<&MarginOffsetPair> {
        self.0
            .binary_search_by_key(&conditional, |pair| pair.conditional.0)
            .ok()
            .and_then(|position| self.0.get(position))
    }
}

impl TryFrom<Vec<MarginOffsetPair>> for MarginOffsetPairs {
    type Error = MarginOffsetPolicyError;

    fn try_from(pairs: Vec<MarginOffsetPair>) -> Result<Self, Self::Error> {
        if pairs.len() > MAX_MARGIN_OFFSET_PAIRS {
            return Err(MarginOffsetPolicyError::TooManyPairs {
                actual: pairs.len(),
                max: MAX_MARGIN_OFFSET_PAIRS,
            });
        }
        let mut previous: Option<ConditionalMarket> = None;
        for (position, pair) in pairs.iter().enumerate() {
            let Some(at) = OffsetPairIndex::from_position(position) else {
                return Err(MarginOffsetPolicyError::TooManyPairs {
                    actual: pairs.len(),
                    max: MAX_MARGIN_OFFSET_PAIRS,
                });
            };
            if pair.perpetual.0 == pair.conditional.0 {
                return Err(MarginOffsetPolicyError::SameMarket {
                    at,
                    market: pair.perpetual,
                });
            }
            if pair.settlement_reference.0 != pair.perpetual.0 {
                return Err(MarginOffsetPolicyError::ReferenceNotPerpetual {
                    at,
                    perpetual: pair.perpetual,
                    reference: pair.settlement_reference,
                });
            }
            if let Some(previous_market) = previous {
                if pair.conditional == previous_market {
                    return Err(MarginOffsetPolicyError::DuplicateConditional {
                        at,
                        market: pair.conditional,
                    });
                }
                if pair.conditional < previous_market {
                    return Err(MarginOffsetPolicyError::OutOfOrderConditional {
                        at,
                        previous: previous_market,
                        market: pair.conditional,
                    });
                }
            }
            previous = Some(pair.conditional);
        }
        Ok(Self(pairs))
    }
}

impl From<MarginOffsetPairs> for Vec<MarginOffsetPair> {
    fn from(pairs: MarginOffsetPairs) -> Self {
        pairs.0
    }
}

/// The rule a pair list breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginOffsetPolicyError {
    /// More pairs than a policy may list.
    TooManyPairs { actual: usize, max: usize },
    /// `market` appears as the conditional of the pair at `at` and of the one
    /// before it.
    DuplicateConditional {
        at: OffsetPairIndex,
        market: ConditionalMarket,
    },
    /// The conditional at `at` is below the preceding conditional.
    OutOfOrderConditional {
        at: OffsetPairIndex,
        previous: ConditionalMarket,
        market: ConditionalMarket,
    },
    /// The pair at `at` names one market on both sides.
    SameMarket {
        at: OffsetPairIndex,
        market: PerpetualMarket,
    },
    /// The pair at `at` settles against a market that is not its perpetual.
    ReferenceNotPerpetual {
        at: OffsetPairIndex,
        perpetual: PerpetualMarket,
        reference: SettlementReferenceMarket,
    },
}

impl core::fmt::Display for MarginOffsetPolicyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooManyPairs { actual, max } => {
                write!(f, "margin offset pair count {actual} exceeds {max}")
            }
            Self::DuplicateConditional { at, market } => {
                write!(f, "pair {at} repeats conditional market {}", market.0)
            }
            Self::OutOfOrderConditional {
                at,
                previous,
                market,
            } => write!(
                f,
                "pair {at} conditional market {} must exceed the previous conditional market {}",
                market.0, previous.0
            ),
            Self::SameMarket { at, market } => write!(
                f,
                "pair {at} names market {} as both perpetual and conditional",
                market.0
            ),
            Self::ReferenceNotPerpetual {
                at,
                perpetual,
                reference,
            } => write!(
                f,
                "pair {at} settlement reference {} must be its perpetual market {}",
                reference.0, perpetual.0
            ),
        }
    }
}

/// The stored record: the policy's sequence and its canonical pairs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarginOffsetPolicy {
    pub sequence: MarginOffsetSequence,
    pub pairs: MarginOffsetPairs,
}

/// Payload of [`crate::types::AdminAction::SetMarginOffsetPolicy`]: replaces
/// the stored policy named by `predecessor`.
///
/// The payload carries no successor sequence: the engine assigns the
/// predecessor's checked successor at execution, so a caller cannot choose it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMarginOffsetPolicy {
    /// Sequence being replaced; `None` when no policy is stored yet.
    pub predecessor: Option<MarginOffsetSequence>,
    pub pairs: MarginOffsetPairs,
}

#[cfg(test)]
#[path = "margin_offsets_tests.rs"]
mod tests;
