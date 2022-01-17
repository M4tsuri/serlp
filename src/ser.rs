use serde::{ser, Serialize};
use crate::error::{Error, Result};
use paste::paste;

pub struct Serializer {
    /// the parser stack, we simulate recursion with this structure
    stack: Vec<Vec<u8>>
}

fn get_be_bytes_compact(src: &[u8]) -> &[u8] {
    for (i, &c) in src.iter().enumerate() {
        if c != 0 { return src.split_at(i).1 }
    }
    unreachable!()
}

// By convention, the public API of a Serde serializer is one or more `to_abc`
// functions such as `to_string`, `to_bytes`, or `to_writer` depending on what
// Rust types the serializer is able to produce as output.
//
// This basic serializer supports only `to_string`.
pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        stack: Vec::new()
    };
    serializer.stack.push(Vec::new());
    value.serialize(&mut serializer)?;
    Ok(serializer.stack.pop().unwrap())
}

macro_rules! impl_seralize_integer {
    ($($ity:ident),+) => {
        paste! {$(
            fn [<serialize_ $ity>](self, v: $ity) -> Result<()> {
                self.serialize_bytes(&v.to_be_bytes())
            }
        )+}
    }
}

macro_rules! impl_seralize_not_supported {
    ($($ity:ident),+) => {
        paste! {$(
            fn [<serialize_ $ity>](self, _v: $ity) -> Result<()> {
                Err(Error::TypeNotSupported)
            }
        )+}
    }
}



impl<'a> ser::Serializer for &'a mut Serializer {
    // The output type produced by this `Serializer` during successful
    // serialization. Most serializers that produce text or binary output should
    // set `Ok = ()` and serialize into an `io::Write` or buffer contained
    // within the `Serializer` instance, as happens here. Serializers that build
    // in-memory data structures may be simplified by using `Ok` to propagate
    // the data structure around.
    type Ok = ();

    // The error type when some error occurs during serialization.
    type Error = Error;

    // Associated types for keeping track of additional state while serializing
    // compound data structures like sequences and maps. In this case no
    // additional state is required beyond what is already stored in the
    // Serializer struct.
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    // yellow paper didn't mention how to encode bool and floats
    impl_seralize_not_supported! {bool, f32, f64}
    
    // according to yellow paper, integers should be encoded as bytes (big endian)
    impl_seralize_integer! {i8, i16, i32, i64, u8, u16, u32, u64}

    /// Serialize a char as a single-character string. 
    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(&v.to_string())
    }

    /// strings are bytes. THE YELLOW PAPER IS ALWAYS RIGHT!!!
    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_bytes())
    }

    /// YELLOW PAPER told us how to encode a byte array.
    /// LONG LIVE THE YELLOW PAPER!
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let last = self.stack.last_mut().unwrap();
        match v.len() as u64 {
            // x if ||x|| = 1 \land x[0] \lt 128
            1 if v[0] < 128 => last.extend(v),
            // (128 + ||x||) \dot x if ||x|| \lt 56
            0..=55 =>  {
                last.push(128 + v.len() as u8);
                last.extend(v);
            },
            // (183 + ||BE(||x||)||) \dot BE(||x||) \dot x if ||x|| \lt 2^64
            56..=u64::MAX => {
                let be_bytes = v.len().to_be_bytes();
                let len_be = get_be_bytes_compact(&be_bytes);
                last.push(183 + len_be.len() as u8);
                last.extend(len_be);
                last.extend(v);
            }
        }
        
        Ok(())
    }

    /// nothing
    /// So what is the difference between (), (()), None, "" and []
    fn serialize_none(self) -> Result<()> {
        Err(Error::TypeNotSupported)
    }

    /// I don't know what is it, so I serialize it
    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::TypeNotSupported)
    }

    /// There is nothing
    /// So what is the difference between (), (()), None, "" and []
    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    /// Another Nothing 
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    /// MORE NOTHING
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::TypeNotSupported)
    }

    /// This is TRANSPARENT!
    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    /// TRANSPARENT! But we do not support it.
    /// What a pity.
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::TypeNotSupported)
    }

    /// serialize a sequence, the sequence will be parsed recursively
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.stack.push(Vec::new());
        Ok(self)
    }
    
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        self.stack.push(Vec::new());
        Ok(self)
    }

    /// There is only a tuple
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_tuple(len)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::TypeNotSupported)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::TypeNotSupported)
    }

    /// We parse struct as we are parsing a sequence
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.stack.push(Vec::new());
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::TypeNotSupported)
    }
}

/// This impl is SerializeSeq so these methods are called after `serialize_seq`
/// is called on the Serializer.
impl<'a> ser::SerializeSeq for &'a mut Serializer {
    // Must match the `Ok` type of the serializer.
    type Ok = ();
    // Must match the `Error` type of the serializer.
    type Error = Error;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    // Close the sequence.
    fn end(self) -> Result<()> {
        self.frame_return();
        Ok(())
    }
}

// Same thing but for tuples.
impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.frame_return();
        Ok(())
    }
}

// Same thing but for tuple structs.
impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.frame_return();
        Ok(())
    }
}

// Tuple variants are a little different. Refer back to the
// `serialize_tuple_variant` method above:
//
//    self.output += "{";
//    variant.serialize(&mut *self)?;
//    self.output += ":[";
//
// So the `end` method in this impl is responsible for closing both the `]` and
// the `}`.
impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::TypeNotSupported)
    }

    fn end(self) -> Result<()> {
        Err(Error::TypeNotSupported)
    }
}

// Some `Serialize` types are not able to hold a key and value in memory at the
// same time so `SerializeMap` implementations are required to support
// `serialize_key` and `serialize_value` individually.
//
// There is a third optional method on the `SerializeMap` trait. The
// `serialize_entry` method allows serializers to optimize for the case where
// key and value are both available simultaneously. In JSON it doesn't make a
// difference so the default behavior for `serialize_entry` is fine.
impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    // The Serde data model allows map keys to be any serializable type. JSON
    // only allows string keys so the implementation below will produce invalid
    // JSON if the key serializes as something other than a string.
    //
    // A real JSON serializer would need to validate that map keys are strings.
    // This can be done by using a different Serializer to serialize the key
    // (instead of `&mut **self`) and having that other serializer only
    // implement `serialize_str` and return an error on any other data type.
    fn serialize_key<T>(&mut self, _key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::TypeNotSupported)
    }

    // It doesn't make a difference whether the colon is printed at the end of
    // `serialize_key` or at the beginning of `serialize_value`. In this case
    // the code is a bit simpler having it here.
    fn serialize_value<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::TypeNotSupported)
    }

    fn end(self) -> Result<()> {
        Err(Error::TypeNotSupported)
    }
}

// Structs are like maps in which the keys are constrained to be compile-time
// constant strings.
impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.frame_return();
        Ok(())
    }
}

impl Serializer {
    fn frame_return(&mut self) {
        // s(x)
        let frame = self.stack.pop().unwrap();
        // ||s(x)||
        let len = frame.len();

        let last = self.stack.last_mut().unwrap();

        match len as u64 {
            // (192 + ||s(x)||) \dot s(x) if s(x) \ne \empty \land ||s(x)|| \lt 56
            0..=55 => {
                last.push(192 + len as u8);
                last.extend(frame);
            },
            56..=u64::MAX => {
                let be_bytes = len.to_be_bytes();
                let len_be = get_be_bytes_compact(&be_bytes);
                last.push(247 + len_be.len() as u8);
                last.extend(len_be);
                last.extend(frame);
            }
        }
        
    }
}

// Similar to `SerializeTupleVariant`, here the `end` method is responsible for
// closing both of the curly braces opened by `serialize_struct_variant`.
impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::TypeNotSupported)
    }

    fn end(self) -> Result<()> {
        Err(Error::TypeNotSupported)
    }
}

