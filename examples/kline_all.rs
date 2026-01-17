//! 获取所有K线数据示例 - 展示各种周期的K线

use tdx_rust::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), ClientError> {
    // 获取当前时间 (北京时间)
    let beijing_offset = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
    let now = chrono::Utc::now().with_timezone(&beijing_offset);
    println!("=== 获取 sz000001 各周期K线数据 ===");
    println!("当前时间: {}\n", now.format("%Y-%m-%d %H:%M:%S"));

    // 连接服务器
    let client = dial("124.71.187.122").await?;
    println!("连接成功！\n");

    let code = "sz000001";

    // 1. 1分钟K线
    println!("【1分钟K线】");
    let klines = client.get_kline(KlineType::Minute, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 2. 5分钟K线
    println!("【5分钟K线】");
    let klines = client.get_kline(KlineType::Minute5, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 3. 15分钟K线
    println!("【15分钟K线】");
    let klines = client.get_kline(KlineType::Minute15, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 4. 30分钟K线
    println!("【30分钟K线】");
    let klines = client.get_kline(KlineType::Minute30, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 5. 60分钟K线（小时线）
    println!("【60分钟K线（小时线）】");
    let klines = client.get_kline(KlineType::Minute60, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 6. 日K线（获取全部）
    println!("【日K线（全部）】");
    let klines = client.get_kline_day_all(code).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 7. 周K线
    println!("【周K线】");
    let klines = client.get_kline(KlineType::Week, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 8. 月K线
    println!("【月K线】");
    let klines = client.get_kline(KlineType::Month, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 9. 季K线
    println!("【季K线】");
    let klines = client.get_kline(KlineType::Quarter, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 10. 年K线
    println!("【年K线】");
    let klines = client.get_kline(KlineType::Year, code, 0, 800).await?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    println!("=== 完成 ===");
    Ok(())
}

fn print_last_5(klines: &KlineResponse) {
    let start = if klines.list.len() > 5 {
        klines.list.len() - 5
    } else {
        0
    };
    for (i, k) in klines.list[start..].iter().enumerate() {
        println!("  {}. {:?}", i + 1, k);
    }
    println!();
}
