use std::collections::VecDeque;
use byteorder::{BigEndian, ReadBytesExt};
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
    let mut deserializer = Deserializer::new(s)?;
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)
}

/// Sometimes we may have already built the RLP Tree from bytes, this method can help us 
/// save another extra tree build.
pub fn from_rlp_tree<'a, T>(tree: RlpTree<'a>) -> Result<T>
where
    T: Deserialize<'a>
{
    let mut deserializer = Deserializer::with_rlp_tree(tree);
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)
}


#[derive(Debug, Clone, PartialEq, Eq)]
enum RlpNode<'de> {
    Bytes(&'de [u8]),
    Compound(VecDeque<RlpNode<'de>>)
}

/// A `RlpTree` is a polytree, each node is either a value or a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlpTree<'de> {
    /// the max capicity of this node is 1, we only use VecDeque for consistency
    root: RlpNode<'de>,
    value_count: usize
}

enum TraverseRlp<'de> {
    Found(&'de [u8]),
    Leave(&'de [u8]),
    Empty
}

impl<'de> RlpTree<'de> {
    pub fn new(buf: &'de [u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::MalformedData)
        }
        let mut root = VecDeque::with_capacity(1);
        let mut value_count = 0;

        let (tree, remained) = Self::parse_node(&mut value_count, buf)?;
        root.push_back(tree);
        if !remained.is_empty() {
            Err(Error::MalformedData)
        } else {
            Ok(Self {
                root: RlpNode::Compound(root),
                value_count
            })
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
    fn parse_node(counter: &mut usize, buf: &'de [u8]) -> Result<(RlpNode<'de>, &'de [u8])> {
        match buf[0] {
            0..=191 => {
                *counter += 1;
                Self::extract_bytes(buf)
            },
            // Compound type
            192..=255 => Self::extract_seq(counter, buf)
        }
    }

    fn extract_bytes(buf: &'de [u8]) -> Result<(RlpNode<'de>, &'de [u8])> {
        Ok(match buf[0] {
            // R_b(x): ||x|| = 1 \land x[0] \lt 128
            0..=127 => (RlpNode::Bytes(&buf[..1]), &buf[1..]),
            // (128 + ||x||) \dot x
            len @ 128..=183 => {
                let pivot = 1 + (len as usize - 128);
                (RlpNode::Bytes(&buf[1..pivot]), &buf[pivot..])
            }
            // (183 + ||BE(||x||)||) \dot BE(||x||) \dot x
            be_len @ 184..=191 => {
                let be_len = be_len as usize - 183;
                let len = (&buf[1..]).read_uint::<BigEndian>(be_len)
                    .or(Err(Error::MalformedData))? as usize;
                let pivot = 1 + be_len + len;
                (RlpNode::Bytes(&buf[1 + be_len..pivot]), &buf[pivot..])
            }, 
            _ => unreachable!()  
        })
    }

    fn extract_seq(counter: &mut usize, buf: &'de [u8]) -> Result<(RlpNode<'de>, &'de [u8])> {
        let (mut buf, remained) = match buf[0] {
            // (192 + ||s(x)||) \dot s(x)
            len @ 192..=247 => {
                let len = len as usize - 192;
                let pivot = len + 1;
                (&buf[1..pivot], &buf[pivot..])
            },
            be_len @ 248..=255 => {
                let be_len = be_len as usize - 247;
                let len = (&buf[1..]).read_uint::<BigEndian>(be_len)
                    .or(Err(Error::MalformedData))? as usize;
                let pivot = 1 + be_len + len;
                (&buf[1 + be_len..pivot], &buf[pivot..])
            },
            _ => unreachable!()
        };

        // now buf is the inner data
        let mut seq = VecDeque::new();
        while buf.len() != 0 {
            let (node, remained) = Self::parse_node(counter, buf)?;
            buf = remained;
            seq.push_back(node);
        }

        Ok((RlpNode::Compound(seq), remained))
    }

    fn pop_front_deep(node: Option<&mut RlpNode<'de>>) -> TraverseRlp<'de> {
        match node {
            Some(&mut RlpNode::Bytes(bytes)) => TraverseRlp::Leave(bytes),
            Some(RlpNode::Compound(compound)) => {
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
                        TraverseRlp::Leave(bytes) => {
                            compound.pop_front().unwrap();
                            return TraverseRlp::Found(bytes)
                        },
                        TraverseRlp::Found(bytes) => {
                            return TraverseRlp::Found(bytes)
                        }
                    }
                }
            },
            // this tree is empty, the upper frame will delete it
            None => TraverseRlp::Empty

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