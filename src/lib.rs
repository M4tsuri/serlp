pub mod ser;
pub mod error;
pub mod de;

#[cfg(test)]
mod test {
    use serde::{Serialize, Deserialize};
    use serde_bytes::Bytes;

    use crate::ser::to_bytes;
    use crate::de::from_bytes;

    #[test]
    fn test_long_string() {
        #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
        struct LongStr<'a>(&'a str);

        let long_str = LongStr("Lorem ipsum dolor sit amet, consectetur adipisicing elit");
        let expected: Vec<u8> = [0xb8_u8, 0x38_u8]
            .into_iter()
            .chain(long_str.0.as_bytes().to_owned())
            .collect();
        
        let origin: LongStr = from_bytes(&expected).unwrap();
        assert_eq!(to_bytes(&long_str).unwrap(), expected);
        assert_eq!(long_str, origin);
    }

    #[test]
    fn test_set_theoretic_definition() {
        // [ [], [[]], [ [], [[]] ] ]
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Three<T>(T);

        let three = Three(((), ((),), ((), ((),))));

        let three_expected = [0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0];
        let origin: Three<((), ((),), ((), ((),)))> = from_bytes(&three_expected).unwrap();
        assert_eq!(to_bytes(&three).unwrap(), three_expected);
        assert_eq!(origin, three)
    }

    #[test]
    fn test_1024() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Int(u16);

        let simp_str = Int(1024);
        let simp_str_expected = [0x82, 0x04, 0x00];
        let origin: Int = from_bytes(&simp_str_expected).unwrap();

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected);
        assert_eq!(simp_str, origin)
    }

    #[test]
    fn test_15() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Int(u8);

        let simp_str = Int(15);
        let simp_str_expected = [0xf];
        let origin: Int = from_bytes(&simp_str_expected).unwrap();

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected);
        assert_eq!(origin, simp_str)
    }

    #[test]
    fn test_zero() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Int(u8);

        let simp_str = Int(0);
        let simp_str_expected = [0x00];
        let origin: Int = from_bytes(&simp_str_expected).unwrap();

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected);
        assert_eq!(origin, simp_str)
    }

    #[test]
    fn test_empty() {
        let simp_str = Bytes::new(b"");
        let simp_str_expected = [0x80];
        let origin: &Bytes = from_bytes(&simp_str_expected).unwrap();

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected);
        assert_eq!(origin, simp_str)
    }

    #[test]
    fn test_bytes() {
        let simp_str = Bytes::new(b"dog");
        let simp_str_expected = [0x83, b'd', b'o', b'g'];
        let origin: &Bytes = from_bytes(&simp_str_expected).unwrap();

        assert_eq!(to_bytes(&simp_str).unwrap(), simp_str_expected);
        assert_eq!(origin, simp_str)
    }

    #[test]
    fn test_list() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
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
        let origin: SimpList = from_bytes(&simp_list_expected).unwrap();

        assert_eq!(to_bytes(&simp_list).unwrap(), simp_list_expected);
        assert_eq!(origin, simp_list)
    }

    #[test]
    fn test_empty_list() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct SimpList {
        }

        let simp_list = SimpList {
        };

        let simp_list_expected = [0xc0];
        let origin: SimpList = from_bytes(&simp_list_expected).unwrap();

        assert_eq!(to_bytes(&simp_list).unwrap(), simp_list_expected);
        assert_eq!(origin, simp_list)
    }

    #[test]
    fn test_boxed_value() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize, Clone)]
        struct Boxed {
            a: Box<String>
        }

        let b = Boxed { a: Box::new("dog".into()) };
        let expected = [0xc4, 0x83, b'd', b'o', b'g'];
        let origin: Boxed = from_bytes(&expected).unwrap();

        assert_eq!(origin, b);
        assert_eq!(to_bytes(&b).unwrap(), expected);
    }

    #[test]
    fn test_simple_enum() {
        #[derive(Serialize, Debug, PartialEq, Eq)]
        enum Simple {
            Empty,
            Int(u32)
        }

        #[derive(Serialize, Debug, PartialEq, Eq)]
        struct Equiv(u32);

        let simple_enum = Simple::Int(114514);
        let unit_variant = Simple::Empty;
        let equiv = Equiv(114514);

        let enum_res = to_bytes(&simple_enum).unwrap();
        let empty_res = to_bytes(&unit_variant).unwrap();
        let struct_res = to_bytes(&equiv).unwrap();

        assert_eq!(enum_res, struct_res);
        assert_eq!(empty_res, []);
    }

    #[test]
    fn test_variant_tuple() {
        #[derive(Serialize, Debug, PartialEq, Eq)]
        enum Simple {
            Empty,
            Int((u32, u64))
        }

        #[derive(Serialize, Debug, PartialEq, Eq)]
        struct Equiv((u32, u64));

        let simple_enum = Simple::Int((114514, 1919810));
        let unit_variant = Simple::Empty;
        let equiv = Equiv((114514, 1919810));

        let enum_res = to_bytes(&simple_enum).unwrap();
        let empty_res = to_bytes(&unit_variant).unwrap();
        let struct_res = to_bytes(&equiv).unwrap();

        assert_eq!(enum_res, struct_res);
        assert_eq!(empty_res, []);
    }

    #[test]
    fn test_variant_struct() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize, Clone)]
        struct Third<T> {
            inner: T
        }
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize, Clone)]
        struct Embeding<'a> {
            tag: &'a str,
            ed: Embedded,
            #[serde(with = "serde_bytes")]
            bytes: Vec<u8>
        }
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize, Clone)]
        struct Embedded {
            time: u64,
            out: (u8, i32),
            three: Third<((), ((),), ((), ((),)))>
        }

        #[derive(Serialize, Debug, PartialEq, Eq)]
        enum Simple<'a> {
            #[allow(dead_code)]
            Empty,
            #[allow(dead_code)]
            Int((u32, u64, Embedded)),
            Struct(Embeding<'a>)
        }

        #[derive(Serialize, Debug, PartialEq, Eq)]
        struct Equiv((u32, u64));

        let embed = Embeding {
            tag: "This is a tooooooooooooo loooooooooooooooooooong tag",
            ed: Embedded {
                time: 114514,
                out: (191, -9810),
                three: Third {
                    inner: ((), ((),), ((), ((),)))
                }
            },
            bytes: "哼.啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊".as_bytes().to_vec()
        };

        let simple_enum = Simple::Struct(embed.clone());

        let simple_res = to_bytes(&simple_enum).unwrap();
        let struct_res = to_bytes(&embed).unwrap();

        assert_eq!(simple_res, struct_res);
    }

    #[test]
    fn test_embedded_struct() {
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Third<T> {
            inner: T
        }
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Embeding<'a> {
            tag: &'a str,
            ed: Embedded,
            #[serde(with = "serde_bytes")]
            bytes: Vec<u8>
        }
        #[derive(Serialize, Debug, PartialEq, Eq, Deserialize)]
        struct Embedded {
            time: u64,
            out: (u8, i32),
            three: Third<((), ((),), ((), ((),)))>
        }

        let embed = Embeding {
            tag: "This is a tooooooooooooo loooooooooooooooooooong tag",
            ed: Embedded {
                time: 114514,
                out: (191, -9810),
                three: Third {
                    inner: ((), ((),), ((), ((),)))
                }
            },
            bytes: "哼.啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊".as_bytes().to_vec()
        };

        let encode = to_bytes(&embed).unwrap();
        let origin: Embeding = from_bytes(&encode).unwrap();
        assert_eq!(embed, origin)
    }

}

