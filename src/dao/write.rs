// TODO(第8条): HashMap/Arc/RwLock 未使用；path: String 建议 PathBuf；20 这个头部长度魔数应提成常量
use std::{collections::HashMap, path::Path, sync::Arc}
;

use tokio::{
    fs::{File, OpenOptions}, io::AsyncWriteExt, sync::RwLock,
};

use anyhow::{Context, Ok, ensure};
use crate::dao::data::{Record};

pub struct Writer {
    write_position: u64,
    file: File,
    index: File,
    path: String,
    offset: u64
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
        let index = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(format!("{path}.index"))
            .await?;

        Ok(Writer {
            write_position: file.metadata().await?.len(),
            file,
            index,
            path: String::from(path),
            // TODO(第2条): 重启后应从索引末项恢复 offset+1；恒为 0 会与旧记录撞号
            offset: 0,
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
        // save the raw record
        self.file.write_all(&record.encode()).await?;

        //save the index
        self.index.write_all(&record.encode_index(self.write_position)).await?;
        self.write_position = next_posotion;
        self.offset += 1;
        // TODO(第6条): 两个文件都没 flush/sync_data，崩溃时索引与数据可能不一致
        Ok(())
    }
}
