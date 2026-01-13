//! 数据编码/解码工具函数

use crate::protocol::types::Price;
use encoding_rs::GBK;
use std::io::{self, Read};

/// 将字节数组转换为小端序的 u16
pub fn bytes_to_u16_le(bytes: &[u8]) -> u16 {
    if bytes.len() < 2 {
        return 0;
    }
    u16::from_le_bytes([bytes[0], bytes[1]])
}

/// 将字节数组转换为小端序的 u32
pub fn bytes_to_u32_le(bytes: &[u8]) -> u32 {
    if bytes.len() < 4 {
        return 0;
    }
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// 将 u16 转换为小端序字节数组
pub fn u16_to_bytes_le(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

/// 将 u32 转换为小端序字节数组
pub fn u32_to_bytes_le(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

/// 反转字节数组（用于大端序转小端序）
pub fn reverse_bytes(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().rev().copied().collect()
}

/// 将 GBK 编码的字节数组转换为 UTF-8 字符串
pub fn gbk_to_utf8(bytes: &[u8]) -> String {
    let (cow, _, _) = GBK.decode(bytes);
    cow.trim_end_matches('\0').to_string()
}

/// 将 UTF-8 字符串转换为 GBK 编码的字节数组
pub fn utf8_to_gbk(s: &str) -> Vec<u8> {
    let (cow, _, _) = GBK.encode(s);
    cow.to_vec()
}

/// 解析变长整数编码
/// 
/// 第一字节：
/// - 第7位（最高位）：0x80，表示是否有后续字节（1=有，0=无）
/// - 第6位：0x40，表示符号（1=负，0=正）
/// - 低6位：有效数据位
/// 
/// 后续字节：
/// - 第7位：0x80，表示是否有后续字节
/// - 低7位：有效数据位
pub fn decode_varint(bytes: &[u8]) -> (i32, usize) {
    if bytes.is_empty() {
        return (0, 0);
    }

    let mut data: i32 = 0;
    let mut consumed = 0;

    for (i, &byte) in bytes.iter().enumerate() {
        if i == 0 {
            // 第一字节：取低6位
            data += (byte & 0x3F) as i32;
        } else {
            // 后续字节：取低7位，左移相应位数
            data += ((byte & 0x7F) as i32) << (6 + (i - 1) * 7);
        }

        consumed += 1;

        // 判断是否有后续数据
        if byte & 0x80 == 0 {
            break;
        }
    }

    // 第一字节的第6位为1表示负数
    if !bytes.is_empty() && bytes[0] & 0x40 > 0 {
        data = -data;
    }

    (data, consumed)
}

/// 编码变长整数
pub fn encode_varint(value: i32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut val = value.abs();

    // 第一字节
    let mut first_byte = (val & 0x3F) as u8;
    val >>= 6;

    // 设置符号位
    if value < 0 {
        first_byte |= 0x40;
    }

    // 如果有后续数据，设置继续位
    if val > 0 {
        first_byte |= 0x80;
    }

    result.push(first_byte);

    // 后续字节
    while val > 0 {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;

        if val > 0 {
            byte |= 0x80;
        }

        result.push(byte);
    }

    result
}

/// 解析价格（变长编码）
pub fn decode_price(bytes: &[u8]) -> (Price, usize) {
    let (value, consumed) = decode_varint(bytes);
    (Price(value as i64), consumed)
}

/// 解析成交量（特殊浮点数编码）
/// 
/// 使用4字节uint32，通过指数和对数计算
pub fn decode_volume(bytes: &[u8]) -> f64 {
    if bytes.len() < 4 {
        return 0.0;
    }

    let val = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i32;
    let logpoint = (val >> 24) as i32;
    let hleax = ((val >> 16) & 0xff) as i32;
    let lheax = ((val >> 8) & 0xff) as i32;
    let lleax = (val & 0xff) as i32;

    let dw_ecx = logpoint * 2 - 0x7f;
    let dw_edx = logpoint * 2 - 0x86;
    let dw_esi = logpoint * 2 - 0x8e;
    let dw_eax = logpoint * 2 - 0x96;

    let tmp_eax = dw_ecx.abs();
    let dbl_xmm6 = if dw_ecx < 0 {
        1.0 / 2_f64.powi(tmp_eax)
    } else {
        2_f64.powi(tmp_eax)
    };

    let dbl_xmm4 = if hleax > 0x80 {
        let dwtmpeax = dw_edx + 1;
        let tmpdbl_xmm3 = 2_f64.powi(dwtmpeax);
        2_f64.powi(dw_edx) * 128.0 + ((hleax & 0x7f) as f64) * tmpdbl_xmm3
    } else {
        if dw_edx >= 0 {
            2_f64.powi(dw_edx) * hleax as f64
        } else {
            (1.0 / 2_f64.powi(-dw_edx)) * hleax as f64
        }
    };

    let dbl_xmm3 = 2_f64.powi(dw_esi) * lheax as f64;
    let dbl_xmm1 = 2_f64.powi(dw_eax) * lleax as f64;

    if (hleax & 0x80) > 0 {
        dbl_xmm6 + dbl_xmm4 + dbl_xmm3 * 2.0 + dbl_xmm1 * 2.0
    } else {
        dbl_xmm6 + dbl_xmm4 + dbl_xmm3 + dbl_xmm1
    }
}

/// 解析成交量（变体2）
pub fn decode_volume2(bytes: &[u8]) -> f64 {
    if bytes.len() < 4 {
        return 0.0;
    }

    let val = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i32;
    let logpoint = (val >> 24) as i32;
    let hleax = ((val >> 16) & 0xff) as i32;
    let lheax = ((val >> 8) & 0xff) as i32;
    let lleax = (val & 0xff) as i32;

    let dw_ecx = logpoint * 2 - 0x7f;
    let dbl_xmm6 = 2_f64.powi(dw_ecx);

    let dbl_xmm4 = if hleax > 0x80 {
        dbl_xmm6 * (64.0 + (hleax & 0x7f) as f64) / 64.0
    } else {
        dbl_xmm6 * hleax as f64 / 128.0
    };

    let scale = if (hleax & 0x80) != 0 { 2.0 } else { 1.0 };

    let dbl_xmm3 = dbl_xmm6 * lheax as f64 / 32768.0 * scale;
    let dbl_xmm1 = dbl_xmm6 * lleax as f64 / 8388608.0 * scale;

    dbl_xmm6 + dbl_xmm4 + dbl_xmm3 + dbl_xmm1
}

/// 从字节数组读取完整数据（用于响应帧解析）
pub fn read_full_frame<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
    
    let mut prefix = [0u8; 4];
    loop {
        reader.read_exact(&mut prefix)?;
        
        // 检查帧头
        let prefix_val = u32::from_le_bytes(prefix);
        if prefix_val == 0x0074CBB1 {  // B1CB7400 的小端序
            let mut result = prefix.to_vec();
            
            // 读取12字节
            let mut buf = [0u8; 12];
            reader.read_exact(&mut buf)?;
            result.extend_from_slice(&buf);
            
            // 获取后续字节长度
            let length = u16::from_le_bytes([buf[0], buf[1]]);
            let mut data_buf = vec![0u8; length as usize];
            reader.read_exact(&mut data_buf)?;
            result.extend_from_slice(&data_buf);
            
            return Ok(result);
        }
    }
}
