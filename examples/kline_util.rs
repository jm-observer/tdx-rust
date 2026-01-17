//! 使用 util_fn 过滤 K 线数据示例
//! 查询 sz000001 从 2026 年开始的日 K 线数据

use tdx_rust::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), ClientError> {
    // 连接服务器
    let client = dial("124.71.187.122").await?;
    println!("连接成功！");

    let code = "sz000001";
    println!("正在查询 {} 从 2026-01-01 开始的日K线数据...", code);

    // 2026-01-01 00:00:00 +0800
    // Timestamp: 1767196800
    let target_time = 1767196800;

    let klines = client
        .get_kline_all_util(KlineType::Day, code, |k| k.time >= target_time)
        .await?;

    println!("查询完成，共获取 {} 条数据", klines.count);
    for (i, k) in klines.list.iter().enumerate() {
        println!("{}. {:?}", i + 1, k);
    }

    Ok(())
}
