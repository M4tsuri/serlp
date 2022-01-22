## A (de)serializer for RLP encoding in ETH

### Cargo.toml

```
serlp = "0.3.0"
serde = { version = "1.0", features = ['derive'] }
```

### Example

This example shows how can we encode a real transcation on ETH mainnet with the help of serlp.

The transcation is the #0 transcation of https://api.etherscan.io/api?module=proxy&action=eth_getBlockByNumber&tag=0xa1a489&boolean=true&apikey=YourApiKeyToken. The encoded data is from README of https://github.com/zhangchiqing/merkle-patricia-trie.

```rust
fn test_bn() {
    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct LegacyTx {
        nonce: u64,
        #[serde(with = "biguint")]
        gas_price: BigUint,
        gas_limit: u64,
        #[serde(with = "byte_array")]
        to: [u8; 20],
        #[serde(with = "biguint")]
        value: BigUint,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
        #[serde(with = "biguint")]
        v: BigUint,
        #[serde(with = "biguint")]
        r: BigUint,
        #[serde(with = "biguint")]
        s: BigUint
    }

    let mut to = [0; 20];
    to.copy_from_slice(&hex::decode("a3bed4e1c75d00fa6f4e5e6922db7261b5e9acd2").unwrap());
    let bn = |s| BigUint::from_bytes_be(&hex::decode(s).unwrap());
    
    let tx = LegacyTx {
        nonce: 0xa5,
        gas_price: bn("2e90edd000"),
        gas_limit: 0x12bc2,
        to,
        value: bn("00"),
        data: hex::decode("a9059cbb0000000000000000000000008bda8b9823b8490e8cf220dc7b91d97da1c54e250000000000000000000000000000000000000000000000056bc75e2d63100000").unwrap(),
        v: bn("26"),
        r: bn("6c89b57113cf7da8aed7911310e03d49be5e40de0bd73af4c9c54726c478691b"),
        s: bn("56223f039fab98d47c71f84190cf285ce8fc7d9181d6769387e5efd0a970e2e9")
    };

    let expected = "f8ab81a5852e90edd00083012bc294a3bed4e1c75d00fa6f4e5e6922db7261b5e9acd280b844a9059cbb0000000000000000000000008bda8b9823b8490e8cf220dc7b91d97da1c54e250000000000000000000000000000000000000000000000056bc75e2d6310000026a06c89b57113cf7da8aed7911310e03d49be5e40de0bd73af4c9c54726c478691ba056223f039fab98d47c71f84190cf285ce8fc7d9181d6769387e5efd0a970e2e9";
    let encoded = to_bytes(&tx).unwrap();
    let orig: LegacyTx = from_bytes(&encoded).unwrap();

    assert_eq!(orig, tx);
    assert_eq!(hex::encode(encoded), expected);
}
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
