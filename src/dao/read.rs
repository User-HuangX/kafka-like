use std::{collections::HashMap, io::SeekFrom, path::Path, sync::Arc};

use anyhow::{Context, Ok, anyhow, ensure};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt},
    sync::RwLock,
};

use crate::dao::data::Record;

// TODO(第8条): read_position/offset 从未使用；new() 传 Some 时白开一次 File；
//              get_index 无调用；find_by_offset/find_index 每次调用都新 open File（已有 self.file）
struct Reader {
    read_position: u64,
    file: File,
    path: String,
    offset: u64,
}

struct Index(u64, u64);
impl Index {
    async fn read_entry(file: &mut File, index: u64) -> anyhow::Result<Self> {
        file.seek(SeekFrom::Start(index * 16)).await?;

        let mut buf = [0u8; 16];
        file.read_exact(&mut buf).await?;

        let offset = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let position = u64::from_be_bytes(buf[8..16].try_into().unwrap());

        Ok(Self(offset, position))
    }

    async fn find_index(path: &str, target: u64) -> anyhow::Result<Option<Index>> {
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
            let Index(offset, _) = Index::read_entry(&mut file, mid).await?;

            if offset <= target {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == 0 {
            return Ok(None);
        }
        let r = Index::read_entry(&mut file, left - 1).await?;
        Ok(Some(r))
    }
}

impl Reader {
    async fn new(path: &str, file: Option<File>) -> anyhow::Result<Self> {
        let p = Path::new(path);
        ensure!(
            p.extension().is_some_and(|ext| ext == "kr"),
            "日志文件必须使用 .kr 扩展名"
        );
        let new_file = OpenOptions::new().read(true).open(path).await?;

        Ok(Self {
            read_position: 0,
            file: file.unwrap_or(new_file),
            path: String::from(path),
            offset: 0,
        })
    }

    #[allow(unreachable_code, unused_variables)]
    async fn get_records(&mut self, r: &mut HashMap<u64, Record>) -> anyhow::Result<()> {
        // TODO(第5条): 每轮循环 metadata() 一次 syscall；vec[i..i+8] 在末尾越界会 panic
        todo!("第5条: get_records 未缓存文件长度，且切片越界会 panic");
        //读取全部，读取时游标需要放到0位置
        self.file.rewind().await?;

        let mut vec: Vec<u8> = Vec::new();
        self.file.read_to_end(&mut vec).await?;
        let mut i = 0;
        // 这里可以改成异步的
        loop {
            let len = usize::from_be_bytes(vec[i..i + 8].try_into()?);
            r.insert(
                u64::from_be_bytes(vec[i + 12..i + 20].try_into()?),
                Record::decode_checked(&vec[i..i + 20 + len])?,
            );
            i = i + 20 + len;
            if i >= usize::try_from(self.file.metadata().await?.len())? {
                break;
            }
        }
        Ok(())
    }

    //先读取稀疏索引，然后再读取
    #[allow(unreachable_code, unused_variables)]
    async fn find_by_offset(&mut self, offset: u64) -> anyhow::Result<Record> {
        // TODO(第1条): 索引只定位到 ≤ offset 的最近一项，还差从 r.1 顺序扫描到目标 offset
        todo!("第1条: find_by_offset 未顺序扫描，target 未命中索引项时会返回错误记录");
        //read through the whole index cache
        let r = Index::find_index(&self.path.to_string(), offset)
            .await?
            .context("没找到这个值")?;
        let mut file = OpenOptions::new()
            .read(true)
            .open(self.path.to_string())
            .await?;

		//后续扫描需要修改
		todo!();
        file.seek(SeekFrom::Start(r.1)).await?;

        let mut buf = vec![0u8; 8];
        file.read_exact(&mut buf).await?;

        let len = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let mut vec = vec![0u8; usize::try_from(12 + len)?];
        file.read_exact(&mut vec).await?;
        buf.append(&mut vec);
        Ok(Record::decode_checked(&buf)?)
    }

    async fn get_index(&self) -> anyhow::Result<Vec<u8>> {
        Ok(fs::read(format!("{}.index", self.path)).await?)
    }
}
