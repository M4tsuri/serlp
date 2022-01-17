## A (de)serializer for RLP encoding in ETH

### Cargo.toml

```
serde_rlp = 0.1.0
serde = { version = "1.0.133", features = ['derive'] }
```

### Not Supportted Types 

- bool
- float numbers
- enum

### Example code

```rust
use serde_rlp::{de::from_bytes, ser::to_bytes};
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

### Design principle

Accroding to the ETH Yellow Paper, all supported data structure can be represented with either recursive list of byte arrays $\mathbb{L}$ or byte arrays $\mathbb{B}$. So we can transform all Rust's compound types, for example, tuple, struct and list, into lists. And then encode them as exactly described in the paper

For example, the structure in example code, can be internally treated as the following form:

```
[
    "This is a tooooooooooooo loooooooooooooooooooong tag", [
        114514, 
        [191, -9810], 
        [
            [[], [[]], [[], [[]]]]
        ]
    ], 
    "哼.啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊啊"
]
```

