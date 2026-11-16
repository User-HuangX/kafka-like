// TODO(第8条): HashMap/Arc/RwLock 未使用；path: String 建议 PathBuf；20 这个头部长度魔数应提成常量
use std::{io::SeekFrom, path::Path};

use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::dao::data::Record;
use anyhow::{Context, Ok, ensure};

pub struct Writer {
    write_position: u64,
    file: File,
    index: File,
    path: String,
    offset: u64,
}

impl Writer {
    async fn new(path: &str) -> anyhow::Result<Self> {
        let p = Path::new(path);
        ensure!(
            p.extension().is_some_and(|ext| ext == "kr"),
            "日志文件必须使用 .kr 扩展名"
        );

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(path)
            .await?;

        let mut index = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(format!("{path}.index"))
            .await?;

        let max_len = file.metadata().await?.len();
        let mut offset = 0;
        if max_len >= 16 {
            let mut buf = [0u8; 8];
            index.seek(SeekFrom::End(-16)).await?;
            index.read_exact(&mut buf).await?;
            offset = u64::from_be_bytes(buf);
        }
        index.flush().await?;
        Ok(Writer {
            write_position: max_len,
            file,
            index,
            path: String::from(path),
            offset: offset + 1,
        })
    }

    pub async fn write_append(&mut self, input: String) -> anyhow::Result<()> {
        if input.is_empty() {
            return Ok(());
        }

        let len = u64::try_from(input.len())?;
        let next_posotion = self
            .write_position
            .checked_add(len + 20)
            .context("文件偏移量溢出")?;

        let record = Record::new(self.offset, input);
        self.file.seek(SeekFrom::End(0)).await?;
        self.index.seek(SeekFrom::End(0)).await?;
        // save the raw record
        self.file.write_all(&record.encode()).await?;

        //save the index
        if record.offset > self.index.metadata().await?.len() / 160 {
            self.index
                .write_all(&record.encode_index(self.write_position))
                .await?;
        }

        self.write_position = next_posotion;
        self.offset += 1;
        self.index.flush().await?;
        self.file.flush().await?;
        Ok(())
    }
}
