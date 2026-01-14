//! 基本使用示例

use tdx_rust::*;

fn main() -> Result<(), ClientError> {
    println!("=== TDX Rust 客户端示例 ===\n");

    // 连接到指定地址
    println!("1. 连接到指定服务器...");
    let client = dial("124.71.187.122")?;
    println!("   连接成功！\n");

    // 获取股票数量
    println!("2. 获取股票数量...");
    let count_sh = client.get_count(Exchange::SH)?;
    let count_sz = client.get_count(Exchange::SZ)?;
    println!("   上海: {} 只, 深圳: {} 只\n", count_sh, count_sz);

    // 获取股票代码列表
    println!("3. 获取股票代码列表...");
    let codes = client.get_code(Exchange::SH, 0)?;
    println!("   获取到 {} 只股票代码", codes.codes.len());
    for (i, code) in codes.codes.iter().take(10).enumerate() {
        println!("     {}. {:?}", i + 1, code);
    }
    println!();

    // 获取行情信息
    println!("4. 获取行情信息（五档报价）...");
    let quotes = client.get_quote(&["sz000001".to_string(), "sh600000".to_string()])?;
    for quote in &quotes {
        println!("   {:?}", quote);
    }
    println!();

    // 获取日K线数据
    println!("5. 获取日K线数据...");
    let klines = client.get_kline_day("sz000001", 0, 5)?;
    println!("   获取到 {} 条日K线", klines.count);
    for k in klines.list.iter().take(5) {
        println!("     {:?}", k);
    }
    println!();

    // 获取分时数据
    println!("6. 获取分时数据...");
    let minute = client.get_minute("sz000001")?;
    println!("   获取到 {} 条分时数据", minute.count);
    for m in minute.list.iter().take(5) {
        println!("     {:?}", m);
    }
    println!();

    // 获取集合竞价数据
    println!("7. 获取集合竞价数据...");
    let mut auction = client.get_call_auction("sz000001")?;
    println!("   获取到 {} 条集合竞价数据", auction.count);
    for a in auction.list.iter().take(10) {
        println!("     {:?}", a);
    }
    auction.list.reverse();
    for a in auction.list.iter().take(10) {
        println!("     {:?}", a);
    }
    println!();

    // 获取股本变迁数据
    println!("8. 获取股本变迁/除权除息数据...");
    let gbbq = client.get_gbbq("sz000001")?;
    println!("   获取到 {} 条股本变迁数据", gbbq.count);
    for g in gbbq.list.iter().take(5) {
        println!("     {:?}", g);
    }
    println!();

    // 发送心跳
    println!("9. 发送心跳包...");
    client.send_heartbeat()?;
    println!("   心跳包发送成功！\n");

    println!("=== 示例完成！===");
    Ok(())
}
