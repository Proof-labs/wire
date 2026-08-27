//! Format-aware serialization for byte-array wire fields.
//!
//! Human-readable formats (pythonize, serde_wasm_bindgen, JSON) see a byte
//! string, so a Python `bytes` / JS `Uint8Array` round-trips as-is; compact
//! formats (rmp-serde) see a `seq` of `u8` — byte-identical to a bare
//! `[u8; N]` / `Vec<u8>`, so the on-wire encoding is unchanged. Deserialize
//! accepts either form. This reproduces the old client newtypes' behavior
//! without changing the field type, so engine code is untouched.
//!
//! No endianness, length, or terminator handling here: these are opaque raw
//! bytes, not multi-byte integers, so byte order does not apply. Length is
//! fixed (`[u8; N]`) or framed by the format (msgpack encodes the element count
//! for the seq, the byte count for a bin) — there is no in-band terminator.

use core::fmt;

use serde::de::{Deserializer, Error, SeqAccess, Visitor};
use serde::ser::{SerializeTuple, Serializer};

pub fn serialize<S: Serializer, const N: usize>(v: &[u8; N], s: S) -> Result<S::Ok, S::Error> {
    if s.is_human_readable() {
        s.serialize_bytes(v)
    } else {
        let mut t = s.serialize_tuple(N)?;
        for b in v.iter() {
            t.serialize_element(b)?;
        }
        t.end()
    }
}

struct ArrVisitor<const N: usize>;

impl<'de, const N: usize> Visitor<'de> for ArrVisitor<N> {
    type Value = [u8; N];
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{N} bytes")
    }
    fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        <[u8; N]>::try_from(v).map_err(|_| E::invalid_length(v.len(), &self))
    }
    fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
        self.visit_bytes(&v)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = [0u8; N];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = seq
                .next_element()?
                .ok_or_else(|| A::Error::invalid_length(i, &self))?;
        }
        Ok(out)
    }
}

pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(d: D) -> Result<[u8; N], D::Error> {
    if d.is_human_readable() {
        d.deserialize_any(ArrVisitor::<N>)
    } else {
        d.deserialize_tuple(N, ArrVisitor::<N>)
    }
}

/// Same format-aware behavior for variable-length `Vec<u8>` fields.
pub mod vec {
    use super::*;
    use serde::ser::SerializeSeq;

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_bytes(v)
        } else {
            let mut seq = s.serialize_seq(Some(v.len()))?;
            for b in v.iter() {
                seq.serialize_element(b)?;
            }
            seq.end()
        }
    }

    struct VecVisitor;
    impl<'de> Visitor<'de> for VecVisitor {
        type Value = Vec<u8>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a byte string or seq of u8")
        }
        fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Vec<u8>, E> {
            Ok(v.to_vec())
        }
        fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Vec<u8>, E> {
            Ok(v)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<u8>, A::Error> {
            let mut out = Vec::new();
            while let Some(b) = seq.next_element()? {
                out.push(b);
            }
            Ok(out)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        if d.is_human_readable() {
            d.deserialize_any(VecVisitor)
        } else {
            d.deserialize_seq(VecVisitor)
        }
    }
}
