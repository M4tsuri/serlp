## A (de)serializer for RLP encoding in ETH

### Cargo.toml

```
serlp = "0.2.1"
serde = { version = "1.0", features = ['derive'] }
```

### Not Supported Types 

- bool
- float numbers
- maps
- enum (only deserialize)

We do not support enum when deserializing because we lost some information (i.e. variant index) about the original value when serializing.

We have to choose this approach because there is no enums in Golang while ETH is written in go. Treating enums as a transparent layer can make our furture implementation compatiable with ETH.

### Design principle

Accroding to the ETH Yellow Paper, all supported data structure can be represented with either recursive list of byte arrays ![](https://latex.codecogs.com/svg.latex?\mathbb{L}) or byte arrays ![](https://latex.codecogs.com/svg.latex?\mathbb{B}). So we can transform all Rust's compound types, for example, tuple, struct and list, into lists. And then encode them as exactly described in the paper

For example, the structure in example code, can be internally treated as the following form:

```
[
    "This is a tooooooooooooo loooooooooooooooooooong tag", 
    [
        114514, 
        [191, -9810], 
        [
            [[], [[]], [[], [[]]]]
        ]
    ], 
    "哼.啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊"
]
```

### ZST serialization

In Rust, we can represent 'empty' in many ways, for example:

```
[], (), "", b"", struct Empty, Variant::Empty, None, PhantomData<T>
```

In our implementation:

1. `[]` and `()` are considered empty list, thus should be serialized into 0xc0
2. All other ZSTs are considered empty, thus should be serialized into 0x80

To better understand ZSTs' behavior when serializing, try this code:

```rust
#[test]
fn test_compound_zst() {
    #[derive(Serialize, Debug, PartialEq, Eq)]
    struct ZST;

    #[derive(Serialize, Debug, PartialEq, Eq)]
    enum Simple {
        Empty(ZST),
        #[allow(dead_code)]
        Int((u32, u64))
    }

    #[derive(Serialize, Debug, PartialEq, Eq)]
    struct ContainZST(Simple);

    #[derive(Serialize, Debug, PartialEq, Eq)]
    struct StructZST {
        zst: Simple
    }

    let zst = Simple::Empty(ZST);
    let zst_res = to_bytes(&zst).unwrap();
    assert_eq!(zst_res, [0x80]);

    let with_zst = ContainZST(Simple::Empty(ZST));
    let with_zst_res = to_bytes(&with_zst).unwrap();
    // the container is transparent because its a newtype
    assert_eq!(with_zst_res, [0x80]);
    
    let with_zst = StructZST { zst: Simple::Empty(ZST) };
    let with_zst_res = to_bytes(&with_zst).unwrap();
    // the container is a list, to this is equivlent to [""]
    assert_eq!(with_zst_res, [0xc1, 0x80]);
    }
```

### Example code

You can find more examples [here](https://github.com/M4tsuri/serlp/tree/main/example)

```rust
use serlp::rlp::{from_bytes, to_bytes};
use serde::{Serialize, Deserialize};
use serde_bytes;

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

fn main() {
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

    println!("encode result: {:?}", encode);

    assert_eq!(origin, embed);
}
```
