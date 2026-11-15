use anyhow::ensure;
use crc32fast::Hasher;

// TODO(第3条): len 与 payload.len() 完全冗余；Default 派不上用场
#[derive(Debug, Default)]
pub struct Record {
    // TODO(第3条): 存盘应定宽 u32/u64；usize 宽度随平台变，而编解码硬编码 8 字节
    len: usize,
    crc32: u32,
    pub(crate) offset: u64,
    payload: Vec<u8>,
}


impl Record {
    // TODO(第4条): len/crc32/payload 全私有且无 getter，调用方拿到 Record 读不出数据
    pub fn payload(&self) -> &[u8] {
        todo!("第4条: 补 payload/crc32/len 的读取接口")
    }
    pub fn new(offset: u64, payload: String) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(&offset.to_be_bytes());
        let p = payload.into_bytes();
        hasher.update(&p);
        let crc32 = hasher.finalize();

        Self {
            len: p.len(),
            crc32,
            offset,
            payload: p,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend(self.len.to_be_bytes());
        vec.extend(self.crc32.to_be_bytes());
        vec.extend(self.offset.to_be_bytes());
        // TODO(第3条): 白克隆一次，改 extend_from_slice(&self.payload)
        vec.append(&mut self.payload.to_vec());
        // TODO(第3条): 调试 println 残留，每条记录把完整字节打到 stdout
        println!("{:?}", vec);
        vec
    }

    // TODO(第3条): 缺边界检查，p[0..8] / p[20..20+len] 对截断数据直接 panic
    #[allow(unreachable_code, unused_variables)]
    pub fn decode(p: &[u8]) -> anyhow::Result<Self> {
        todo!("第3条: decode 未校验切片长度，文件末尾半条记录会 panic");
        let len = usize::from_be_bytes(p[0..8].try_into()?);
        let crc32: u32 = u32::from_be_bytes(p[8..12].try_into()?);
        let offset = u64::from_be_bytes(p[12..20].try_into()?);
        let payload = p[20..20 + len].to_vec();
        Ok(Self {
            len,
            crc32,
            offset,
            payload,
        })
    }

    pub fn decode_checked(p: &[u8]) -> anyhow::Result<Self> {
        let record = Record::decode(p)?;
        let mut hasher = Hasher::new();
        // TODO(第3条): crc 只覆盖 offset+payload，不含 len；而 len 决定切片长度
        hasher.update(&record.offset.to_be_bytes());
        let p = record.payload.clone();
        hasher.update(&p);
        let crc32 = hasher.finalize();
        ensure!(crc32 == record.crc32, "crc32 verification failed!");
        Ok(record)
    }

    pub fn encode_index(&self, position: u64) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend(self.offset.to_be_bytes());
        vec.extend(position.to_be_bytes());
        // TODO(第3条): 调试 println 残留
        println!("{:?}", vec);
        vec
    }
}
