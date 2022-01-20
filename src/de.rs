use serde::de::{
    self, DeserializeSeed, SeqAccess, Visitor, MapAccess,
};
use byteorder::{BigEndian, ReadBytesExt};

use crate::error::{Error, Result};
use crate::rlp::RlpTree;
use paste::paste;

pub struct Deserializer<'de> {
    tree: RlpTree<'de>,
}

impl<'de> Deserializer<'de> {
    /// Create a deserializer instance from a byte slice, this will covert 
    /// the slice into a tree and store it.
    pub fn new(input: &'de [u8]) -> Result<Self> {
        Ok(Self { 
            tree: RlpTree::new(input)?
        })
    }

    pub fn with_rlp_tree(tree: RlpTree<'de>) -> Self {
        Self {
            tree
        }
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
        let empty = self.tree.next()
            .ok_or(Error::MalformedData)?;
        if empty.len() == 0 {
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
