use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::FileExt,
    path::Path,
    sync::BarrierWaitResult,
};

use anyhow::{Context, Ok, bail, ensure};
use crc32fast::Hasher;

struct Writer {
    write_position: u64,
    file: File,
    offset: u64,
}

struct Reader {
    read_position: u64,
    file: File,
    offset: u64,
}
impl Reader {
    fn new(path: impl AsRef<Path>, file: Option<File>) -> anyhow::Result<Self> {
        ensure!(
            path.as_ref().extension().is_some_and(|ext| ext == "kr"),
            "日志文件必须使用 .kr 扩展名"
        );

        Ok(Self {
            read_position: 0,
            file: file.unwrap_or(OpenOptions::new().read(true).open(path)?),
            offset: 0,
        })
    }

    fn from_write(w: &Writer) -> anyhow::Result<Self> {
        Ok(Reader {
            read_position: w.write_position,
            file: w.file.try_clone()?,
            offset: w.offset,
        })
    }

    fn get_list(&mut self,  r: &mut Vec<Record>) -> anyhow::Result<()> {
        let mut vec: Vec<u8> = Vec::new();
        self.file.read_to_end(&mut vec)?;
        let mut i = 0;
        // 这里可以改成异步的
        loop {
            let len = usize::from_be_bytes(vec[i..i + 8].try_into()?);
            r.push(Record::decode_checked(&vec[i..i + 20 + len])?);
            i = i + 20 + len;
            if i >= usize::try_from(self.file.metadata()?.len())? {
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Record {
    len: usize,
    crc32: u32,
    offset: u64,
    payload: Vec<u8>,
}

impl Record {
    fn new(offset: u64, payload: String) -> Self {
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

    fn encode(&self) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend(self.len.to_be_bytes());
        vec.extend(self.crc32.to_be_bytes());
        vec.extend(self.offset.to_be_bytes());
        vec.append(&mut self.payload.to_vec());
        println!("{:?}", vec);
        vec
    }

    fn decode(p: &[u8]) -> anyhow::Result<Self> {
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

    fn decode_checked(p: &[u8]) -> anyhow::Result<Self> {
        let record = Record::decode(p)?;
        let mut hasher = Hasher::new();
        hasher.update(&record.offset.to_be_bytes());
        let p = record.payload.clone();
        hasher.update(&p);
        let crc32 = hasher.finalize();
        ensure!(crc32 == record.crc32, "crc32校验失败，解码失效");
        Ok(record)
    }
}

impl Writer {
    fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        ensure!(
            path.as_ref().extension().is_some_and(|ext| ext == "kr"),
            "日志文件必须使用 .kr 扩展名"
        );

        let file = OpenOptions::new().create(true).write(true).open(path)?;

        Ok(Writer {
            write_position: file.metadata()?.len(),
            file,
            offset: 0,
        })
    }

    pub fn write_append(&mut self, input: String) -> anyhow::Result<()> {
        if input.is_empty() {
            return Ok(());
        }

        let len = u64::try_from(input.len())?;
        let next_posotion = self
            .write_position
            .checked_add(len + 20)
            .context("文件偏移量溢出")?;

        let record = Record::new(self.offset, input);

        self.file
            .write_all_at(&record.encode(), self.write_position)?;
        self.write_position = next_posotion;
        self.offset += 1;
        Ok(())
    }
}
mod test {
    use std::{
        fs::{self, File, OpenOptions},
        io::Read,
    };

    use anyhow::Ok;

    use crate::dao::{Reader, Record, Writer};

    #[test]
    fn test_write() -> anyhow::Result<()> {
        let mut w = Writer::new("./local.kr")?;
        w.write_append("Was ist".to_string())?;
        w.write_append("des Deutschen".to_string())?;
        w.write_append("Vaterland?".to_string())?;
        let mut rd = Reader::new("./local.kr",None)?;
        let mut vec = Vec::new();
        rd.get_list(&mut vec)?;
        for m in &vec {
            println!(
                "第{}条数据为{}",
                m.offset,
                String::from_utf8(m.payload.clone())?
            );
        }
        fs::remove_file("./local.kr")?;

        Ok(())
    }
}
