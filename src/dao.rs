use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::FileExt,
    path::Path,
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

    fn decode(p: &[u8], read_position: Option<u64>, file: File) -> anyhow::Result<Self> {
        let len = usize::from_be_bytes(p[0..7].try_into()?);
        let crc32 = usize::from_be_bytes(p[8..11].try_into()?);
        let offset = usize::from_be_bytes(p[12..19].try_into()?);
        let len: usize = usize::from_be_bytes(p[0..7].try_into()?);
        Ok(
			Self { len, crc32, offset: (), payload: () }
		)
    }

    fn decode_checked(p: &[u8], read_position: Option<u64>, file: File) -> anyhow::Result<Self> {
        let record = Record::decode(p, read_position, file)?;
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
            .checked_add(len)
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

    use crate::dao::Writer;

    #[test]
    fn test_write() -> anyhow::Result<()> {
        let mut w = Writer::new("./local.kr")?;
        w.write_append("1".to_string())?;
        w.write_append("什么".to_string())?;
        w.write_append("3".to_string())?;
        let data: Vec<u8> = fs::read("./local.kr")?;
        // fs::remove_file("./local.kr");
        Ok(())
    }
}
