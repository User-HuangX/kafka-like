use std::{
    collections::HashMap,
    io::SeekFrom::{self},
    path::Path,
    vec,
};

use anyhow::{Context, Error, Ok, ensure};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt},
};

use crate::dao::{data::Record, index::Index};

// TODO(第8条): read_position/offset 从未使用；new() 传 Some 时白开一次 File；
//              get_index 无调用；find_by_offset/find_index 每次调用都新 open File（已有 self.file）
struct Reader {
    read_position: u64,
    file: File,
    path: String,
    offset: u64,
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

    async fn get_records(
        &mut self,
        r: &mut HashMap<u64, Record>,
        start: Option<u64>,
        end: Option<u64>,
    ) -> anyhow::Result<()> {
        //读取全部，读取时游标需要放到0位置
        self.file
            .seek(SeekFrom::Start(start.unwrap_or_default()))
            .await?;

        let mut vec: Vec<u8> = vec![
            0u8;
            end.unwrap_or(self.file.metadata().await?.len())
                .try_into()
                .unwrap()
        ];
        self.file.read_exact(&mut vec).await?;
        let mut i = 0;

        loop {
            let len = usize::from_be_bytes(vec[i..i + 8].try_into()?);
            r.insert(
                u64::from_be_bytes(vec[i + 12..i + 20].try_into()?),
                Record::decode_checked(&vec[i..i + 20 + len])?,
            );
            i = i + 20 + len;
            if i >= vec.len() {
                break;
            }
        }
        Ok(())
    }

    //先读取稀疏索引，然后再读取
    async fn find_by_offset(&mut self, offset: u64) -> anyhow::Result<Record> {
        //read through the whole index cache
        let (l, r) = Index::find_index_range(&self.path.to_string(), offset).await?;

        if l.is_none() && r.is_none() {
            let _ = Error::msg("can't find the right position");
        }

        let mut file = OpenOptions::new()
            .read(true)
            .open(self.path.to_string())
            .await?;
        let start = l.as_ref().unwrap().position;
        let end = r.as_ref().unwrap().position;
        file.seek(SeekFrom::Start(start)).await?;
        let mut map = HashMap::new();
        self.get_records(&mut map, Some(start), Some(end)).await?;
        let r = map.remove(&offset).context("no record")?;

        Ok(r)
    }

    async fn get_index(&self) -> anyhow::Result<Vec<u8>> {
        Ok(fs::read(format!("{}.index", self.path)).await?)
    }
}
