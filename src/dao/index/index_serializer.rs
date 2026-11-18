use anyhow::anyhow;

use crate::dao::{data::LEN_SIZE, index::index::Index};

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
        if p.len() != LEN_SIZE + POSITION_LEN {
            return Err(anyhow!("index len error"));
        }

        let offset = u64::from_be_bytes(p[0..LEN_SIZE].try_into()?);
        let position = u64::from_be_bytes(p[LEN_SIZE..LEN_SIZE + POSITION_LEN].try_into()?);
        Ok(Index { offset, position })
    }

    pub fn decode_all(p: &[u8]) -> anyhow::Result<Vec<Index>> {
        if !p.len().is_multiple_of(16) {
            return Err(anyhow!("index len error"));
        }
        let mut i = 0;
        let mut vec = Vec::new();
        while i < p.len() {
            let index = Self::decode_exact(&p[i..i + LEN_SIZE + POSITION_LEN])?;
            vec.push(index);
            i += LEN_SIZE + POSITION_LEN;
        }
        Ok(vec)
    }
}
