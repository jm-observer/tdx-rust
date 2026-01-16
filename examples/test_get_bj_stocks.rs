//! 异步查询各个服务器的北京股票数量示例
//!
//! 并发连接默认服务器列表，对每个地址查询北京市场股票数量，
//! 打印查询结果或错误信息。

use tdx_rust::dial::DEFAULT_HOSTS;
use tdx_rust::{dial, ClientError};

type QueryResult = (String, Result<usize, ClientError>);

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    println!("=== 异步查询北京股票数量 ===");
    println!(
        "将并发测试 {} 个服务器地址，输出北京市场股票数量或错误\n",
        DEFAULT_HOSTS.len()
    );

    let mut tasks = Vec::new();

    for host in DEFAULT_HOSTS {
        let host = host.to_string();
        tasks.push(tokio::spawn(async move {
            let result = match dial(&host).await {
                Ok(client) => client.get_bj_stocks().await.map(|stocks| stocks.len()),
                Err(err) => Err(err),
            };
            (host, result)
        }));
    }

    for task in tasks {
        match task.await {
            Ok((host, Ok(count))) => println!("[{}] ✓ 北京股票数量: {}", host, count),
            Ok((host, Err(err))) => println!("[{}] ✗ 查询失败: {}", host, err),
            Err(err) => println!("任务执行失败: {}", err),
        }
    }
}
