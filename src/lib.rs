pub mod protocol;
pub mod client;
pub mod dial;

pub use client::{Client, ClientError, LogLevel};
pub use dial::{dial, dial_default, dial_hosts_range, dial_hosts_random, fast_hosts, DialResult};
pub use protocol::*;

// 重新导出 log 宏供用户使用
pub use log;

/// 初始化日志（使用 env_logger，可通过 RUST_LOG 环境变量控制日志级别）
/// 
/// 示例：
/// ```
/// tdx_sync_rust::init_logger();
/// // 或设置环境变量: RUST_LOG=debug
/// ```
pub fn init_logger() {
    let _ = env_logger::try_init();
}
