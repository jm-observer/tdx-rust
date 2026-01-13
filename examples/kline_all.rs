//! 获取所有K线数据示例 - 展示各种周期的K线

use tdx_sync_rust::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<(), ClientError> {
    // 获取当前时间
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = (time_of_day / 3600 + 8) % 24; // UTC+8
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    
    // 简单计算年月日
    let mut year = 1970i32;
    let mut remaining = days as i32;
    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        year += 1;
    }
    let days_in_month = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for d in days_in_month.iter() {
        if remaining < *d { break; }
        remaining -= d;
        month += 1;
    }
    let day = remaining + 1;
    
    println!("=== 获取 sz000001 各周期K线数据 ===");
    println!("当前时间: {:04}-{:02}-{:02} {:02}:{:02}:{:02}\n", year, month, day, hours, minutes, seconds);

    // 连接服务器
    let client = dial("124.71.187.122")?;
    println!("连接成功！\n");

    let code = "sz000001";

    // 1. 1分钟K线
    println!("【1分钟K线】");
    let klines = client.get_kline(KlineType::Minute, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 2. 5分钟K线
    println!("【5分钟K线】");
    let klines = client.get_kline(KlineType::Minute5, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 3. 15分钟K线
    println!("【15分钟K线】");
    let klines = client.get_kline(KlineType::Minute15, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 4. 30分钟K线
    println!("【30分钟K线】");
    let klines = client.get_kline(KlineType::Minute30, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 5. 60分钟K线（小时线）
    println!("【60分钟K线（小时线）】");
    let klines = client.get_kline(KlineType::Minute60, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 6. 日K线（获取全部）
    println!("【日K线（全部）】");
    let klines = client.get_kline_day_all(code)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 7. 周K线
    println!("【周K线】");
    let klines = client.get_kline(KlineType::Week, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 8. 月K线
    println!("【月K线】");
    let klines = client.get_kline(KlineType::Month, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 9. 季K线
    println!("【季K线】");
    let klines = client.get_kline(KlineType::Quarter, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    // 10. 年K线
    println!("【年K线】");
    let klines = client.get_kline(KlineType::Year, code, 0, 800)?;
    println!("共 {} 条，最近5条:", klines.count);
    print_last_5(&klines);

    println!("=== 完成 ===");
    Ok(())
}

fn print_last_5(klines: &KlineResponse) {
    let start = if klines.list.len() > 5 { klines.list.len() - 5 } else { 0 };
    for (i, k) in klines.list[start..].iter().enumerate() {
        println!("  {}. {:?}", i + 1, k);
    }
    println!();
}
