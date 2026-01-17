//! 使用 get_kline_all_during 获取指定时间范围的 K 线数据示例
//! 查询 000001 在 2026-01-01 00:00:00 到 2026-01-10 00:00:00 的日 K 线数据

use tdx_rust::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), ClientError> {
    // 连接服务器
    let client = dial("124.71.187.122").await?;
    println!("连接成功！");

    let code = "000001";

    // 2026-01-01 00:00:00 Beijing Time (UTC+8) -> 1767196800
    // 2026-01-10 00:00:00 Beijing Time (UTC+8) -> 1767974400
    let start_time = 1767196800;
    let end_time = 1767974400;

    println!(
        "正在查询 {} 在 2026-01-01 到 2026-01-10 之间的日K线数据...",
        code
    );

    let klines = client
        .get_kline_all_during(KlineType::Day, code, start_time, end_time)
        .await?;

    println!("查询完成，共获取 {} 条数据", klines.count);
    for (i, k) in klines.list.iter().enumerate() {
        println!("{}. {:?}", i + 1, k);
    }

    Ok(())
}
