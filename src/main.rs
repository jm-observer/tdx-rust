use tdx_sync_rust::protocol::*;

fn main() {
    println!("TDX Sync Rust - 通达信协议 Rust 实现");
    
    // 示例：创建连接请求
    let connect_frame = Connect::request(1);
    let encoded = connect_frame.encode();
    println!("连接请求帧: {:02X?}", encoded);
    
    // 示例：创建获取股票数量请求
    let count_frame = Count::request(2, Exchange::SH);
    let encoded = count_frame.encode();
    println!("获取股票数量请求帧: {:02X?}", encoded);
}
