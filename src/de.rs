use std::collections::VecDeque;
use serde::Deserialize;
use serde::de::{
    self, DeserializeSeed, MapAccess, SeqAccess, Visitor,
};
use byteorder::{BigEndian, ReadBytesExt};

use crate::error::{Error, Result};
use paste::paste;

enum RLPNode<'a> {
    Bytes(&'a [u8]),
    Compound(VecDeque<RLPNode<'a>>)
}

impl<'a> RLPNode<'a> {
    /// parse a single node
    fn parse_node(buf: &'a [u8]) -> Result<(RLPNode, &'a [u8])> {
        match buf[0] {
            0..=191 => Self::extract_bytes(buf),
            // Compound type
            192..=255 => Self::extract_seq(buf)
        }
    }

    fn extract_bytes(buf: &'a [u8]) -> Result<(RLPNode, &'a [u8])> {
        Ok(match buf[0] {
            // R_b(x): ||x|| = 1 \land x[0] \lt 128
            0..=127 => (RLPNode::Bytes(&buf[..1]), &buf[1..]),
            // (128 + ||x||) \dot x
            len @ 128..=183 => {
                let pivot = 1 + (len as usize - 128);
                (RLPNode::Bytes(&buf[1..pivot]), &buf[pivot..])
            }
            // (183 + ||BE(||x||)||) \dot BE(||x||) \dot x
            be_len @ 184..=191 => {
                let be_len = be_len as usize - 183;
                let len = (&buf[1..]).read_uint::<BigEndian>(be_len)
                    .or(Err(Error::MalformedData))? as usize;
                let pivot = 1 + be_len + len;
                (RLPNode::Bytes(&buf[1 + be_len..pivot]), &buf[pivot..])
            }, 
            _ => unreachable!()  
        })
    }

    fn extract_seq(buf: &'a [u8]) -> Result<(RLPNode, &'a [u8])> {
        let (mut buf, remained) = match buf[0] {
            // (192 + ||s(x)||) \dot s(x)
            len @ 192..=247 => {
                let len = len as usize - 192;
                let pivot = len + 1;
                (&buf[1..pivot], &buf[pivot..])
            },
            be_len @ 248..=255 => {
                let be_len = be_len as usize - 247;
                let len = (&buf[1..]).read_uint::<BigEndian>(be_len)
                    .or(Err(Error::MalformedData))? as usize;
                let pivot = 1 + be_len + len;
                (&buf[1 + be_len..pivot], &buf[pivot..])
            },
            _ => unreachable!()
        };

        // now buf is the inner data
        let mut seq = VecDeque::new();
        let mut node;
        while buf.len() != 0 {
            (node, buf) = Self::parse_node(buf)?;
            seq.push_back(node);
        }

        Ok((RLPNode::Compound(seq), remained))
    }

    fn from_bytes(buf: &'a [u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::MalformedData)
        }
        let (root, remained) = Self::parse_node(buf)?;
        if !remained.is_empty() {
            Err(Error::MalformedData)
        } else {
            Ok(root)
        }
    }
}

struct RLPTree<'a> {
    /// the max capicity of this node is 1, we only use VecDeque for consistency
    root: VecDeque<RLPNode<'a>>
}

impl<'a> RLPTree<'a> {
    fn new(buf: &'a [u8]) -> Result<Self> {
        let queue = VecDeque::with_capacity(1);
        queue.push_back(RLPNode::from_bytes(buf)?);
        Ok(Self {
            root: queue
        })
    }

    fn is_empty(&self) -> bool {
        self.root.is_empty()
    }

    fn pop_front_deep(root: &'a mut VecDeque<RLPNode>) -> Option<&'a [u8]> {
        match root.pop_front() {
            Some(RLPNode::Bytes(bytes)) => Some(bytes),
            Some(RLPNode::Compound(compount)) => Self::pop_front_deep(&mut compount),
            None => None
        }
    } 

    /// get the next value 
    fn next(&'a mut self) -> Option<&'a [u8]> {
        Self::pop_front_deep(&mut self.root)
    }   
}

pub struct Deserializer<'de> {
    tree: RLPTree<'de>
}

impl<'de> Deserializer<'de> {
    // By convention, `Deserializer` constructors are named like `from_xyz`.
    // That way basic use cases are satisfied by something like
    // `serde_json::from_str(...)` while advanced use cases that require a
    // deserializer can make one with `serde_json::Deserializer::from_str(...)`.
    pub fn new(input: &'de [u8]) -> Result<Self> {
        Ok(Deserializer { 
            tree: RLPTree::new(input)?
        })
    }
}

// By convention, the public API of a Serde deserializer is one or more
// `from_xyz` methods such as `from_str`, `from_bytes`, or `from_reader`
// depending on what Rust types the deserializer is able to consume as input.
//
// This basic deserializer supports only `from_str`.
pub fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::new(s)?;
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.tree.is_empty() {
        Ok(t)
    } else {
        Err(Error::TrailingCharacters)
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
                visitor.[<visit_ $ity>](self.tree.next()
                    .ok_or(Error::MalformedData)?
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
                visitor.[<visit_ $ity>](self.tree.next()
                    .ok_or(Error::MalformedData)?
                    .[<read_ $ity>]()
                    .or(Err(Error::MalformedData))?)
            }
        )+}
    }
}

/// We must make sure 'de outlives
impl<'de: 'a, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    impl_deseralize_not_supported! {any, bool, f32, f64, identifier, ignored_any, map}
    impl_deseralize_integer! {@bytes i16, i32, i64, u16, u32, u64}
    impl_deseralize_integer! {@single u8, i8}

    // The `Serializer` implementation on the previous page serialized chars as
    // single-character strings so handle that representation here.
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let string = String::from_utf8(self.tree.next()
            .ok_or(Error::MalformedData)?
            .to_vec()).or(Err(Error::MalformedData))?;
        
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
        visitor.visit_string(String::from_utf8(self.tree.next()
            .ok_or(Error::MalformedData)?
            .to_vec()).or(Err(Error::MalformedData))?)
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
        visitor.visit_bytes(self.tree.next()
            .ok_or(Error::MalformedData)?)
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }
    
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
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
        visitor.visit_unit()
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
        self.deserialize_unit(visitor)
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
        // lifetime is 'a
        visitor.visit_seq(self)
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
        unimplemented!()
        // visitor.visit_map(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de: 'a, 'a> SeqAccess<'de> for Deserializer<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        // Deserialize an array element.
        seed.deserialize(self).map(Some)
    }
}


