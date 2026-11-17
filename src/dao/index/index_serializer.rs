use anyhow::{Ok, anyhow};

use crate::dao::{
    data::LEN_SIZE ,
    index::index::Index,
};

const POSITION_LEN: usize = 8;

pub struct IndexSerializer;
impl IndexSerializer {
    pub fn encode(offset: u64, position: u64) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend(offset.to_be_bytes());
        vec.extend(position.to_be_bytes());
        vec
    }

    fn decode_exact(p: &[u8]) -> anyhow::Result<Index> {
        if p.len() % 16 != 0 {
            return Err(anyhow!("index len error"));
        }
        
        let offset = u64::from_be_bytes(p[0..LEN_SIZE].try_into().unwrap());
        let position = u64::from_be_bytes(p[LEN_SIZE..LEN_SIZE + POSITION_LEN].try_into().unwrap());
        Ok(Index { offset, position })
    }
}
