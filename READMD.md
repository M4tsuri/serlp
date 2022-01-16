## A toy serializer for RLP encoding in ETH

**This is a toy implementation just for fun. DO NOT USE IT IN PRODUCTION**

**There is no deserializer. Because I'm lazy**

### Installation

Do not use it, use https://crates.io/crates/rlp instead.

### Usage

See tests, for example (https://eth.wiki/fundamentals/rlp):

```rust
    #[test]
    fn test_set_theoretic_definition() {
        // [ [], [[]], [ [], [[]] ] ]
        #[derive(Serialize)]
        struct Three<T>(T);

        let three = Three(vec![vec![], vec![vec![]], vec![vec![], vec![vec![0_u8; 0]]]]);

        let three_expected = [0xc7, 0xc0, 0xc1, 0xc0, 0xc3, 0xc0, 0xc1, 0xc0];
        assert_eq!(to_bytes(&three).unwrap(), three_expected)
    }
```

