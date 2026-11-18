use crate::config::CONFIG;
use crate::dao::data::RecordSerializer;
use crate::dao::index::index::IndexWriteHandler;
use crate::dao::index::index_serializer::IndexSerializer;
use crate::dao::read::Reader;
use crate::dao::write::Writer;

mod data;
mod index;
mod read;
mod write;

// 所有会读写 msg.kr / index.kr 的测试共用这把锁，避免并行测试互相删文件。
#[cfg(test)]
pub(crate) static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_dao() -> anyhow::Result<()> {
    let _guard = TEST_LOCK.lock().await;
    // 测试前清空旧文件，避免上一次运行的脏数据干扰。
    let _ = std::fs::remove_file(&CONFIG.dao.path);
    let _ = std::fs::remove_file(&CONFIG.dao.index_path);

    let index_w = IndexWriteHandler::new().await?;
    let mut w = Writer::new(index_w).await?;
    w.write_append("你好".to_string()).await?;
    w.write_append("rust".to_string()).await?;
    w.write_append("   ".to_string()).await?;
    w.write_append("\n".to_string()).await?;
    w.write_append("??".to_string()).await?;
    w.write_append("".to_string()).await?;

    let s = std::fs::read(&CONFIG.dao.path)?;
    let all = RecordSerializer::decode(&s)?;
    assert_eq!(all.len(), 6);
    assert_eq!(all.iter().map(|r| r.offset).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);

    let last = Reader::find_last().await?;
    assert_eq!(last.offset, 6);
    assert!(last.payload.is_empty());

    let r = Reader::get_records(Some(1), Some(3)).await?;
    assert_eq!(r.iter().map(|r| r.offset).collect::<Vec<_>>(), vec![1, 2]);

    let r = Reader::get_records(Some(2), None).await?;
    assert_eq!(
        r.iter().map(|r| r.offset).collect::<Vec<_>>(),
        vec![2, 3, 4, 5, 6]
    );

    let r = Reader::get_records(None, Some(3)).await?;
    assert_eq!(r.iter().map(|r| r.offset).collect::<Vec<_>>(), vec![1, 2]);

    assert_eq!(Reader::find_by_offset(6).await?.offset, 6);
    assert_eq!(Reader::find_by_offset(4).await?.payload, b"\n");

    let _ = std::fs::remove_file(&CONFIG.dao.path);
    let _ = std::fs::remove_file(&CONFIG.dao.index_path);
    Ok(())
}

#[tokio::test]
async fn test_restart_recovery() -> anyhow::Result<()> {
    let _guard = TEST_LOCK.lock().await;
    let _ = std::fs::remove_file(&CONFIG.dao.path);
    let _ = std::fs::remove_file(&CONFIG.dao.index_path);

    {
        let h = IndexWriteHandler::new().await?;
        let mut w = Writer::new(h).await?;
        for i in 0..25 {
            w.write_append(format!("m{i}")).await?;
        }
    }

    // 模拟进程重启：重新打开文件和索引，offset 必须接着之前的最后一条继续。
    {
        let h = IndexWriteHandler::new().await?;
        let mut w = Writer::new(h).await?;
        assert_eq!(Reader::find_last().await?.offset, 25);
        for i in 25..30 {
            w.write_append(format!("m{i}")).await?;
        }
    }

    let all = Reader::get_records(None, None).await?;
    assert_eq!(all.len(), 30);
    assert_eq!(all.last().unwrap().offset, 30);
    assert_eq!(Reader::find_by_offset(17).await?.payload, b"m16");

    let index = IndexSerializer::decode_all(&std::fs::read(&CONFIG.dao.index_path)?)?;
    assert_eq!(index[0].offset, 1);
    assert_eq!(index.iter().map(|i| i.offset).collect::<Vec<_>>(), vec![1, 11, 21]);

    let _ = std::fs::remove_file(&CONFIG.dao.path);
    let _ = std::fs::remove_file(&CONFIG.dao.index_path);
    Ok(())
}
