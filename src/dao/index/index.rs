#[cfg(test)]
use std::fs;
use std::io::SeekFrom;

use crate::config::CONFIG;
use crate::dao::index::index_serializer::IndexSerializer;
use anyhow::ensure;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

/// 单个稀疏索引项：offset 是消息偏移量，position 是消息在日志文件中的起始字节位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Index {
    pub offset: u64,
    pub position: u64,
}

pub struct IndexWriteHandler {
    file: File,
    /// 下一次写入索引的起始位置（也等于当前索引文件长度）。
    seek: u64,
}

/// 打开索引文件。读取时也允许文件不存在（第一次启动的情况）。
async fn open_index() -> anyhow::Result<File> {
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&CONFIG.dao.index_path)
        .await?)
}

impl IndexWriteHandler {
    pub async fn new() -> anyhow::Result<Self> {
        let mut file = open_index().await?;
        let seek = file.seek(SeekFrom::End(0)).await?;
        Ok(Self { file, seek })
    }

    async fn write_append(&mut self, i: Index) -> anyhow::Result<()> {
        self.file.seek(SeekFrom::Start(self.seek)).await?;
        self.file.write_all(&IndexSerializer::encode(i.offset, i.position)).await?;
        self.seek += 16;
        Ok(())
    }
}

/// 每 DENSITY 条消息记录一个索引项。
const DENSITY: u64 = 10;

impl Index {
    /// 根据消息写入情况决定是否追加索引项。position 必须是这条消息的起始位置。
    pub async fn sync_entry(
        h: &mut IndexWriteHandler,
        offset: u64,
        position: u64,
    ) -> anyhow::Result<()> {
        // 索引为空时必须写入第一条，否则重启后无法定位。
        if h.seek != 0 {
            let last = Self::read_last_entry().await?;
            if offset < last.offset + DENSITY {
                return Ok(());
            }
        }
        h.write_append(Index { offset, position }).await?;
        Ok(())
    }

    async fn read_entry_from(file: &mut File, seek: SeekFrom) -> anyhow::Result<Self> {
        file.seek(seek).await?;

        let mut buf = [0u8; 16];
        file.read_exact(&mut buf).await?;

        let offset = u64::from_be_bytes(buf[0..8].try_into()?);
        let position = u64::from_be_bytes(buf[8..16].try_into()?);

        Ok(Self { offset, position })
    }

    /// 读取最后一个索引项；索引不存在或为空时返回 (0, 0) 作为“日志开头”的虚拟索引。
    pub async fn read_last_entry() -> anyhow::Result<Self> {
        let mut file = open_index().await?;
        let len = file.metadata().await?.len();
        if len < 16 {
            return Ok(Index {
                offset: 0,
                position: 0,
            });
        }
        Self::read_entry_from(&mut file, SeekFrom::End(-16)).await
    }

    /// 返回 target 附近的索引区间：
    /// - `.0` 是 offset <= target 的最后一个索引项（没有则返回虚拟项 0/0）
    /// - `.1` 是 offset > target 的第一个索引项（没有则返回最后一项）
    pub async fn find_index_range(target: u64) -> anyhow::Result<(Index, Index)> {
        let mut file = open_index().await?;
        let len = file.metadata().await?.len();

        if len == 0 {
            return Ok((
                Index {
                    offset: 0,
                    position: 0,
                },
                Index {
                    offset: 0,
                    position: 0,
                },
            ));
        }
        ensure!(len.is_multiple_of(16), "index len invalid: {len}");

        let n = len / 16;
        // 二分查找第一个 offset > target 的索引项
        let mut lo = 0u64;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let entry = Self::read_entry_from(&mut file, SeekFrom::Start(mid * 16)).await?;
            if entry.offset <= target {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        let left = if lo == 0 {
            Index {
                offset: 0,
                position: 0,
            }
        } else {
            Self::read_entry_from(&mut file, SeekFrom::Start((lo - 1) * 16)).await?
        };
        let right = if lo == n {
            Self::read_entry_from(&mut file, SeekFrom::Start((n - 1) * 16)).await?
        } else {
            Self::read_entry_from(&mut file, SeekFrom::Start(lo * 16)).await?
        };

        Ok((left, right))
    }
}

#[tokio::test]
async fn write() -> anyhow::Result<()> {
    let _guard = crate::dao::TEST_LOCK.lock().await;
    let _ = fs::remove_file(&CONFIG.dao.index_path);
    let mut h = IndexWriteHandler::new().await?;
    for i in 0..200 {
        Index::sync_entry(&mut h, i, i * 30).await?;
    }
    let r = fs::read(&CONFIG.dao.index_path)?;
    let v = IndexSerializer::decode_all(&r)?;
    println!("{:#?}", v);
    let _ = fs::remove_file(&CONFIG.dao.index_path);
    Ok(())
}

#[tokio::test]
async fn read() -> anyhow::Result<()> {
    let _guard = crate::dao::TEST_LOCK.lock().await;
    println!("the last entry is {:?}", Index::read_last_entry().await?);
    println!(
        "the entry is in {:?}",
        Index::find_index_range(0).await?
    );
    println!(
        "the entry is in {:?}",
        Index::find_index_range(111).await?
    );
    println!(
        "the entry is in {:?}",
        Index::find_index_range(110).await?
    );
    println!(
        "the entry is in {:?}",
        Index::find_index_range(200).await?
    );
    Ok(())
}
