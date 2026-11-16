use std::fmt::Debug;

use anyhow::{Ok, anyhow, ensure};
use crc32fast::Hasher;

pub const LEN_SIZE: usize = 8;
pub const CRC_SIZE: usize = 4;
pub const OFFSET_SIZE: usize = 8;
pub const META_SIZE: usize = LEN_SIZE + CRC_SIZE + OFFSET_SIZE;

pub struct Record {
    // payload's len(bit)
    pub(crate) len: usize,
    pub crc32: u32,
    pub offset: u64,
    pub payload: Vec<u8>,
}
impl Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Record")
            .field("len", &self.len)
            .field("crc32", &self.crc32)
            .field("offset", &self.offset)
            .field("payload", &String::from_utf8_lossy(&self.payload))
            .finish()
    }
}

impl Record {
    pub fn new(offset: u64, payload: String) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(&offset.to_be_bytes());
        let p = payload.into_bytes();
        hasher.update(&p);
        hasher.update(&p.len().to_be_bytes());
        let crc32 = hasher.finalize();

        Self {
            len: p.len(),
            crc32,
            offset,
            payload: p,
        }
    }
}

struct RecordSerializer;
impl RecordSerializer {
    pub fn encode(r: &Record) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend(r.len.to_be_bytes());
        vec.extend(r.crc32.to_be_bytes());
        vec.extend(r.offset.to_be_bytes());
        vec.extend_from_slice(&mut r.payload.to_vec());
        vec
    }

    fn decode_exact(p: &[u8]) -> anyhow::Result<Record> {
        let len = usize::from_be_bytes(p[0..LEN_SIZE].try_into()?);
        if p.len() != META_SIZE + len {
            return Err(anyhow!("the slice size error"));
        }
        let crc32: u32 = u32::from_be_bytes(p[LEN_SIZE..LEN_SIZE + CRC_SIZE].try_into()?);
        let offset = u64::from_be_bytes(p[LEN_SIZE + CRC_SIZE..META_SIZE].try_into()?);
        let payload = p[META_SIZE..META_SIZE + len].to_vec();
        Ok(Record {
            len,
            crc32,
            offset,
            payload,
        })
    }

    fn decode_checked(p: &[u8]) -> anyhow::Result<Record> {
        if p.len() < META_SIZE {
            return Err(anyhow!("the size of slice less than meta size {META_SIZE}"));
        }
        let record = Self::decode_exact(p)?;
        let mut hasher = Hasher::new();
        hasher.update(&record.offset.to_be_bytes());
        let p = record.payload.clone();
        hasher.update(&p);
        hasher.update(&p.len().to_be_bytes());
        let crc32 = hasher.finalize();
        ensure!(crc32 == record.crc32, "crc32 verification failed!");
        Ok(record)
    }

    pub fn decode(p: &[u8]) -> anyhow::Result<Vec<Record>> {
        if p.len() < LEN_SIZE {
            return Err(anyhow!("payload len err"));
        }
        let mut len;
        let mut i = 0;
        let mut vec: Vec<Record> = Vec::new();
        while p.len() > i {
            len = usize::from_be_bytes(p[i..i + LEN_SIZE].try_into()?);
            let r = Self::decode_checked(&p[i..i + META_SIZE + len])?;
            vec.push(r);

            i = i + META_SIZE + len;
        }
        Ok(vec)
    }
}

#[test]
fn test_encode() -> anyhow::Result<()> {
    let r = Record::new(0, String::from("你好"));
    let p = RecordSerializer::encode(&r);
    println!("\"你好\"解析后的结果为:{:?}", p);
    let r = Record::new(1, String::from("hello \n rust"));
    let p = RecordSerializer::encode(&r);
    println!("\"hello \\n rust\"解析后的结果为:{:?}", p);
    let r = Record::new(2, String::from("^%&*?"));
    let p = RecordSerializer::encode(&r);
    println!("\"^%&*?\"解析后的结果为:{:?}", p);
    Ok(())
}

#[test]
fn test_decode() -> anyhow::Result<()> {
    let v = RecordSerializer::decode(&[
        0, 0, 0, 0, 0, 0, 0, 6, 111, 185, 14, 225, 0, 0, 0, 0, 0, 0, 0, 0, 228, 189, 160, 229, 165,
        189, 0, 0, 0, 0, 0, 0, 0, 12, 69, 24, 15, 165, 0, 0, 0, 0, 0, 0, 0, 1, 104, 101, 108, 108,
        111, 32, 10, 32, 114, 117, 115, 116, 0, 0, 0, 0, 0, 0, 0, 5, 100, 32, 187, 14, 0, 0, 0, 0,
        0, 0, 0, 2, 94, 37, 38, 42, 63,
    ]);
    println!("反序列化的结果为 {:?}", v);
    Ok(())
}
