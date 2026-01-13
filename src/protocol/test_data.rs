//! 测试数据结构定义

use serde::{Deserialize, Serialize};

#[cfg(feature = "test-data")]
use hex;

/// 测试数据文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestData {
    /// 接口名称
    pub name: String,
    /// 类型名称
    #[serde(rename = "type")]
    pub type_name: String,
    /// 类型值（十六进制）
    #[serde(rename = "type_value")]
    pub type_value: String,
    /// 接口说明
    pub description: String,
    /// 请求帧的十六进制字符串
    pub request: String,
    /// 请求帧说明
    #[serde(default)]
    pub request_description: Option<String>,
    /// 请求数据域（不含帧头）
    #[serde(default)]
    pub request_data: Option<String>,
    /// 响应帧的十六进制字符串（完整帧）
    pub response: String,
    /// 响应帧说明
    #[serde(default)]
    pub response_description: Option<String>,
    /// 响应数据域（解压后，不含帧头）
    #[serde(default)]
    pub response_data: Option<String>,
    /// 请求参数说明
    #[serde(default)]
    pub params: serde_json::Value,
    /// 其他说明
    #[serde(default)]
    pub notes: Option<String>,
}

impl TestData {
    /// 解码请求帧的十六进制字符串
    #[cfg(feature = "test-data")]
    pub fn decode_request(&self) -> Result<Vec<u8>, hex::FromHexError> {
        hex::decode(self.request.replace(" ", ""))
    }

    /// 解码响应帧的十六进制字符串
    #[cfg(feature = "test-data")]
    pub fn decode_response(&self) -> Result<Vec<u8>, hex::FromHexError> {
        let response = self.response.replace(" ", "");
        if response.contains("[") {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        hex::decode(response)
    }

    /// 解码请求数据域
    #[cfg(feature = "test-data")]
    pub fn decode_request_data(&self) -> Result<Option<Vec<u8>>, hex::FromHexError> {
        self.request_data
            .as_ref()
            .map(|s| hex::decode(s.replace(" ", "")))
            .transpose()
    }

    /// 解码响应数据域
    #[cfg(feature = "test-data")]
    pub fn decode_response_data(&self) -> Result<Option<Vec<u8>>, hex::FromHexError> {
        self.response_data
            .as_ref()
            .filter(|s| !s.contains("["))
            .map(|s| hex::decode(s.replace(" ", "")))
            .transpose()
    }
}
