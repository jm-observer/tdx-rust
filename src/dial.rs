//! 连接管理（异步）

use crate::client::Client;
use crate::client::ClientError;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time;

/// 默认服务器地址列表
pub const DEFAULT_HOSTS: &[&str] = &[
    "124.71.187.122",
    "122.51.120.217",
    "111.229.247.189",
    "124.70.176.52",
    "123.60.186.45",
    "122.51.232.182",
    "118.25.98.114",
    "124.70.199.56",
    "121.36.225.169",
    "123.60.70.228",
    "123.60.73.44",
    "124.70.133.119",
];

/// 连接到指定地址
pub async fn dial(addr: &str) -> Result<Client, ClientError> {
    Client::connect(addr).await
}

/// 遍历多个地址进行连接，成功则返回
pub async fn dial_hosts_range(hosts: &[&str]) -> Result<Client, ClientError> {
    let hosts = if hosts.is_empty() {
        DEFAULT_HOSTS
    } else {
        hosts
    };

    let mut last_error = None;
    for host in hosts {
        match Client::connect(host).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = Some(e);
                // 等待2秒后尝试下一个
                time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ClientError::Other("所有服务器连接失败".to_string())))
}

/// 随机选择一个地址连接
pub async fn dial_hosts_random(hosts: &[&str]) -> Result<Client, ClientError> {
    let hosts = if hosts.is_empty() {
        DEFAULT_HOSTS
    } else {
        hosts
    };

    // Use a Send-friendly RNG so this async fn can be spawned onto the multithread runtime.
    let mut rng = StdRng::from_entropy();
    let host = hosts
        .choose(&mut rng)
        .ok_or_else(|| ClientError::Other("没有可用的服务器地址".to_string()))?;

    Client::connect(host).await
}

/// 使用默认连接方式（遍历默认服务器列表）
pub async fn dial_default() -> Result<Client, ClientError> {
    dial_hosts_range(DEFAULT_HOSTS).await
}

/// 连接结果（用于测试连接速度）
#[derive(Debug, Clone)]
pub struct DialResult {
    pub host: String,
    pub duration: Duration,
}

/// 测试多个地址的连接速度并排序
pub async fn fast_hosts(hosts: &[&str]) -> Vec<DialResult> {
    let hosts = if hosts.is_empty() {
        DEFAULT_HOSTS
    } else {
        hosts
    };

    let mut handles = Vec::new();

    for host in hosts {
        let host = host.to_string();
        handles.push(tokio::spawn(async move {
            let addr = if host.contains(':') {
                host.clone()
            } else {
                format!("{}:7709", host)
            };

            let start = Instant::now();
            match TcpStream::connect(&addr).await {
                Ok(_) => Some(DialResult {
                    host,
                    duration: start.elapsed(),
                }),
                Err(_) => None,
            }
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Some(result)) = handle.await {
            results.push(result);
        }
    }

    // 按连接时间排序
    results.sort_by(|a, b| a.duration.cmp(&b.duration));
    results
}
