use std::{io::SeekFrom, path::Path};

use anyhow::{ensure, Context};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncSeekExt, AsyncWriteExt},
};

use crate::config::CONFIG;
use crate::dao::data::{Record, RecordSerializer};
use crate::dao::index::index::{Index, IndexWriteHandler};
use crate::dao::read::Reader;

pub struct Writer {
    write_position: u64,
    last_offset: u64,
    file: File,
    index: IndexWriteHandler,
}

impl Writer {
    pub async fn new(index: IndexWriteHandler) -> anyhow::Result<Self> {
        let p = Path::new(&CONFIG.dao.path);
        ensure!(
            p.extension().is_some_and(|ext| ext == "kr"),
            "日志文件必须使用 .kr 扩展名"
        );

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(p)
            .await?;

        let write_position = file.metadata().await?.len();
        // 重启后从日志里恢复最后一条消息的 offset，而不是依赖稀疏索引。
        let last_offset = if write_position == 0 {
            0
        } else {
            Reader::find_last().await?.offset
        };

        Ok(Writer {
            write_position,
            last_offset,
            file,
            index,
        })
    }

    pub async fn write_append(&mut self, input: String) -> anyhow::Result<()> {
        let offset = self.last_offset + 1;
        let record = Record::new(offset, input);
        let bytes = RecordSerializer::encode(&record);

        let next_position = self
            .write_position
            .checked_add(u64::try_from(bytes.len())?)
            .context("文件偏移量溢出")?;

        // 先写日志，再更新稀疏索引；索引里的 position 是这条消息的起始位置。
        self.file
            .seek(SeekFrom::Start(self.write_position))
            .await?;
        self.file.write_all(&bytes).await?;
        self.file.flush().await?;

        Index::sync_entry(&mut self.index, offset, self.write_position).await?;

        self.write_position = next_position;
        self.last_offset = offset;
        Ok(())
    }
}
