//! 协议帧格式定义和编解码

use crate::protocol::{
    constants::{Control, MessageType, PREFIX},
    codec::{bytes_to_u16_le, bytes_to_u32_le, u16_to_bytes_le, u32_to_bytes_le},
};
use flate2::read::ZlibDecoder;
use std::io::Read;
use thiserror::Error;

/// 请求帧
#[derive(Debug, Clone)]
pub struct RequestFrame {
    pub msg_id: u32,
    pub control: Control,
    pub msg_type: MessageType,
    pub data: Vec<u8>,
}

impl RequestFrame {
    /// 创建新的请求帧
    pub fn new(msg_id: u32, msg_type: MessageType, data: Vec<u8>) -> Self {
        Self {
            msg_id,
            control: Control::Control01,
            msg_type,
            data,
        }
    }

    /// 编码为字节数组
    pub fn encode(&self) -> Vec<u8> {
        let length = (self.data.len() + 2) as u16;
        let mut result = Vec::with_capacity(12 + self.data.len());

        // Prefix
        result.push(PREFIX);

        // MsgID (小端序)
        result.extend_from_slice(&u32_to_bytes_le(self.msg_id));

        // Control
        result.push(self.control.as_u8());

        // Length (重复两次，小端序)
        result.extend_from_slice(&u16_to_bytes_le(length));
        result.extend_from_slice(&u16_to_bytes_le(length));

        // Type (小端序)
        result.extend_from_slice(&u16_to_bytes_le(self.msg_type.as_u16()));

        // Data
        result.extend_from_slice(&self.data);

        result
    }

    /// 从字节数组解码
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        if bytes.len() < 12 {
            return Err(FrameError::InsufficientData);
        }

        if bytes[0] != PREFIX {
            return Err(FrameError::InvalidPrefix);
        }

        let msg_id = bytes_to_u32_le(&bytes[1..5]);
        let _control = bytes[5];
        let length1 = bytes_to_u16_le(&bytes[6..8]);
        let length2 = bytes_to_u16_le(&bytes[8..10]);
        let msg_type_val = bytes_to_u16_le(&bytes[10..12]);

        if length1 != length2 {
            return Err(FrameError::LengthMismatch);
        }

        // Length 字段包含 Type 字段的 2 字节，所以数据域长度 = length1 - 2
        let data_length = length1.saturating_sub(2) as usize;

        if bytes.len() < 12 + data_length {
            return Err(FrameError::InsufficientData);
        }

        let msg_type = MessageType::from_u16(msg_type_val)
            .ok_or(FrameError::UnknownMessageType(msg_type_val))?;

        let data = bytes[12..12 + data_length].to_vec();

        Ok(Self {
            msg_id,
            control: Control::Control01, // 通常都是 0x01
            msg_type,
            data,
        })
    }
}

/// 响应帧
#[derive(Debug, Clone)]
pub struct ResponseFrame {
    pub prefix: u32,
    pub control: u8,
    pub msg_id: u32,
    pub unknown: u8,
    pub msg_type: MessageType,
    pub zip_length: u16,
    pub length: u16,
    pub data: Vec<u8>,
    decompressed: bool,
}

impl ResponseFrame {
    /// 创建响应帧（未解压）
    pub fn new(
        prefix: u32,
        control: u8,
        msg_id: u32,
        unknown: u8,
        msg_type: MessageType,
        zip_length: u16,
        length: u16,
        data: Vec<u8>,
    ) -> Self {
        Self {
            prefix,
            control,
            msg_id,
            unknown,
            msg_type,
            zip_length,
            length,
            data,
            decompressed: false,
        }
    }

    /// 解压数据
    pub fn decompress(&mut self) -> Result<(), FrameError> {
        if self.decompressed {
            return Ok(());
        }

        // 如果压缩长度 != 未压缩长度，需要解压
        if self.zip_length != self.length {
            let mut decoder = ZlibDecoder::new(self.data.as_slice());
            let mut decompressed = Vec::with_capacity(self.length as usize);
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| FrameError::DecompressionError(e.to_string()))?;
            self.data = decompressed;
        }

        // 验证解压后的数据长度
        if self.data.len() != self.length as usize {
            return Err(FrameError::LengthMismatch);
        }

        self.decompressed = true;
        Ok(())
    }

    /// 获取解压后的数据
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl ResponseFrame {
    /// 从字节数组解码
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        if bytes.len() < 16 {
            return Err(FrameError::InsufficientData);
        }

        // 前缀是大端序：B1CB7400
        let prefix = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let control = bytes[4];
        let msg_id = bytes_to_u32_le(&bytes[5..9]);
        let unknown = bytes[9];
        let msg_type_val = bytes_to_u16_le(&bytes[10..12]);
        let zip_length = bytes_to_u16_le(&bytes[12..14]);
        let length = bytes_to_u16_le(&bytes[14..16]);

        // 检查帧头
        use crate::protocol::constants::PREFIX_RESP;
        if prefix != PREFIX_RESP {
            return Err(FrameError::InvalidPrefix);
        }

        if bytes.len() < 16 + zip_length as usize {
            return Err(FrameError::InsufficientData);
        }

        let msg_type = MessageType::from_u16(msg_type_val)
            .ok_or(FrameError::UnknownMessageType(msg_type_val))?;

        let data = bytes[16..16 + zip_length as usize].to_vec();

        let mut frame = Self {
            prefix,
            control,
            msg_id,
            unknown,
            msg_type,
            zip_length,
            length,
            data,
            decompressed: false,
        };

        // 解压数据
        frame.decompress()?;

        Ok(frame)
    }

    /// 检查响应是否成功
    pub fn is_success(&self) -> bool {
        self.control & 0x10 == 0x10
    }
}

/// 帧错误类型
#[derive(Debug, Error)]
pub enum FrameError {
    #[error("数据长度不足")]
    InsufficientData,
    #[error("无效的帧头")]
    InvalidPrefix,
    #[error("长度不匹配")]
    LengthMismatch,
    #[error("未知的消息类型: 0x{0:04X}")]
    UnknownMessageType(u16),
    #[error("解压错误: {0}")]
    DecompressionError(String),
}
