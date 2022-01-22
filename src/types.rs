/// This module provides two (de)serializers for frequently used types in blockchain.
/// - `biguint` for `num_bigint::BigUint`
/// - `byte_array` for `[u8; N]`
/// 
/// Put `#[serde(with = "biguint")]` or `#[serde(with = "byte_array")]` before your 
/// struct **field** to use them.


pub mod biguint {
    use num_bigint::BigUint;
    use serde::{Serializer, Deserializer};
    use serde_bytes;
    
    pub fn serialize<S>(bn: &BigUint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = bn.to_bytes_be();
        // trim the zero
        if bytes.len() == 1 && bytes[0] == 0 {
            bytes.pop();
        }
        serde_bytes::serialize(&bytes, serializer)
    }

    /// This takes the result of [`serde_bytes::deserialize`] from `[u8]` to `[u8; N]`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<BigUint, D::Error>
    where
        D: Deserializer<'de>,
    {
        let slice: &[u8] = serde_bytes::deserialize(deserializer)?;
        Ok(BigUint::from_bytes_be(slice))
    }
}

/// See <https://github.com/serde-rs/bytes/issues/26>
/// We have to manually implement serialize and deserialize 
/// until specification is supported in rust
pub mod byte_array {
    use core::convert::TryInto;

    use serde::de::Error;
    use serde::{Deserializer, Serializer};

    /// This just specializes [`serde_bytes::serialize`] to `<T = [u8]>`.
    pub fn serialize<S>(key: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde_bytes::serialize(key, serializer)
    }

    /// This takes the result of [`serde_bytes::deserialize`] from `[u8]` to `[u8; N]`.
    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        let slice: &[u8] = serde_bytes::deserialize(deserializer)?;
        slice.try_into().map_err(|_| {
            let expected = format!("[u8; {}]", N);
            D::Error::invalid_length(slice.len(), &expected.as_str())
        })
    }
}
