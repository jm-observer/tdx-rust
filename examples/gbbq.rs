//! 获取股本变迁数据示例

use tdx_sync_rust::*;

fn main() -> Result<(), ClientError> {
    let client = dial("124.71.187.122")?;
    
    let resp = client.get_gbbq("sz000001")?;
    
    // 使用与 Go 版本一致的格式输出（用于对比）
    for g in &resp.list {
        // println!("{}", g.to_go_format());
        println!("{:?}", g);
    }
    
    println!("总数: {}", resp.count);
    
    Ok(())
}
