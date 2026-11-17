use std::fs;
use std::io::SeekFrom::{self};

use anyhow::{Ok, anyhow, ensure};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::dao::index::index_serializer::IndexSerializer;

#[derive(Debug)]
pub struct Index {
    pub offset: u64,
    pub position: u64,
}

struct IndexWriteHandler {
    file: File,
    path: String,
    seek: usize,
}

impl IndexWriteHandler {
    async fn new(path: String) -> anyhow::Result<Self> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
            .await?;
        let seek = file.seek(SeekFrom::End(0)).await? as usize;
        Ok(Self {
            file,
            path,
            seek,
        })
    }

    async fn write_append(&mut self, i: Index) -> anyhow::Result<()> {
        self.file.seek(SeekFrom::Start(self.seek as u64)).await?;
        self.file.write_all(&i.offset.to_be_bytes()).await?;
        self.file.write_all(&i.position.to_be_bytes()).await?;
        self.seek = self.file.seek(SeekFrom::End(0)).await? as usize;
        Ok(())
    }
}

const DENSITY: u64 = 10;

impl Index {
    async fn sync_entry(
        h: &mut IndexWriteHandler,
        offset: u64,
        position: u64,
    ) -> anyhow::Result<()> {
        if offset < Self::read_last_entry().await?.offset + DENSITY {
            return Ok(());
        }
        h.write_append(Index { offset, position }).await?;
        Ok(())
    }

    pub async fn read_entry(seek: SeekFrom) -> anyhow::Result<Self> {
        let mut file = OpenOptions::new().read(true).open("./index.kr").await?;
        if file.metadata().await?.len() == 0 {
            return Err(anyhow!("index.kr is empty"));
        }
        file.seek(seek).await?;

        let mut buf = [0u8; 16];
        file.read_exact(&mut buf).await?;

        let offset = u64::from_be_bytes(buf[0..8].try_into()?);
        let position = u64::from_be_bytes(buf[8..16].try_into()?);

        Ok(Self { offset, position })
    }

    // read recent entry
    pub async fn read_last_entry() -> anyhow::Result<Self> {
        let seek = SeekFrom::End(-16);
        Ok(Self::read_entry(seek).await.unwrap_or(Index {
            offset: 0,
            position: 0,
        }))
    }
//二分查找算法有问题，明天修复
    pub async fn find_index_range(
        path: &str,
        target: u64,
    ) -> anyhow::Result<(Option<Index>, Option<Index>)> {
        let file = OpenOptions::new().read(true).open(path).await?;
        let len = file.metadata().await?.len();

        ensure!(len % 16 == 0, "index len invalid");

        let mut left = 0;
        let mut right = len / 16;

        // 寻找第一个 offset > target 的索引项
        while left < right {
            let mid = left + (right - left) / 2;
            let Index {
                offset,
                position: _,
            } = Index::read_entry(SeekFrom::Start(mid)).await?;

            if offset <= target {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == 0 {
            return Ok((None, None));
        }
        let l = Index::read_entry(SeekFrom::Start(left - 1)).await?;
        if left != Index::read_last_entry().await?.offset {
            let r = Index::read_entry(SeekFrom::Start(left)).await?;
            return Ok((Some(l), Some(r)));
        }
        Ok((Some(l), None))
    }
}

#[tokio::test]
async fn write() -> anyhow::Result<()> {
    let mut h = IndexWriteHandler::new(String::from("./index.kr")).await?;
    for i in 0..200 {
        Index::sync_entry(&mut h, i, i * 30).await?;
    }
    let r = fs::read("./index.kr")?;
    let v = IndexSerializer::decode_all(&r)?;
    println!("{:#?}", v);
    Ok(())
}

#[tokio::test]
async fn read() -> anyhow::Result<()> {
    println!("the last entry is {:?}",Index::read_last_entry().await?);
    println!("the entry is in {:?}",Index::find_index_range("./index.kr", 0).await?);
    println!("the entry is in {:?}",Index::find_index_range("./index.kr", 111).await?);
    println!("the entry is in {:?}",Index::find_index_range("./index.kr", 200).await?);
    Ok(())
}
