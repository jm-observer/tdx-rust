//! 协议常量定义

/// 请求帧固定前缀
pub const PREFIX: u8 = 0x0C;

/// 响应帧固定前缀（小端序：B1CB7400）
pub const PREFIX_RESP: u32 = 0xB1CB7400;

/// 消息类型常量
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Connect = 0x000D,            // 建立连接
    Heart = 0x0004,              // 心跳
    Gbbq = 0x000F,               // 除权除息
    Count = 0x044E,               // 获取股票数量
    Code = 0x0450,                // 获取股票代码
    Quote = 0x053E,               // 行情信息
    Minute = 0x051D,              // 分时数据
    CallAuction = 0x056A,         // 集合竞价
    MinuteTrade = 0x0FC5,         // 分时交易
    HistoryMinute = 0x0FB4,       // 历史分时数据
    HistoryMinuteTrade = 0x0FB5,  // 历史分时交易
    Kline = 0x052D,               // K线图
}

impl MessageType {
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x000D => Some(MessageType::Connect),
            0x0004 => Some(MessageType::Heart),
            0x000F => Some(MessageType::Gbbq),
            0x044E => Some(MessageType::Count),
            0x0450 => Some(MessageType::Code),
            0x053E => Some(MessageType::Quote),
            0x051D => Some(MessageType::Minute),
            0x056A => Some(MessageType::CallAuction),
            0x0FC5 => Some(MessageType::MinuteTrade),
            0x0FB4 => Some(MessageType::HistoryMinute),
            0x0FB5 => Some(MessageType::HistoryMinuteTrade),
            0x052D => Some(MessageType::Kline),
            _ => None,
        }
    }
}

/// K线类型
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KlineType {
    Minute5 = 0,      // 5分钟K线
    Minute15 = 1,     // 15分钟K线
    Minute30 = 2,     // 30分钟K线
    Minute60 = 3,     // 60分钟K线（1小时）
    Day2 = 4,         // 日K线（需除以100）
    Week = 5,         // 周K线
    Month = 6,        // 月K线
    Minute = 7,       // 1分钟K线
    Minute2 = 8,      // 1分钟K线（变体）
    Day = 9,          // 日K线
    Quarter = 10,     // 季K线
    Year = 11,        // 年K线
}

/// 交易所类型
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exchange {
    SZ = 0,  // 深圳交易所
    SH = 1,  // 上海交易所
    BJ = 2,  // 北京交易所
}

impl Exchange {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Exchange::SZ),
            1 => Some(Exchange::SH),
            2 => Some(Exchange::BJ),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Exchange::SZ => "sz",
            Exchange::SH => "sh",
            Exchange::BJ => "bj",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Exchange::SH => "上海",
            Exchange::SZ => "深圳",
            Exchange::BJ => "北京",
        }
    }
}

/// 控制码
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Control01 = 0x01,  // 通常为 0x01
}

impl Control {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}
