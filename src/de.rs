use std::collections::VecDeque;
use serde::Deserialize;
use serde::de::{
    self, DeserializeSeed, SeqAccess, Visitor, MapAccess,
};
use byteorder::{BigEndian, ReadBytesExt};

use crate::error::{Error, Result};
use paste::paste;

#[derive(Debug)]
enum RLPNode<'de> {
    Bytes(&'de [u8]),
    Compound(VecDeque<RLPNode<'de>>)
}

impl<'de> RLPNode<'de> {
    /// parse a single node
    fn parse_node(buf: &'de [u8]) -> Result<(RLPNode, &'de [u8])> {
        match buf[0] {
            0..=191 => Self::extract_bytes(buf),
            // Compound type
            192..=255 => Self::extract_seq(buf)
        }
    }

    fn extract_bytes(buf: &'de [u8]) -> Result<(RLPNode, &'de [u8])> {
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

    fn extract_seq(buf: &'de [u8]) -> Result<(RLPNode, &'de [u8])> {
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

    fn from_bytes(buf: &'de [u8]) -> Result<Self> {
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

#[derive(Debug)]
struct RLPTree<'de> {
    /// the max capicity of this node is 1, we only use VecDeque for consistency
    root: RLPNode<'de>
}

enum TraverseRLP<'de> {
    Found(&'de [u8]),
    Leave(&'de [u8]),
    Empty
}

impl<'de> RLPTree<'de> {
    fn new(buf: &'de [u8]) -> Result<Self> {
        let mut queue = VecDeque::with_capacity(1);
        queue.push_back(RLPNode::from_bytes(buf)?);
        Ok(Self {
            root: RLPNode::Compound(queue)
        })
    }

    fn pop_front_deep(node: Option<&mut RLPNode<'de>>) -> TraverseRLP<'de> {
        match node {
            Some(&mut RLPNode::Bytes(bytes)) => TraverseRLP::Leave(bytes),
            Some(RLPNode::Compound(compound)) => {
                loop {
                    match Self::pop_front_deep(compound.front_mut()) {
                        TraverseRLP::Empty => {
                            if !compound.is_empty() {
                                // the first subtree is empty, check the next one
                                compound.pop_front().unwrap();
                                continue;
                            }
                            // this tree is empty, we tell the upper frame to delete it
                            return TraverseRLP::Empty
                        },
                        // we found a valid leave, just remove it from the tree
                        TraverseRLP::Leave(bytes) => {
                            compound.pop_front().unwrap();
                            return TraverseRLP::Found(bytes)
                        },
                        TraverseRLP::Found(bytes) => {
                            return TraverseRLP::Found(bytes)
                        }
                    }
                }
            },
            // this tree is empty, the upper frame will delete it
            None => TraverseRLP::Empty

        }
    }

    /// get the next value 
    fn next(&mut self) -> Option<&'de [u8]> {
        match Self::pop_front_deep(Some(&mut self.root)) {
            TraverseRLP::Found(bytes) => Some(bytes),
            // getting a Leave is impossible because we wrapped the root in a VecDeque
            _ => None
        }
    }   
}

pub struct Deserializer<'de> {
    tree: RLPTree<'de>,
}

impl<'de> Deserializer<'de> {
    // By convention, `Deserializer` constructors are named like `from_xyz`.
    // That way basic use cases are satisfied by something like
    // `serde_json::from_str(...)` while advanced use cases that require a
    // deserializer can make one with `serde_json::Deserializer::from_str(...)`.
    pub fn new(input: &'de [u8]) -> Result<Self> {
        Ok(Deserializer { 
            tree: RLPTree::new(input)?,
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
    Ok(t)
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
        let string = std::str::from_utf8(self.tree.next()
            .ok_or(Error::MalformedData)?)
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
        visitor.visit_borrowed_bytes(self.tree.next()
            .ok_or(Error::MalformedData)?)
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
        // unimplemented!()
        visitor.visit_seq(CompoundAccess::new(self))
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
        visitor.visit_seq(CompoundAccess::new(self))
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


struct CompoundAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> CompoundAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self {
            de
        }
    }
}


// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for CompoundAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        // Deserialize an array element.
        seed.deserialize(&mut *self.de).map(Some)
    }
}

impl<'de, 'a> MapAccess<'de> for CompoundAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, _seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de> 
    {
        Ok(None)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de> 
    {
        seed.deserialize(&mut *self.de)
    }
}
