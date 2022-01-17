## A toy (de)serializer for RLP encoding in ETH

**This is a toy implementation just for fun. DO NOT USE IT IN PRODUCTION**

**This works only if the order when visiting struct fields is guaranteed by serde, i.e. when serde accesses struct fields with the same order when serializing and deserializing.**

### Installation

Do not use it, use https://crates.io/crates/rlp instead.

If you really want to use it, please create an issue.

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

