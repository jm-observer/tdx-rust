//! 使用 util_fn 过滤 K 线数据示例
//! 查询 sz000001 从 2026 年开始的日 K 线数据

use std::time::UNIX_EPOCH;
use tdx_rust::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), ClientError> {
    // 连接服务器
    let client = dial("124.71.187.122").await?;
    println!("连接成功！");

    let code = "sz000001";
    println!("正在查询 {} 从 2026-01-01 开始的日K线数据...", code);

    // 定义起始时间戳：2026-01-01 00:00:00 UTC+8 (约为 1767225600)
    // 注意：SystemTime 是 UTC。2026-01-01 00:00:00 UTC+8 = 2025-12-31 16:00:00 UTC
    // 简单起见，我们构造一个 SystemTime
    // 2026-01-01 00:00:00 +0800
    // Timestamp: 1767225600

    // 直接硬编码 timestamp for 2026-01-01
    let target_time = UNIX_EPOCH + std::time::Duration::from_secs(1767225600);

    let klines = client
        .get_kline_all_util(KlineType::Day, code, |k| k.time >= target_time)
        .await?;

    println!("查询完成，共获取 {} 条数据", klines.count);
    for (i, k) in klines.list.iter().enumerate() {
        println!("{}. {:?}", i + 1, k);
    }

    Ok(())
}
