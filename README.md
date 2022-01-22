## A (de)serializer for RLP encoding in ETH

### Cargo.toml

```
serlp = "0.3.0"
serde = { version = "1.0", features = ['derive'] }
```

### Not Supported Types 

- bool
- float numbers and signed integers
- maps
- enum (only deserialize)

We do not support enum when deserializing because we lost some information (i.e. variant index) about the original value when serializing. However, in some specific cases you can derive `Deserialize` trait for a enum with the help of `RlpProxy`, which will be discussed later.

We have to choose this approach because there is no enums in Golang while ETH is written in go. Treating enums as a transparent layer can make our furture implementation compatiable with ETH.

### Design principle

Accroding to the ETH Yellow Paper, all supported data structure can be represented with either recursive list of byte arrays ![](https://latex.codecogs.com/svg.latex?\mathbb{L}) or byte arrays ![](https://latex.codecogs.com/svg.latex?\mathbb{B}). So we can transform all Rust's compound types, for example, tuple, struct and list, into lists. And then encode them as exactly described in the paper

### Features

#### RLP Proxy 

We have a `RlpProxy` struct that implemented `Deserialize` trait, which just stores the original rlp encoded data after deserialization (no matter what type it is). You can gain more control over the deserialization process with it. 

Here is an example:

```rust
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(from = "RlpProxy")]
enum Classify {
    Zero(u8),
    One(u8),
    Ten((u8, u8))
}

impl From<RlpProxy> for Classify {
    fn from(proxy: RlpProxy) -> Self {
        let raw = proxy.raw();
        let mut tree = proxy.rlp_tree();
        if tree.value_count() == 2 {
            return Classify::Ten(from_bytes(raw).unwrap())
        }

        let val = tree.next().unwrap()[0];
        match val {
            0 => Classify::Zero(0),
            1 => Classify::One(1),
            _ => panic!("Value Error.")
        }
    }
}
```

#### (de)serializers for frequently used types

We provide two (de)serializers for frequently used types in blockchain.

- `biguint` for `num_bigint::BigUint`
- `byte_array` for `[u8; N]`

Put `#[serde(with = "biguint")]` or `#[serde(with = "byte_array")]` before your struct **field** to use them.

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
    out: (u8, u32),
    three: Third<((), ((),), ((), ((),)))>
}

fn main() {
    let embed = Embeding {
        tag: "This is a tooooooooooooo loooooooooooooooooooong tag",
        ed: Embedded {
            time: 114514,
            out: (191, 9810),
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
