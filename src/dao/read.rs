use std::io::SeekFrom;

use anyhow::anyhow;
use tokio::{
    fs::OpenOptions,
    io::{AsyncReadExt, AsyncSeekExt},
};

use crate::config::CONFIG;
use crate::dao::data::{Record, RecordSerializer};
use crate::dao::index::index::Index;

pub struct Reader;

impl Reader {
    /// 从日志文件的指定字节位置开始，读取到文件末尾。
    async fn read_from(position: u64) -> anyhow::Result<Vec<u8>> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(&CONFIG.dao.path)
            .await?;
        let len = file.metadata().await?.len();
        file.seek(SeekFrom::Start(position.min(len))).await?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    /// 读取 [start_offset, end_offset) 区间的消息。
    /// start_offset 默认 0，end_offset 默认到文件末尾。
    pub async fn get_records(
        start_offset: Option<u64>,
        end_offset: Option<u64>,
    ) -> anyhow::Result<Vec<Record>> {
        let start = start_offset.unwrap_or(0);
        let end = end_offset.unwrap_or(u64::MAX);
        if start >= end {
            return Ok(Vec::new());
        }

        // 先通过稀疏索引定位到 start 之前最近的一条记录，再顺序读取。
        let position = Index::find_index_range(start).await?.0.position;
        let buf = Self::read_from(position).await?;
        let records = RecordSerializer::decode(&buf)?
            .into_iter()
            .filter(|r| r.offset >= start && r.offset < end)
            .collect();
        Ok(records)
    }

    pub async fn find_by_offset(offset: u64) -> anyhow::Result<Record> {
        let position = Index::find_index_range(offset).await?.0.position;
        let buf = Self::read_from(position).await?;
        RecordSerializer::decode(&buf)?
            .into_iter()
            .find(|r| r.offset == offset)
            .ok_or_else(|| anyhow!("can't find the record with offset {offset}"))
    }

    pub async fn find_last() -> anyhow::Result<Record> {
        let last_index = Index::read_last_entry().await?;
        let buf = Self::read_from(last_index.position).await?;
        if let Some(record) = RecordSerializer::decode(&buf)?.into_iter().last() {
            return Ok(record);
        }

        // 索引为空或索引位置异常时，从文件头兜底读取。
        let buf = Self::read_from(0).await?;
        RecordSerializer::decode(&buf)?
            .into_iter()
            .last()
            .ok_or_else(|| anyhow!("no record found"))
    }
}
