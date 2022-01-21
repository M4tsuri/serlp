use std::collections::VecDeque;
use serde::{Serialize, Deserialize};

use crate::{
    ser::Serializer,
    error::{Result, Error}, 
    de::Deserializer
};

/// This function serialize a type instance into a byte vector with RLP encoding.
/// Note that we treat all compund types in rust as list.
/// Also, only **explicitly** values are encoded to keep consistence with the golang 
/// implementation.
/// Here *explicit values* corresponds to *implicit values*, which means the values 
/// used as markers, such as enum variant indexes and type wrappers.
/// 
/// For example: 
/// 
/// ```rust
/// struct Int(u8)
/// 
/// enum Sample {
///     Empty,
///     Int(u8)
/// }
/// ```
/// 
/// In out implementation, value `Int(15)` and `Sample::Int(15)` and `15_u8` will all
/// be encoded into the same value. This is because we consider the type wrapper `Int` 
/// and the variant `Sample::Int()` as implicit values, which are all language-level 
/// abstractions, thus we only take the value `u8` into consideration.
/// 
/// As is explained above, we can easily know that `Sample::Empty` will be encoded into 
/// an empty byte array because it contains no **explicit** values.
///  
pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    Serializer::to_bytes(value)
}

/// This function deserialize a byte slice into a type.
/// It works by construct a tree from the RLP encoded slice.
/// When serde is deserializing each field, it will call the corresponding
/// deserialize method, thus pops an element from the tree and decode it.
/// A potential problem is the standard RLP encoding is not capable to 
/// enocde all Rust types, for example, variants. So you may need to implement 
/// your own deserialize trait for some variant types when nessessary.
/// This function returns `Error::MalformedData` when the input is not 
/// valid RLP encoded bytes.
pub fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::new(s);
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlpNodeValue<'de> {
    Bytes(&'de [u8]),
    Compound(VecDeque<RlpNode<'de>>)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlpNode<'de> {
    span: &'de [u8],
    value: RlpNodeValue<'de>
}

/// A `RlpTree` is a polytree, each node is either a value or a list.
/// We build the tree by simulating the deserialization process with 
/// `de::Deserializer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlpTree<'de> {
    /// the max capicity of this node is 1, we only use VecDeque for consistency
    root: RlpNode<'de>,
    value_count: usize
}

enum TraverseRlp<'de> {
    Found(&'de [u8]),
    Leaf(&'de [u8]),
    Empty
}

impl<'de> RlpTree<'de> {
    pub fn new(buf: &'de [u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::MalformedData)
        }
        let mut root = VecDeque::with_capacity(1);
        let mut value_count = 0;

        let de = Deserializer::new(buf);
        let (tree, remained) = Self::parse_node(&mut value_count, de)?;
        root.push_back(tree);
        if !remained.is_empty() {
            Err(Error::MalformedData)
        } else {
            Ok(Self {
                root: RlpNode {
                    span: buf,
                    value: RlpNodeValue::Compound(root),
                },
                value_count
            })
        }
    }

    pub fn root(&'de self) -> &RlpNode {
        if let RlpNodeValue::Compound(root) = &self.root.value {
            root.front().unwrap()
        } else {
            panic!("No root node: Tree is empty.")
        }
    }

    pub fn root_mut(&'de mut self) -> &mut RlpNode {
        if let RlpNodeValue::Compound(root) = &mut self.root.value {
            root.front_mut().unwrap()
        } else {
            panic!("No root node: Tree is empty.")
        }
    }

    /// Each tree contains a `value_count` field. This value initially 
    /// represents the number of fields of the original type and decrements during 
    /// deserialization. 
    /// 
    /// This field is useful because sometime it can help you distinguish 
    /// different variant members when implementing your own Deserialize trait for 
    /// specific variant type. 
    /// 
    /// For example, here is a Golang correnpondence in the source code of ETH:
    /// 
    /// <https://github.com/ethereum/go-ethereum/blob/7dec26db2abcb062e676fd4972abc1d282ac3ced/trie/node.go#L117>
    pub fn value_count(&self) -> usize {
        self.value_count
    }

    /// parse a single node
    fn parse_node(counter: &mut usize, de: Deserializer<'de>) -> Result<(RlpNode<'de>, Deserializer<'de>)> {
        if de.next_is_bytes() {
            *counter += 1;
            Self::extract_bytes(de)
        } else {
            Self::extract_seq(counter, de)
        }
    }

    fn extract_bytes(de: Deserializer<'de>) -> Result<(RlpNode<'de>, Deserializer<'de>)> {
        let (span, bytes, new) = de.next_bytes()?;
        Ok((RlpNode {
            span,
            value: RlpNodeValue::Bytes(bytes)
        }, new))
    }

    fn extract_seq(counter: &mut usize, de: Deserializer<'de>) -> Result<(RlpNode<'de>, Deserializer<'de>)> {
        let (span, mut seq, remained) = de.next_seq()?;

        // now buf is the inner data
        let mut nodes = VecDeque::new();
        while !seq.is_empty()  {
            let (node, remained) = Self::parse_node(counter, seq)?;
            seq = remained;
            nodes.push_back(node);
        }

        Ok((RlpNode {
            span,
            value: RlpNodeValue::Compound(nodes)
        }, remained))
    }

    fn pop_front_deep(node: Option<&mut RlpNode<'de>>) -> TraverseRlp<'de> {
        let node = if let Some(node) = node {
            node
        } else {
            return TraverseRlp::Empty
        };
        match &mut node.value {
            RlpNodeValue::Bytes(bytes) => TraverseRlp::Leaf(bytes),
            RlpNodeValue::Compound(compound) => {
                loop {
                    match Self::pop_front_deep(compound.front_mut()) {
                        TraverseRlp::Empty => {
                            if !compound.is_empty() {
                                // the first subtree is empty, check the next one
                                compound.pop_front().unwrap();
                                continue;
                            }
                            // this tree is empty, we tell the upper frame to delete it
                            return TraverseRlp::Empty
                        },
                        // we found a valid leave, just remove it from the tree
                        TraverseRlp::Leaf(bytes) => {
                            compound.pop_front().unwrap();
                            return TraverseRlp::Found(bytes)
                        },
                        TraverseRlp::Found(bytes) => {
                            return TraverseRlp::Found(bytes)
                        }
                    }
                }
            }
        }
    }
}

impl<'de> Iterator for RlpTree<'de> {
    type Item = &'de [u8];

    /// Get the next value, the returned value is **removed** from the tree.
    /// This method always returns the leftmost leaf.
    fn next(&mut self) -> Option<&'de [u8]> {
        if self.value_count == 0 {
            return None
        }
        self.value_count -= 1;
        match Self::pop_front_deep(Some(&mut self.root)) {
            TraverseRlp::Found(bytes) => Some(bytes),
            // getting a Leave is impossible because we wrapped the root in a VecDeque
            _ => unreachable!()
        }
    }
}