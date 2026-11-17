use std::io::SeekFrom::{self};

use anyhow::{Ok, ensure};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::dao::index::index_serializer::IndexSerializer;

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
        let file = OpenOptions::new().create(true).open(&path).await?;

        Ok(Self {
            file,
            path,
            seek: 0,
        })
    }
}

const DENSITY: u64 = 10;

impl Index {
    pub async fn sycn_entry(
        h: &mut IndexWriteHandler,
        offset: u64,
        position: u64,
    ) -> anyhow::Result<()> {
        if offset / DENSITY <= Self::read_last_entry().await?.offset {
            return Ok(());
        }
        h.file.seek(SeekFrom::End(0)).await?;
        h.file
            .write_all(&IndexSerializer::encode(offset, position))
            .await?;
        Ok(())
    }

    pub async fn read_entry(seek: SeekFrom) -> anyhow::Result<Self> {
        let mut file = OpenOptions::new().read(true).open("./index.kr").await?;
        file.seek(seek).await?;

        let mut buf = [0u8; 16];
        file.read_exact(&mut buf).await?;

        let offset = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let position = u64::from_be_bytes(buf[8..16].try_into().unwrap());

        Ok(Self { offset, position })
    }

    // read recent entry
    pub async fn read_last_entry() -> anyhow::Result<Self> {
        let seek = SeekFrom::End(16);
        Ok(Self::read_entry(seek).await?)
    }

    pub async fn find_index_range(
        path: &str,
        target: u64,
    ) -> anyhow::Result<(Option<Index>, Option<Index>)> {
        let file = OpenOptions::new().read(true).open(path).await?;
        let len = file.metadata().await?.len();

        ensure!(len % 16 == 0, "index len invaild");

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

