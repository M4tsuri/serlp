//! A recursive deserializer, theoritically this is more efficient than the tree 
//! based one, because all data are decoded only when needed and accessed only once.

use serde::{de::{
    self, DeserializeSeed, SeqAccess, Visitor,
}, Deserialize};
use byteorder::{BigEndian, ReadBytesExt};

use crate::{error::{Error, Result}, rlp::RlpTree};
use paste::paste;

pub struct Deserializer<'de> {
    input: &'de [u8]
}

impl<'de> Deserializer<'de> {
    /// Create a deserializer instance from a byte slice, this will covert 
    /// the slice into a tree and store it.
    pub fn new(input: &'de [u8]) -> Self {
        Self { 
            input
        }
    }

    pub fn next_is_bytes(&self) -> bool {
        self.input[0] < 192
    }

    /// return value:
    /// - RLP encoding of the byte slice,
    /// - the byte slice,
    /// - the Deserializer for remaining data
    pub fn next_bytes(&self) -> Result<(&'de [u8], &'de [u8], Self)> {
        let buf = self.input;
        let (start, end) = match buf[0] {
            // R_b(x): ||x|| = 1 \land x[0] \lt 128
            0..=127 => (0, 1),
            // (128 + ||x||) \dot x
            len @ 128..=183 => (1, 1 + (len as usize - 128)),
            // (183 + ||BE(||x||)||) \dot BE(||x||) \dot x
            be_len @ 184..=191 => {
                let be_len = be_len as usize - 183;
                let len = (&buf[1..]).read_uint::<BigEndian>(be_len)
                    .or(Err(Error::MalformedData))? as usize;
                (1 + be_len, 1 + be_len + len)
            },
            _ => Err(Error::MalformedData)?
        };
        Ok((&buf[..end], &buf[start..end], Self::new(&buf[end..])))
    }

    /// return value: 
    /// - RLP encoding of this sequence, 
    /// - the deserializer of this sequence
    /// - the deserializer of remaining data.
    pub fn next_seq(&self) -> Result<(&'de [u8], Self, Self)> {
        let buf = self.input;
        // (192 + ||s(x)||) \dot s(x)
        let (start, end) = match buf[0] {
            len @ 192..=247 => (1, 1 + len as usize - 192),
            be_len @ 248..=255 => {
                let be_len = be_len as usize - 247;
                let len = (&buf[1..]).read_uint::<BigEndian>(be_len)
                    .or(Err(Error::MalformedData))? as usize;
                (1 + be_len, 1 + be_len + len)
            },
            _ => Err(Error::MalformedData)?
        };

        Ok((&buf[..end], Self::new(&buf[start..end]), Self::new(&buf[end..])))
    }
}

macro_rules! impl_deseralize_not_supported {
    ($($ity:ident),+) => {
        paste! {$(
            fn [<deserialize_ $ity>]<V>(self, _visitor: V) -> Result<V::Value>
            where
                V: Visitor<'de>,
            {
                unimplemented!()
            }
        )+}
    }
}

macro_rules! impl_deseralize_integer {
    (@bytes $($ity:ident),+) => {
        paste! {$(
            fn [<deserialize_ $ity>]<V>(self, visitor: V) -> Result<V::Value>
            where
                V: Visitor<'de>,
            {
                let (_, mut bytes, new) = self.next_bytes()?;
                *self = new;
                visitor.[<visit_ $ity>](bytes
                    .[<read_ $ity>]::<BigEndian>()
                    .or(Err(Error::MalformedData))?)
            }
        )+}
    };
    (@single $($ity:ident),+) => {
        paste! {$(
            fn [<deserialize_ $ity>]<V>(self, visitor: V) -> Result<V::Value>
            where
                V: Visitor<'de>,
            {
                let (_, mut bytes, new) = self.next_bytes()?;
                *self = new;
                visitor.[<visit_ $ity>](bytes
                    .[<read_ $ity>]()
                    .or(Err(Error::MalformedData))?)
            }
        )+}
    }
}

/// A proxy for more refined manipulation of data when deserializing. 
/// 
/// Here is an example about how to use it.
/// 
/// ```
///  #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
/// #[serde(from = "RlpProxy")]
/// enum Classify {
///     Zero(u8),
///     One(u8),
///     Ten((u8, u8))
/// }
/// 
/// impl From<RlpProxy> for Classify {
///     fn from(proxy: RlpProxy) -> Self {
///         let raw = proxy.raw();
///         let mut tree = proxy.rlp_tree();
///         if tree.value_count() == 2 {
///             return Classify::Ten(from_bytes(raw).unwrap())
///         }
/// 
///         let val = tree.next().unwrap()[0];
///         match val {
///             0 => Classify::Zero(0),
///             1 => Classify::One(1),
///             _ => panic!("Value Error.")
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlpProxy(Vec<u8>);

impl RlpProxy {
    pub fn raw(&self) -> &[u8] {
        &self.0
    }

    pub fn rlp_tree(&self) -> RlpTree {
        RlpTree::new(&self.0).unwrap()
    }
}

impl<'de> Deserialize<'de> for RlpProxy {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        deserializer.deserialize_any(RlpProxyVisitor)
    }
}

struct RlpProxyVisitor;

impl<'de> Visitor<'de> for RlpProxyVisitor {
    type Value = RlpProxy;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("AggregateVisitor Error.")
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> std::result::Result<Self::Value, E>
    where
        E: de::Error
    {
        Ok(RlpProxy(v.to_vec()))
    }
}

/// We must make sure 'de outlives
impl<'de: 'a, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    impl_deseralize_not_supported! {bool, f32, f64, identifier, ignored_any, map}
    impl_deseralize_integer! {@bytes i16, i32, i64, u16, u32, u64}
    impl_deseralize_integer! {@single u8, i8}

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de> 
    {
        let (bytes, new) = if self.next_is_bytes() {
            let (bytes, _, new) = self.next_bytes()?;
            (bytes, new)
        } else {
            let (bytes, _, new) = self.next_seq()?;
            (bytes, new)
        };
        
        *self = new;
        visitor.visit_borrowed_bytes(bytes)
    }

    // The `Serializer` implementation on the previous page serialized chars as
    // single-character strings so handle that representation here.
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let (_, bytes, new) = self.next_bytes()?;
        *self = new;
        let string = String::from_utf8(bytes.to_vec())
            .or(Err(Error::MalformedData))?;
        
        visitor.visit_char(
            string
            .as_str()
            .chars()
            .next()
            .ok_or(Error::MalformedData)?
        )
    }

    // Refer to the "Understanding deserializer lifetimes" page for information
    // about the three deserialization flavors of strings in Serde.
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let (_, bytes, new) = self.next_bytes()?;
        *self = new;
        let string = std::str::from_utf8(bytes)
            .or(Err(Error::MalformedData))?;

        visitor.visit_borrowed_str(string)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // The `Serializer` implementation on the previous page serialized byte
    // arrays as JSON arrays of bytes. Handle that representation here.
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let (_, bytes, new) = self.next_bytes()?;
        *self = new;
        visitor.visit_borrowed_bytes(bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }
    
    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    // In Serde, unit means an anonymous value containing no data.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {

        let (_, seq, new) = self.next_seq()?;
        *self = new;
        if seq.input.is_empty() {
            visitor.visit_unit()
        } else {
            Err(Error::MalformedData)
        }
    }

    // Unit struct means a named value containing no data.
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let (_, bytes, new) = self.next_bytes()?;
        *self = new;
        if bytes.is_empty() {
            visitor.visit_unit()
        } else {
            Err(Error::MalformedData)
        }
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    // Deserialization of compound types like sequences and maps happens by
    // passing the visitor an "Access" object that gives it the ability to
    // iterate through the data contained in the sequence.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let (_, seq, new) = self.next_seq()?;
        *self = new;
        visitor.visit_seq(seq)
    }

    // Tuples look just like sequences in JSON. Some formats may be able to
    // represent tuples more efficiently.
    //
    // As indicated by the length parameter, the `Deserialize` implementation
    // for a tuple in the Serde data model is required to know the length of the
    // tuple before even looking at the input data.
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Tuple structs look just like sequences in JSON.
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Structs look just like maps in JSON.
    //
    // Notice the `fields` parameter - a "struct" in the Serde data model means
    // that the `Deserialize` implementation is required to know what the fields
    // are before even looking at the input data. Any key-value pairing in which
    // the fields cannot be known ahead of time is probably a map.
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for Deserializer<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        // Deserialize an array element.
        seed.deserialize(&mut *self).map(Some)
    }
}
