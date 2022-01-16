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

    // Here we go with the simple methods. The following 12 methods receive one
    // of the primitive types of the data model and map it to JSON by appending
    // into the output string.
    impl_seralize_not_supported! {bool, f32, f64}
    
    // JSON does not distinguish between different sizes of integers, so all
    // signed integers will be serialized the same and all unsigned integers
    // will be serialized the same. Other formats, especially compact binary
    // formats, may need independent logic for the different sizes.
    impl_seralize_integer! {i8, i16, i32, i64, u8, u16, u32, u64}

    // Serialize a char as a single-character string. Other formats may
    // represent this differently.
    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(&v.to_string())
    }

    // This only works for strings that don't require escape sequences but you
    // get the idea. For example it would emit invalid JSON if the input string
    // contains a '"' character.
    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_bytes())
    }

    // Serialize a byte array as an array of bytes. Could also use a base64
    // string here. Binary formats will typically represent byte arrays more
    // compactly.
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

    // An absent optional is represented as the JSON `null`.
    fn serialize_none(self) -> Result<()> {
        Ok(())
    }

    // A present optional is represented as just the contained value. Note that
    // this is a lossy representation. For example the values `Some(())` and
    // `None` both serialize as just `null`. Unfortunately this is typically
    // what people expect when working with JSON. Other formats are encouraged
    // to behave more intelligently if possible.
    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    // In Serde, unit means an anonymous value containing no data. Map this to
    // JSON as `null`.
    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    // Unit struct means a named value containing no data. Again, since there is
    // no data, map this to JSON as `null`. There is no need to serialize the
    // name in most formats.
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    // When serializing a unit variant (or any other kind of variant), formats
    // can choose whether to keep track of it by index or by name. Binary
    // formats typically use the index of the variant and human-readable formats
    // typically use the name.
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::TypeNotSupported)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain.
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

    // newtype variant is not supported in RLP
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

    // serialize a sequence, the sequence will be parsed recursively
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.stack.push(Vec::new());
        Ok(self)
    }
    
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        self.stack.push(Vec::new());
        Ok(self)
    }

    // Tuple structs look just like sequences in JSON.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_tuple(len)
    }

    // Tuple variants are represented in JSON as `{ NAME: [DATA...] }`. Again
    // this method is only responsible for the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::TypeNotSupported)
    }

    // Maps are represented in JSON as `{ K: V, K: V, ... }`.
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::TypeNotSupported)
    }

    // Structs look just like maps in JSON. In particular, JSON requires that we
    // serialize the field names of the struct. Other formats may be able to
    // omit the field names when serializing structs because the corresponding
    // Deserialize implementation is required to know what the keys are without
    // looking at the serialized data.
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.stack.push(Vec::new());
        Ok(self)
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
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

// The following 7 impls deal with the serialization of compound types like
// sequences and maps. Serialization of such types is begun by a Serializer
// method and followed by zero or more calls to serialize individual elements of
// the compound type and one call to end the compound type.
//
// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
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

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    use std::vec;

    use serde::Serialize;
    use serde_bytes::Bytes;

    use crate::ser::to_bytes;

    #[test]
    fn test_long_string() {
        #[derive(Serialize)]
        struct LongStr<'a>(&'a str);

        let long_str = LongStr("Lorem ipsum dolor sit amet, consectetur adipisicing elit");
        let expected: Vec<u8> = [0xb8_u8, 0x38_u8]
            .into_iter()
            .chain(long_str.0.as_bytes().to_owned())
            .collect();
        assert_eq!(to_bytes(&long_str).unwrap(), expected)
    }

    #[test]
    fn test_set_theoretic_definition() {
        // [ [], [[]], [ [], [[]] ] ]
        #[derive(Serialize)]
        struct Three<T>(T);

        let three = Three(vec![vec![], vec![vec![]], vec![vec![], vec![vec![0_u8; 0]]]]);

        let three_expected = [0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0];
        assert_eq!(to_bytes(&three).unwrap(), three_expected)
    }

    #[test]
    fn test_1024() {
        #[derive(Serialize)]
        struct Int(u16);

        let simp_str = Int(1024);
        let simp_str_expected = [0x82, 0x04, 0x00];

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected)
    }

    #[test]
    fn test_15() {
        #[derive(Serialize)]
        struct Int(u8);

        let simp_str = Int(15);
        let simp_str_expected = [0xf];

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected)
    }

    #[test]
    fn test_zero() {
        #[derive(Serialize)]
        struct Int(u8);

        let simp_str = Int(0);
        let simp_str_expected = [0x00];

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected)
    }

    #[test]
    fn test_empty() {
        let simp_str = Bytes::new(b"");
        let simp_str_expected = [0x80];

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected)
    }

    #[test]
    fn test_bytes() {
        let simp_str = Bytes::new(b"dog");
        let simp_str_expected = [0x83, b'd', b'o', b'g'];

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected)
    }

    #[test]
    fn test_list() {
        #[derive(Serialize)]
        struct SimpList {
            #[serde(with = "serde_bytes")]
            cat: Vec<u8>,
            #[serde(with = "serde_bytes")]
            dog: Vec<u8>
        }

        let simp_list = SimpList {
            cat: b"cat".to_vec(),
            dog: b"dog".to_vec(),
        };
        let simp_list_expected = [0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g'];

        assert_eq!(to_bytes(&simp_list).unwrap(), simp_list_expected);
    }

    #[test]
    fn test_empty_list() {
        #[derive(Serialize)]
        struct SimpList {
        }

        let simp_list = SimpList {
        };

        let simp_list_expected = [0xc0];

        assert_eq!(to_bytes(&simp_list).unwrap(), simp_list_expected);
    }

}

