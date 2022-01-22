use serlp::rlp::{from_bytes, to_bytes, RlpTree};
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
    let tree = RlpTree::new(&encode).unwrap();

    println!("original type has {} values.", tree.value_count());
    println!("encode result: {:?}", encode);
    assert_eq!(origin, embed);
}
