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
    pub async fn sycn_entry(offset: u64, position: u64) -> anyhow::Result<()> {
        if offset / DENSITY <= Self::read_last_entry().await?.offset {
            return Ok(());
        }

        file.write_all(&IndexSerializer::encode(offset, position))
            .await?;
        Ok(())
    }

    pub async fn read_entry(file: &mut File, index: u64) -> anyhow::Result<Self> {
        file.seek(SeekFrom::Start(index * 16)).await?;

        let mut buf = [0u8; 16];
        file.read_exact(&mut buf).await?;

        let offset = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let position = u64::from_be_bytes(buf[8..16].try_into().unwrap());

        Ok(Self { offset, position })
    }

    // read recent entry
    pub async fn read_last_entry() -> anyhow::Result<Self> {
        let mut file = OpenOptions::new().read(true).open("index.kr").await?;
        file.seek(SeekFrom::End(-16)).await?;
        let mut buf = [0u8; 16];
        file.read_exact(&mut buf).await?;
        let offset = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let position = u64::from_be_bytes(buf[8..16].try_into().unwrap());
        Ok(Self { offset, position })
    }

    pub async fn find_index_range(
        path: &str,
        target: u64,
    ) -> anyhow::Result<(Option<Index>, Option<Index>)> {
        let mut file = OpenOptions::new().read(true).open(path).await?;
        let len = file.metadata().await?.len();
        // TODO(第7条): 太硬 —— 崩溃留下的半个索引项(8字节)会让整个索引永久报错
        //              应向下取整到 16 的倍数并容忍尾部残缺
        ensure!(len % 16 == 0, "index len invaild");

        let mut left = 0;
        let mut right = len / 16;

        // 寻找第一个 offset > target 的索引项
        while left < right {
            let mid = left + (right - left) / 2;
            let Index {
                offset,
                position: _,
            } = Index::read_entry(&mut file, mid).await?;

            if offset <= target {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == 0 {
            return Ok((None, None));
        }
        let l = Index::read_entry(&mut file, left - 1).await?;
        if left == Index::read_last_entry().await?.offset {
            let r = Index::read_entry(&mut file, left).await?;
            return Ok((Some(l), Some(r)));
        }
        Ok((Some(l), None))
    }
}
