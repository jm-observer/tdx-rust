//! 连接管理

use crate::client::Client;
use crate::client::ClientError;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::time::{Duration, Instant};

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
pub fn dial(addr: &str) -> Result<Client, ClientError> {
    Client::connect(addr)
}

/// 遍历多个地址进行连接，成功则返回
pub fn dial_hosts_range(hosts: &[&str]) -> Result<Client, ClientError> {
    let hosts = if hosts.is_empty() {
        DEFAULT_HOSTS
    } else {
        hosts
    };

    let mut last_error = None;
    for host in hosts {
        match Client::connect(host) {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = Some(e);
                // 等待2秒后尝试下一个
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        ClientError::Other("所有服务器连接失败".to_string())
    }))
}

/// 随机选择一个地址连接
pub fn dial_hosts_random(hosts: &[&str]) -> Result<Client, ClientError> {
    let hosts = if hosts.is_empty() {
        DEFAULT_HOSTS
    } else {
        hosts
    };

    let mut rng = thread_rng();
    let host = hosts
        .choose(&mut rng)
        .ok_or_else(|| ClientError::Other("没有可用的服务器地址".to_string()))?;

    Client::connect(host)
}

/// 使用默认连接方式（遍历默认服务器列表）
pub fn dial_default() -> Result<Client, ClientError> {
    dial_hosts_range(DEFAULT_HOSTS)
}

/// 连接结果（用于测试连接速度）
#[derive(Debug, Clone)]
pub struct DialResult {
    pub host: String,
    pub duration: Duration,
}

/// 测试多个地址的连接速度并排序
pub fn fast_hosts(hosts: &[&str]) -> Vec<DialResult> {
    use std::sync::mpsc;
    use std::thread;

    let hosts = if hosts.is_empty() {
        DEFAULT_HOSTS
    } else {
        hosts
    };

    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();

    for host in hosts {
        let tx = tx.clone();
        let host = host.to_string();
        let handle = thread::spawn(move || {
            let addr = if host.contains(':') {
                host.clone()
            } else {
                format!("{}:7709", host)
            };

            let start = Instant::now();
            match std::net::TcpStream::connect(&addr) {
                Ok(_) => {
                    let duration = start.elapsed();
                    let _ = tx.send(DialResult { host, duration });
                }
                Err(_) => {}
            }
        });
        handles.push(handle);
    }

    drop(tx);

    let mut results = Vec::new();
    while let Ok(result) = rx.recv() {
        results.push(result);
    }

    for handle in handles {
        let _ = handle.join();
    }

    // 按连接时间排序
    results.sort_by(|a, b| a.duration.cmp(&b.duration));
    results
}
