#![allow(clippy::arithmetic_side_effects, clippy::expect_used)]

use super::*;

#[test]
fn typed_event_fields_preserve_the_scalar_wire_encoding() {
    let sequence = MarginOffsetSequence::try_from(1).expect("nonzero sequence");
    let count = MarginOffsetPairCount::try_from(1).expect("bounded pair count");
    let index = OffsetPairIndex(0);
    let rate = PartialOffsetBps::try_from(5_000).expect("partial rate");
    let typed = (
        sequence,
        count,
        index,
        PerpetualMarket(1),
        ConditionalMarket(2),
        SettlementReferenceMarket(1),
        rate,
    );
    let scalars = (1_u64, 1_u32, 0_u32, 1_u32, 2_u32, 1_u32, 5_000_u32);
    let encoded = rmp_serde::to_vec(&typed).expect("typed fields encode");
    assert_eq!(
        encoded,
        rmp_serde::to_vec(&scalars).expect("scalar fields encode")
    );
    let decoded: (
        MarginOffsetSequence,
        MarginOffsetPairCount,
        OffsetPairIndex,
        PerpetualMarket,
        ConditionalMarket,
        SettlementReferenceMarket,
        PartialOffsetBps,
    ) = rmp_serde::from_slice(&encoded).expect("typed fields decode");
    assert_eq!(decoded, typed);
}

#[test]
fn pair_count_decode_enforces_the_policy_bound() {
    let zero = MarginOffsetPairCount::try_from(0).expect("revocation has zero pairs");
    assert_eq!(zero.to_string(), "0");
    let maximum = u16::try_from(MAX_MARGIN_OFFSET_PAIRS).expect("pair capacity fits count type");
    assert!(MarginOffsetPairCount::try_from(maximum).is_ok());
    let excessive = rmp_serde::to_vec(&(maximum + 1)).expect("raw pair count encodes");
    assert!(rmp_serde::from_slice::<MarginOffsetPairCount>(&excessive).is_err());
}

fn pair(perpetual: MarketId, conditional: MarketId) -> MarginOffsetPair {
    MarginOffsetPair {
        perpetual: PerpetualMarket(perpetual),
        conditional: ConditionalMarket(conditional),
        settlement_reference: SettlementReferenceMarket(perpetual),
        rate: PartialOffsetBps::try_from(5_000).expect("5000 is a valid partial rate"),
        resolution: OffsetResolutionTreatment::RemoveOnResolution,
    }
}

fn canonical(pairs: Vec<MarginOffsetPair>) -> MarginOffsetPairs {
    MarginOffsetPairs::try_from(pairs).expect("helper pairs are canonical")
}

fn sequence(value: u64) -> MarginOffsetSequence {
    MarginOffsetSequence::try_from(value).expect("helper sequence is non-zero")
}

#[test]
fn partial_rate_admits_only_strictly_partial_values() {
    assert!(
        PartialOffsetBps::try_from(0).is_err(),
        "0 bps credits nothing"
    );
    assert!(
        PartialOffsetBps::try_from(10_000).is_err(),
        "10000 bps is full netting, not a partial rate"
    );
    assert_eq!(
        PartialOffsetBps::try_from(1)
            .expect("1 bps is a partial rate")
            .get(),
        1
    );
    assert_eq!(
        PartialOffsetBps::try_from(9_999)
            .expect("9999 bps is a partial rate")
            .get(),
        9_999
    );
}

#[test]
fn partial_rate_rejects_out_of_range_bytes() {
    let zero = [0x00_u8];
    assert!(
        rmp_serde::from_slice::<PartialOffsetBps>(&zero).is_err(),
        "a decoded 0 bps must fail the rate bound"
    );

    let full_netting = [0xcd_u8, 0x27, 0x10];
    assert!(
        rmp_serde::from_slice::<PartialOffsetBps>(&full_netting).is_err(),
        "a decoded 10000 bps must fail the rate bound"
    );

    let partial = [0xcd_u8, 0x13, 0x88];
    assert_eq!(
        rmp_serde::from_slice::<PartialOffsetBps>(&partial).expect("5000 bps decodes"),
        PartialOffsetBps::try_from(5_000).expect("5000 is a valid partial rate")
    );
}

#[test]
fn partial_rate_error_carries_the_rejected_value() {
    assert_eq!(
        PartialOffsetBps::try_from(0),
        Err(InvalidPartialOffsetBps(0))
    );
    assert_eq!(
        PartialOffsetBps::try_from(10_000),
        Err(InvalidPartialOffsetBps(10_000))
    );
}

#[test]
fn sequence_is_non_zero_and_advances() {
    assert!(
        MarginOffsetSequence::try_from(0).is_err(),
        "zero is not a sequence"
    );
    let first = sequence(1);
    assert_eq!(first.get(), 1);
    assert_eq!(
        first.checked_next().expect("1 has a successor").get(),
        2,
        "the successor is the checked next value"
    );
    let last = sequence(u64::MAX);
    assert!(
        last.checked_next().is_none(),
        "the last sequence has no successor"
    );
}

#[test]
fn sequence_rejects_zero_bytes() {
    let zero = [0x00_u8];
    assert!(
        rmp_serde::from_slice::<MarginOffsetSequence>(&zero).is_err(),
        "a decoded sequence must be non-zero"
    );

    let one = [0x01_u8];
    assert_eq!(
        rmp_serde::from_slice::<MarginOffsetSequence>(&one).expect("sequence 1 decodes"),
        sequence(1)
    );
}

#[test]
fn offset_pair_index_displays_its_position() {
    assert_eq!(OffsetPairIndex(7).to_string(), "7");
}

#[test]
fn resolution_treatment_names_the_removal() {
    assert_eq!(
        OffsetResolutionTreatment::RemoveOnResolution.to_string(),
        "remove_on_resolution"
    );
}

#[test]
fn market_newtypes_encode_as_their_market_id() {
    for market in [MarketId::MIN, 1, u32::MAX] {
        assert_eq!(
            rmp_serde::to_vec(&PerpetualMarket(market)).expect("perpetual encodes"),
            rmp_serde::to_vec(&market).expect("market id encodes"),
            "the perpetual newtype is wire-transparent"
        );
        assert_eq!(
            rmp_serde::to_vec(&ConditionalMarket(market)).expect("conditional encodes"),
            rmp_serde::to_vec(&market).expect("market id encodes"),
            "the conditional newtype is wire-transparent"
        );
        assert_eq!(
            rmp_serde::to_vec(&SettlementReferenceMarket(market))
                .expect("settlement reference encodes"),
            rmp_serde::to_vec(&market).expect("market id encodes"),
            "the settlement reference newtype is wire-transparent"
        );
    }
}

#[test]
fn empty_policy_lists_nothing() {
    let pairs = MarginOffsetPairs::default();
    assert_eq!(pairs.len(), 0);
    assert!(pairs.is_empty());
    assert!(
        pairs.pair_for(7).is_none(),
        "an empty policy offsets nothing"
    );
    assert_eq!(pairs.iter().count(), 0);
}

#[test]
fn pair_for_finds_a_listed_conditional() {
    let pairs = canonical(vec![pair(1, 3), pair(2, 5), pair(4, 9)]);
    let found = pairs.pair_for(5).expect("5 is a listed conditional");
    assert_eq!(found.perpetual, PerpetualMarket(2));
    assert_eq!(found.conditional, ConditionalMarket(5));
    assert_eq!(
        found.resolution,
        OffsetResolutionTreatment::RemoveOnResolution,
        "the lookup returns the treatment the pair carries"
    );
    assert!(
        pairs.pair_for(4).is_none(),
        "4 is a perpetual, not a listed conditional"
    );
    assert!(pairs.pair_for(10).is_none(), "10 is not listed at all");
}

#[test]
fn pair_count_bound_admits_exactly_the_maximum() {
    let mut pairs = Vec::with_capacity(MAX_MARGIN_OFFSET_PAIRS + 1);
    for i in 0..MAX_MARGIN_OFFSET_PAIRS + 1 {
        let conditional = u32::try_from(i)
            .expect("pair bound fits u32")
            .checked_add(2)
            .expect("bound fits u32");
        pairs.push(pair(1, conditional));
    }

    let at_limit = pairs
        .get(..MAX_MARGIN_OFFSET_PAIRS)
        .expect("the prefix is in range")
        .to_vec();
    assert_eq!(
        MarginOffsetPairs::try_from(at_limit)
            .expect("the bound itself is admitted")
            .len(),
        MAX_MARGIN_OFFSET_PAIRS
    );

    assert_eq!(
        MarginOffsetPairs::try_from(pairs),
        Err(MarginOffsetPolicyError::TooManyPairs {
            actual: MAX_MARGIN_OFFSET_PAIRS + 1,
            max: MAX_MARGIN_OFFSET_PAIRS,
        })
    );
}

#[test]
fn duplicate_conditional_is_named_as_a_duplicate() {
    assert_eq!(
        MarginOffsetPairs::try_from(vec![pair(1, 100), pair(2, 100)]),
        Err(MarginOffsetPolicyError::DuplicateConditional {
            at: OffsetPairIndex(1),
            market: ConditionalMarket(100),
        }),
        "equality is a duplicate, not an ordering fault"
    );
}

#[test]
fn descending_conditional_is_out_of_order() {
    assert_eq!(
        MarginOffsetPairs::try_from(vec![pair(1, 200), pair(2, 100)]),
        Err(MarginOffsetPolicyError::OutOfOrderConditional {
            at: OffsetPairIndex(1),
            previous: ConditionalMarket(200),
            market: ConditionalMarket(100),
        })
    );
}

#[test]
fn pair_must_name_two_different_markets() {
    assert_eq!(
        MarginOffsetPairs::try_from(vec![pair(5, 5)]),
        Err(MarginOffsetPolicyError::SameMarket {
            at: OffsetPairIndex(0),
            market: PerpetualMarket(5),
        })
    );
}

#[test]
fn settlement_reference_must_be_the_perpetual() {
    let mut invalid = pair(1, 2);
    invalid.settlement_reference = SettlementReferenceMarket(9);
    assert_eq!(
        MarginOffsetPairs::try_from(vec![invalid]),
        Err(MarginOffsetPolicyError::ReferenceNotPerpetual {
            at: OffsetPairIndex(0),
            perpetual: PerpetualMarket(1),
            reference: SettlementReferenceMarket(9),
        })
    );
}

#[test]
fn deserialization_revalidates_pair_order() {
    let unordered = vec![pair(1, 200), pair(2, 100)];
    let bytes = rmp_serde::to_vec(&unordered).expect("raw pairs encode");
    assert!(
        rmp_serde::from_slice::<MarginOffsetPairs>(&bytes).is_err(),
        "a decoded descending list must be refused"
    );

    let duplicate = vec![pair(1, 100), pair(2, 100)];
    let bytes = rmp_serde::to_vec(&duplicate).expect("raw pairs encode");
    assert!(
        rmp_serde::from_slice::<MarginOffsetPairs>(&bytes).is_err(),
        "a decoded duplicate conditional must be refused"
    );
}

#[test]
fn deserialization_rejects_an_oversized_pair_list() {
    let pairs: Vec<MarginOffsetPair> = (0..MAX_MARGIN_OFFSET_PAIRS + 1)
        .map(|i| {
            let conditional = u32::try_from(i)
                .expect("pair bound fits u32")
                .checked_add(2)
                .expect("bound fits u32");
            pair(1, conditional)
        })
        .collect();
    let bytes = rmp_serde::to_vec(&pairs).expect("raw pairs encode");
    assert!(
        rmp_serde::from_slice::<MarginOffsetPairs>(&bytes).is_err(),
        "a decoded list above the pair bound must be refused"
    );
}

#[test]
fn deserialization_rejects_a_nested_bad_rate() {
    for rate in [0_u16, 10_000] {
        let raw = vec![(
            1_u32,
            2_u32,
            1_u32,
            rate,
            OffsetResolutionTreatment::RemoveOnResolution,
        )];
        let bytes = rmp_serde::to_vec(&raw).expect("raw pairs encode");
        assert!(
            rmp_serde::from_slice::<Vec<MarginOffsetPair>>(&bytes).is_err(),
            "a nested rate of {rate} must fail the pair's rate bound"
        );
        assert!(
            rmp_serde::from_slice::<MarginOffsetPairs>(&bytes).is_err(),
            "a nested rate of {rate} must fail through the collection"
        );
    }
}

#[test]
fn empty_and_non_empty_policies_round_trip() {
    let empty = MarginOffsetPolicy {
        sequence: sequence(1),
        pairs: MarginOffsetPairs::default(),
    };
    let bytes = rmp_serde::to_vec(&empty).expect("the empty policy encodes");
    assert_eq!(
        rmp_serde::from_slice::<MarginOffsetPolicy>(&bytes).expect("the empty policy decodes"),
        empty
    );

    let mut low = pair(1, 2);
    low.rate = PartialOffsetBps::try_from(1).expect("1 bps is valid");
    let mut high = pair(3, 4);
    high.rate = PartialOffsetBps::try_from(9_999).expect("9999 bps is valid");
    let policy = MarginOffsetPolicy {
        sequence: sequence(1),
        pairs: canonical(vec![low, high]),
    };
    let bytes = rmp_serde::to_vec(&policy).expect("the non-empty policy encodes");
    assert_eq!(
        rmp_serde::from_slice::<MarginOffsetPolicy>(&bytes).expect("the non-empty policy decodes"),
        policy
    );

    for predecessor in [None, Some(sequence(7))] {
        let payload = SetMarginOffsetPolicy {
            predecessor,
            pairs: policy.pairs.clone(),
        };
        let bytes = rmp_serde::to_vec(&payload).expect("the set payload encodes");
        assert_eq!(
            rmp_serde::from_slice::<SetMarginOffsetPolicy>(&bytes)
                .expect("the set payload decodes"),
            payload
        );
    }
}

#[test]
fn frozen_policy_wire_vector() {
    let policy = MarginOffsetPolicy {
        sequence: sequence(1),
        pairs: canonical(vec![pair(1, 2)]),
    };
    // Regenerate with:
    // println!("{}", hex::encode(rmp_serde::to_vec(&policy).expect("policy encodes")));
    let expected = "92019195010201cd1388b252656d6f76654f6e5265736f6c7574696f6e";
    assert_eq!(
        hex::encode(rmp_serde::to_vec(&policy).expect("policy encodes")),
        expected
    );
}

#[test]
fn worst_case_policy_encoding_is_bounded() {
    // Per pair, worst case: array header 1 + three market ids at 5 bytes each
    // (uint32) + one rate at 3 bytes (uint16) + one treatment at 1 + 18 bytes
    // (fixstr) = 38. Policy header, worst case: array marker 1 + sequence 9 +
    // pair-list array16 marker 3 = 13, rounded to 16.
    const PER_PAIR_WORST_CASE_BYTES: usize = 1 + 3 * 5 + 3 + (1 + "RemoveOnResolution".len());
    const POLICY_HEADER_BYTES: usize = 16;
    const MAX_ENCODED_POLICY_BYTES: usize =
        MAX_MARGIN_OFFSET_PAIRS * PER_PAIR_WORST_CASE_BYTES + POLICY_HEADER_BYTES;

    let perpetual = u32::MAX.checked_sub(1_000).expect("perpetual fits u32");
    let count = u32::try_from(MAX_MARGIN_OFFSET_PAIRS).expect("pair bound fits u32");
    let pairs: Vec<MarginOffsetPair> = (0..count)
        .map(|i| {
            let conditional = u32::MAX
                .checked_sub(count)
                .expect("count fits u32")
                .checked_add(1)
                .expect("count fits u32")
                .checked_add(i)
                .expect("index fits u32");
            MarginOffsetPair {
                rate: PartialOffsetBps::try_from(9_999).expect("9999 bps is valid"),
                ..pair(perpetual, conditional)
            }
        })
        .collect();
    let policy = MarginOffsetPolicy {
        sequence: sequence(u64::MAX),
        pairs: canonical(pairs),
    };
    let encoded = rmp_serde::to_vec(&policy).expect("the worst-case policy encodes");
    assert!(
        encoded.len() < MAX_ENCODED_POLICY_BYTES,
        "{} encoded bytes must stay below the {MAX_ENCODED_POLICY_BYTES}-byte bound",
        encoded.len()
    );
    let action = crate::types::AdminAction::SetMarginOffsetPolicy(SetMarginOffsetPolicy {
        predecessor: Some(sequence(u64::MAX - 1)),
        pairs: policy.pairs,
    });
    let action_bytes =
        crate::codec::canonical_admin_action_bytes(&action).expect("maximum policy action encodes");
    assert_eq!(
        action_bytes.len(),
        MAX_ENCODED_SET_OVERHEAD + MAX_MARGIN_OFFSET_PAIRS * MAX_ENCODED_PAIR_BYTES,
        "the wire-size derivation matches the complete admin action"
    );
    assert!(
        action_bytes.len() <= MAX_ADMIN_ACTION_BYTES,
        "the maximum policy must fit governance admission"
    );
    assert!(
        MAX_ADMIN_ACTION_BYTES - action_bytes.len() < MAX_ENCODED_PAIR_BYTES,
        "one more maximum-width pair would exceed the governance budget"
    );
}

#[test]
fn an_oversized_array_header_is_rejected_before_reading_any_pair() {
    // MessagePack array32 with no body: rejection must precede element decode.
    let header = [0xdd, 0xff, 0xff, 0xff, 0xff];
    let error = rmp_serde::from_slice::<MarginOffsetPairs>(&header)
        .expect_err("advertised pair count exceeds the bound");
    assert!(
        error.to_string().contains("pair count exceeds"),
        "must reject the count rather than decode a missing first pair: {error}"
    );
}

struct UnknownLengthPairs {
    encoded_pair: Vec<u8>,
    remaining: usize,
}

impl<'de> serde::de::SeqAccess<'de> for UnknownLengthPairs {
    type Error = serde::de::value::Error;

    fn next_element_seed<T: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut rmp_serde::Deserializer::new(
            self.encoded_pair.as_slice(),
        ))
        .map(Some)
        .map_err(serde::de::Error::custom)
    }
}

#[test]
fn an_unknown_length_sequence_is_stopped_at_the_pair_bound() {
    let mut sequence = UnknownLengthPairs {
        encoded_pair: rmp_serde::to_vec(&pair(1, 2)).expect("pair encodes"),
        remaining: MAX_MARGIN_OFFSET_PAIRS + 2,
    };
    let decoded =
        MarginOffsetPairs::deserialize(serde::de::value::SeqAccessDeserializer::new(&mut sequence));
    let error = decoded.expect_err("unknown-length sequence still has a hard bound");
    assert!(error.to_string().contains("pair count exceeds"));
    assert_eq!(
        sequence.remaining, 1,
        "decoding stops after the first excess element"
    );
}

#[test]
fn the_complete_admin_action_has_a_frozen_vector() {
    assert_eq!(
        crate::types::AdminActionType::SetMarginOffsetPolicy as u8,
        0x1B,
        "the inner tag must not occupy the insurance-account allocation"
    );
    let action = crate::types::AdminAction::SetMarginOffsetPolicy(SetMarginOffsetPolicy {
        predecessor: None,
        pairs: canonical(vec![pair(1, 2)]),
    });
    // Regenerate with: canonical_admin_action_bytes(&action), printed as hex.
    assert_eq!(
        hex::encode(crate::codec::canonical_admin_action_bytes(&action).expect("encodes")),
        "81b55365744d617267696e4f6666736574506f6c69637992c09195010201cd1388b252656d6f76654f6e5265736f6c7574696f6e"
    );
    let revoke = crate::types::AdminAction::SetMarginOffsetPolicy(SetMarginOffsetPolicy {
        predecessor: Some(sequence(1)),
        pairs: canonical(vec![]),
    });
    assert_eq!(
        hex::encode(crate::codec::canonical_admin_action_bytes(&revoke).expect("encodes")),
        "81b55365744d617267696e4f6666736574506f6c696379920190"
    );
}
