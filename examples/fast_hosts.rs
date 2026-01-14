//! 测试服务器连接速度示例

use tdx_rust::*;

fn main() {
    println!("=== 测试服务器连接速度 ===\n");

    let hosts = &[
        "124.71.187.122",
        "122.51.120.217",
        "111.229.247.189",
        "124.70.176.52",
        "123.60.186.45",
    ];

    println!("正在测试 {} 个服务器地址...\n", hosts.len());

    let results = fast_hosts(hosts);

    if results.is_empty() {
        println!("没有可用的服务器地址");
        return;
    }

    println!("连接速度排序（从快到慢）:");
    println!("{:-<60}", "");
    for (i, result) in results.iter().enumerate() {
        println!(
            "{}. {} - {:.2}ms",
            i + 1,
            result.host,
            result.duration.as_secs_f64() * 1000.0
        );
    }
    println!("{:-<60}", "");

    // 使用最快的服务器连接
    if let Some(fastest) = results.first() {
        println!("\n使用最快的服务器连接: {}", fastest.host);
        match dial(&fastest.host) {
            Ok(client) => {
                println!("连接成功！");
                // 测试获取股票数量
                match client.get_count(Exchange::SH) {
                    Ok(count) => println!("上海交易所股票数量: {}", count),
                    Err(e) => println!("获取股票数量失败: {}", e),
                }
            }
            Err(e) => println!("连接失败: {}", e),
        }
    }
}
