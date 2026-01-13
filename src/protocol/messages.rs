//! 各种消息类型的编解码实现

use crate::protocol::{
    constants::{Exchange, KlineType, MessageType},
    codec::{
        bytes_to_u16_le, bytes_to_u32_le, decode_price, decode_varint, decode_volume2, gbk_to_utf8,
        u16_to_bytes_le, u32_to_bytes_le,
    },
    frame::RequestFrame,
    types::{
        CallAuction, CallAuctionResponse, Gbbq, GbbqResponse, K, Kline, KlineCache, KlineResponse,
        MinuteResponse, Price, PriceLevel, PriceNumber, QuoteInfo, StockCode, Trade, TradeResponse,
        TradeStatus,
    },
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// 消息编解码错误
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("数据长度不足")]
    InsufficientData,
    #[error("无效的股票代码: {0}")]
    InvalidCode(String),
    #[error("解析错误: {0}")]
    ParseError(String),
}

/// 连接消息
pub struct Connect;

impl Connect {
    /// 创建连接请求帧
    pub fn request(msg_id: u32) -> RequestFrame {
        RequestFrame::new(msg_id, MessageType::Connect, vec![0x01])
    }

    /// 解码连接响应
    pub fn decode_response(data: &[u8]) -> Result<String, MessageError> {
        if data.len() < 68 {
            return Err(MessageError::InsufficientData);
        }
        // 前68字节未知，后续为GBK编码的字符串信息
        let info = gbk_to_utf8(&data[68..]);
        Ok(info)
    }
}

/// 心跳消息
pub struct Heartbeat;

impl Heartbeat {
    /// 创建心跳请求帧
    pub fn request(msg_id: u32) -> RequestFrame {
        RequestFrame::new(msg_id, MessageType::Heart, vec![])
    }
}

/// 获取股票数量消息
pub struct Count;

impl Count {
    /// 创建获取股票数量请求帧
    pub fn request(msg_id: u32, exchange: Exchange) -> RequestFrame {
        let data = vec![exchange.as_u8(), 0x00, 0x75, 0xC7, 0x33, 0x01];
        RequestFrame::new(msg_id, MessageType::Count, data)
    }

    /// 解码股票数量响应
    pub fn decode_response(data: &[u8]) -> Result<u16, MessageError> {
        if data.len() < 2 {
            return Err(MessageError::InsufficientData);
        }
        Ok(bytes_to_u16_le(data))
    }
}

/// 获取股票代码列表消息
pub struct Code;

impl Code {
    /// 创建获取股票代码列表请求帧
    pub fn request(msg_id: u32, exchange: Exchange, start: u16) -> RequestFrame {
        let mut data = vec![exchange.as_u8(), 0x00];
        data.extend_from_slice(&u16_to_bytes_le(start));
        RequestFrame::new(msg_id, MessageType::Code, data)
    }

    /// 解码股票代码列表响应
    pub fn decode_response(data: &[u8]) -> Result<CodeResponse, MessageError> {
        if data.len() < 2 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[0..2]);
        let mut codes = Vec::new();
        let mut offset = 2;

        for _ in 0..count {
            if offset + 29 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            let code_str = String::from_utf8_lossy(&data[offset..offset + 6]).to_string();
            let multiple = bytes_to_u16_le(&data[offset + 6..offset + 8]);
            let name_bytes = &data[offset + 8..offset + 16];
            let name = gbk_to_utf8(name_bytes);
            let decimal = data[offset + 20] as i8;
            let last_price = decode_volume2(&data[offset + 21..offset + 25]);

            codes.push(StockCode {
                name: name.clone(),
                code: code_str.clone(),
                multiple,
                decimal,
                last_price,
            });

            offset += 29;
        }

        Ok(CodeResponse { count, codes })
    }
}

/// 股票代码列表响应
#[derive(Debug, Clone)]
pub struct CodeResponse {
    pub count: u16,
    pub codes: Vec<StockCode>,
}

/// 行情信息消息
pub struct Quote;

impl Quote {
    /// 创建行情信息请求帧
    pub fn request(msg_id: u32, codes: &[String]) -> Result<RequestFrame, MessageError> {
        let mut data = vec![0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        data.extend_from_slice(&u16_to_bytes_le(codes.len() as u16));

        for code_str in codes {
            let (exchange, code_num) = decode_code(code_str)?;
            data.push(exchange.as_u8());
            data.extend_from_slice(code_num.as_bytes());
        }

        Ok(RequestFrame::new(msg_id, MessageType::Quote, data))
    }

    /// 解码行情信息响应
    pub fn decode_response(data: &[u8]) -> Result<Vec<QuoteInfo>, MessageError> {
        if data.len() < 4 {
            return Err(MessageError::InsufficientData);
        }

        // 前2字节未知（可能是版本或其他标识），第3-4字节是数量（小端序）
        let mut offset = 2; // 跳过前2字节
        let count = bytes_to_u16_le(&data[offset..offset + 2]);
        offset += 2;

        let mut quotes = Vec::new();

        for _ in 0..count {
            if offset + 9 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            // 交易所：0=深圳，1=上海，2=北京
            let exchange_val = data[offset];
            let exchange = Exchange::from_u8(exchange_val)
                .ok_or_else(|| MessageError::ParseError(format!("无效的交易所: {}", exchange_val)))?;
            offset += 1;

            // 股票代码（6字节）
            let code_bytes = &data[offset..offset + 6];
            let code = gbk_to_utf8(code_bytes);
            offset += 6;

            let active1 = bytes_to_u16_le(&data[offset..offset + 2]);
            offset += 2;

            // 解析K线数据
            let (k, k_consumed) = decode_k(&data[offset..])?;
            offset += k_consumed;

            // ReversedBytes0 (变长整数) - 服务器时间
            let (reversed0, consumed) = decode_varint(&data[offset..]);
            offset += consumed;
            let server_time = format!("{}", reversed0);

            // ReversedBytes1 (变长整数)
            let (_reversed1, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // TotalHand (变长整数)
            let (total_hand, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // Intuition (变长整数)
            let (intuition, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // Amount (4字节，特殊浮点编码)
            let amount = decode_volume2(&data[offset..offset + 4]);
            offset += 4;

            // InsideDish (变长整数)
            let (inside_dish, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // OuterDisc (变长整数)
            let (outer_disc, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // ReversedBytes2 (变长整数)
            let (_reversed2, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // ReversedBytes3 (变长整数)
            let (_reversed3, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 5档买卖盘
            let mut buy_level = [PriceLevel { buy: true, price: Price(0), number: 0 }; 5];
            let mut sell_level = [PriceLevel { buy: false, price: Price(0), number: 0 }; 5];

            for i in 0..5 {
                // 买价差值
                let (buy_price_diff, consumed) = decode_price(&data[offset..]);
                offset += consumed;
                buy_level[i].price = Price(buy_price_diff.0 * 10 + k.close.0);

                // 卖价差值
                let (sell_price_diff, consumed) = decode_price(&data[offset..]);
                offset += consumed;
                sell_level[i].price = Price(sell_price_diff.0 * 10 + k.close.0);

                // 买量
                let (buy_num, consumed) = decode_varint(&data[offset..]);
                offset += consumed;
                buy_level[i].number = buy_num;

                // 卖量
                let (sell_num, consumed) = decode_varint(&data[offset..]);
                offset += consumed;
                sell_level[i].number = sell_num;
            }

            // ReversedBytes4 (2字节)
            offset += 2;

            // ReversedBytes5 ~ 8 (变长整数)
            for _ in 0..4 {
                let (_val, consumed) = decode_varint(&data[offset..]);
                offset += consumed;
            }

            // ReversedBytes9 (2字节) - Rate
            let rate_raw = bytes_to_u16_le(&data[offset..offset + 2]);
            let rate = rate_raw as f64 / 100.0;
            offset += 2;

            // Active2 (2字节)
            let active2 = bytes_to_u16_le(&data[offset..offset + 2]);
            offset += 2;

            quotes.push(QuoteInfo {
                exchange,
                code,
                active1,
                k,
                server_time,
                total_hand,
                intuition,
                amount,
                inside_dish,
                outer_disc,
                buy_level,
                sell_level,
                rate,
                active2,
            });
        }

        Ok(quotes)
    }
}

/// 解码K线数据（简化版）
/// 返回 (K线数据, 消耗的字节数)
fn decode_k(data: &[u8]) -> Result<(K, usize), MessageError> {
    if data.is_empty() {
        return Err(MessageError::InsufficientData);
    }

    let mut offset = 0;
    
    // 当日收盘价差值（一般2字节）
    let (close_diff, consumed1) = decode_price(&data[offset..]);
    offset += consumed1;
    
    // 前日收盘价差值（一般1字节）
    let (last_diff, consumed2) = decode_price(&data[offset..]);
    offset += consumed2;
    
    // 当日开盘价差值（一般1字节）
    let (open_diff, consumed3) = decode_price(&data[offset..]);
    offset += consumed3;
    
    // 当日最高价差值（一般1字节）
    let (high_diff, consumed4) = decode_price(&data[offset..]);
    offset += consumed4;
    
    // 当日最低价差值（一般1字节）
    let (low_diff, consumed5) = decode_price(&data[offset..]);
    offset += consumed5;

    // 根据 Go 代码逻辑：K线价格是累加的
    // Last = Last + Close
    // Open = Close + Open
    // Close = Close
    // High = Close + High
    // Low = Close + Low
    let close = Price(close_diff.0 * 10);
    let last = Price(close.0 + last_diff.0 * 10);
    let open = Price(close.0 + open_diff.0 * 10);
    let high = Price(close.0 + high_diff.0 * 10);
    let low = Price(close.0 + low_diff.0 * 10);

    Ok((
        K {
            last,
            open,
            high,
            low,
            close,
        },
        offset,
    ))
}

/// 解码股票代码
pub fn decode_code(code: &str) -> Result<(Exchange, String), MessageError> {
    let code = code.to_lowercase();
    if code.len() < 2 {
        return Err(MessageError::InvalidCode(code));
    }

    let exchange = match &code[..2] {
        "sh" => Exchange::SH,
        "sz" => Exchange::SZ,
        "bj" => Exchange::BJ,
        _ => return Err(MessageError::InvalidCode(code)),
    };

    if code.len() < 8 {
        return Err(MessageError::InvalidCode(code));
    }

    Ok((exchange, code[2..].to_string()))
}

/// 添加交易所前缀
pub fn add_prefix(code: &str) -> String {
    let code = code.to_lowercase();
    if code.starts_with("sh") || code.starts_with("sz") || code.starts_with("bj") {
        return code;
    }
    // 根据代码前缀自动添加交易所
    if code.starts_with("6") || code.starts_with("9") {
        format!("sh{}", code)
    } else if code.starts_with("0") || code.starts_with("3") || code.starts_with("2") {
        format!("sz{}", code)
    } else if code.starts_with("4") || code.starts_with("8") {
        format!("bj{}", code)
    } else {
        format!("sz{}", code)
    }
}

/// 判断是否为股票代码
pub fn is_stock(code: &str) -> bool {
    let code = add_prefix(code);
    if code.len() < 8 {
        return false;
    }
    let num = &code[2..];
    match &code[..2] {
        "sh" => num.starts_with("6") || num.starts_with("688"),
        "sz" => num.starts_with("0") || num.starts_with("3"),
        "bj" => num.starts_with("4") || num.starts_with("8"),
        _ => false,
    }
}

/// 判断是否为ETF
pub fn is_etf(code: &str) -> bool {
    let code = add_prefix(code);
    if code.len() < 8 {
        return false;
    }
    let num = &code[2..];
    match &code[..2] {
        "sh" => num.starts_with("51") || num.starts_with("56") || num.starts_with("58"),
        "sz" => num.starts_with("15") || num.starts_with("16"),
        _ => false,
    }
}

/// 判断是否为指数
pub fn is_index(code: &str) -> bool {
    let code = add_prefix(code);
    if code.len() < 8 {
        return false;
    }
    let num = &code[2..];
    match &code[..2] {
        "sh" => num.starts_with("000") || num.starts_with("880"),
        "sz" => num.starts_with("399"),
        "bj" => num.starts_with("899"),
        _ => false,
    }
}

// ==================== K线数据消息 ====================

/// K线数据消息
pub struct KlineMsg;

impl KlineMsg {
    /// 创建K线数据请求帧
    pub fn request(msg_id: u32, kline_type: KlineType, code: &str, start: u16, count: u16) -> Result<RequestFrame, MessageError> {
        if count > 800 {
            return Err(MessageError::ParseError("单次数量不能超过800".to_string()));
        }

        let (exchange, number) = decode_code(code)?;

        let mut data = vec![exchange.as_u8(), 0x00];
        data.extend_from_slice(number.as_bytes());
        data.push(kline_type as u8);
        data.push(0x00);
        data.extend_from_slice(&[0x01, 0x00]);
        data.extend_from_slice(&u16_to_bytes_le(start));
        data.extend_from_slice(&u16_to_bytes_le(count));
        data.extend_from_slice(&[0u8; 10]); // 未知字段

        Ok(RequestFrame::new(msg_id, MessageType::Kline, data))
    }

    /// 解码K线数据响应
    pub fn decode_response(data: &[u8], cache: KlineCache) -> Result<KlineResponse, MessageError> {
        if data.len() < 2 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[0..2]);
        let mut offset = 2;
        let mut list = Vec::with_capacity(count as usize);
        let mut last_price = Price(0);

        for _ in 0..count {
            if offset + 4 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            // 解析时间（4字节）
            let time = decode_kline_time(&data[offset..offset + 4], cache.kline_type);
            offset += 4;

            // 解析价格差值
            let (open_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;
            let (close_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;
            let (high_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;
            let (low_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;

            // 计算实际价格
            let open = Price(last_price.0 + open_diff.0);
            let close = Price(last_price.0 + open_diff.0 + close_diff.0);
            let high = Price(last_price.0 + open_diff.0 + high_diff.0);
            let low = Price(last_price.0 + open_diff.0 + low_diff.0);

            // 成交量（4字节）
            if offset + 4 > data.len() {
                return Err(MessageError::InsufficientData);
            }
            let mut volume = decode_volume2(&data[offset..offset + 4]) as i64;
            offset += 4;

            // 分钟级K线成交量需要除以100
            match cache.kline_type {
                0 | 1 | 2 | 3 | 4 | 7 | 8 => volume /= 100,
                _ => {}
            }

            // 成交额（4字节）
            if offset + 4 > data.len() {
                return Err(MessageError::InsufficientData);
            }
            let amount = Price((decode_volume2(&data[offset..offset + 4]) * 1000.0) as i64);
            offset += 4;

            // 如果是指数，还有额外4字节（上涨/下跌数量）
            let (up_count, down_count) = if cache.is_index {
                if offset + 4 > data.len() {
                    return Err(MessageError::InsufficientData);
                }
                volume *= 100;
                let up = bytes_to_u16_le(&data[offset..offset + 2]) as i32;
                let down = bytes_to_u16_le(&data[offset + 2..offset + 4]) as i32;
                offset += 4;
                (up, down)
            } else {
                (0, 0)
            };

            last_price = close;

            list.push(Kline {
                last: last_price,
                open,
                high,
                low,
                close,
                order: 0,
                volume,
                amount,
                time,
                up_count,
                down_count,
            });
        }

        Ok(KlineResponse { count, list })
    }
}

/// 解码K线时间
fn decode_kline_time(data: &[u8], kline_type: u8) -> SystemTime {
    // 根据K线类型解析时间
    let (year, month, day, hour, minute) = match kline_type {
        // 分钟级K线：前2字节是年月日压缩格式，后2字节是小时分钟
        // TypeKlineMinute=7, TypeKlineMinute2=8, TypeKline5Minute=0, 
        // TypeKline15Minute=1, TypeKline30Minute=2, TypeKline60Minute=3, TypeKlineDay2=4
        0 | 1 | 2 | 3 | 4 | 7 | 8 => {
            let year_month_day = bytes_to_u16_le(&data[0..2]);
            let hour_minute = bytes_to_u16_le(&data[2..4]);
            
            // year = (yearMonthDay >> 11) + 2004
            // month = (yearMonthDay % 2048) / 100
            // day = (yearMonthDay % 2048) % 100
            let year = ((year_month_day >> 11) + 2004) as i32;
            let month = ((year_month_day % 2048) / 100) as u32;
            let day = ((year_month_day % 2048) % 100) as u32;
            let hour = (hour_minute / 60) as u32;
            let minute = (hour_minute % 60) as u32;
            (year, month, day, hour, minute)
        }
        // 日线及以上：4字节是 YYYYMMDD 格式
        _ => {
            let val = bytes_to_u32_le(data);
            let year = (val / 10000) as i32;
            let month = ((val % 10000) / 100) as u32;
            let day = (val % 100) as u32;
            (year, month, day, 15, 0)
        }
    };

    // 转换为 SystemTime
    let days_since_epoch = days_from_date(year, month, day);
    let secs = days_since_epoch as u64 * 86400 + hour as u64 * 3600 + minute as u64 * 60;
    UNIX_EPOCH + Duration::from_secs(secs)
}

/// 计算从1970年1月1日到指定日期的天数（修正版）
fn days_from_date(year: i32, month: u32, day: u32) -> i64 {
    // 使用更准确的儒略日算法
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;
    
    // 调整月份（1、2月算作上一年的13、14月）
    let (y2, m2) = if m <= 2 {
        (y - 1, m + 12)
    } else {
        (y, m)
    };
    
    // 计算儒略日
    let jd = 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 + (153 * m2 - 457) / 5 + d - 306;
    
    // 1970-01-01 的儒略日偏移
    jd - 719163
}

// ==================== 分时数据消息 ====================

/// 分时数据消息
pub struct MinuteMsg;

impl MinuteMsg {
    /// 创建分时数据请求帧
    pub fn request(msg_id: u32, code: &str) -> Result<RequestFrame, MessageError> {
        let (exchange, number) = decode_code(code)?;

        let mut data = vec![exchange.as_u8(), 0x00];
        data.extend_from_slice(number.as_bytes());
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

        Ok(RequestFrame::new(msg_id, MessageType::Minute, data))
    }

    /// 解码分时数据响应
    /// 
    /// Go 代码参考（model_history_minute.go）：
    /// - 前 2 字节是数量
    /// - 2-6 字节未知
    /// - 每条记录：价格差值(GetPrice) + 未知(GetPrice) + 成交量(CutInt)
    /// - 价格是累加的，且要乘以 10
    /// - 时间从 09:30 开始，使用 i+1 分钟
    /// - 当 i==120 时额外加 90 分钟
    pub fn decode_response(data: &[u8]) -> Result<MinuteResponse, MessageError> {
        if data.len() < 6 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[0..2]);
        let mut offset = 6; // 前2字节是数量，2-6字节未知
        let mut list = Vec::with_capacity(count as usize);
        let mut last_price = Price(0);

        // Go 实现（model_history_minute.go）：
        // t := time.Date(0, 0, 0, 9, 30, 0, 0, time.Local)  // 从 09:30 开始
        // lastPrice := Price(0)
        // multiple := Price(1) * 10
        // for i := uint16(0); i < resp.Count; i++ {
        //     bs, price = GetPrice(bs)
        //     bs, _ = GetPrice(bs)  // 未知字段
        //     lastPrice += price
        //     bs, number = CutInt(bs)
        //     if i == 120 { t = t.Add(time.Minute * 90) }
        //     Time: t.Add(time.Minute * time.Duration(i+1)).Format("15:04")
        //     Price: lastPrice * multiple
        // }
        for i in 0..count {
            // 价格差值
            let (price_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;

            // 未知字段（也用 GetPrice 解码）
            let (_unknown, consumed) = decode_price(&data[offset..]);
            offset += consumed;

            // 累加价格
            last_price = Price(last_price.0 + price_diff.0);

            // 成交量
            let (number, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 计算时间：从 09:30 开始，使用 i+1 分钟
            // i=0 -> 09:31, i=1 -> 09:32, ..., i=119 -> 11:30
            // i=120 时 t += 90 分钟 (09:30 + 90 = 11:00)
            // i=120 -> 11:00 + 121分钟 = 13:01, i=121 -> 13:02, ...
            let total_minutes = if i < 120 {
                9 * 60 + 30 + (i + 1) as u32  // 09:30 + (i+1) 分钟
            } else {
                11 * 60 + (i + 1) as u32      // 11:00 + (i+1) 分钟
            };
            let time = format!("{:02}:{:02}", total_minutes / 60, total_minutes % 60);

            // 价格乘以 10（multiple）
            let price = Price(last_price.0 * 10);

            list.push(PriceNumber {
                time,
                price,
                number,
            });
        }

        Ok(MinuteResponse { count, list })
    }
}

// ==================== 历史分时数据消息 ====================

/// 历史分时数据消息
pub struct HistoryMinuteMsg;

impl HistoryMinuteMsg {
    /// 创建历史分时数据请求帧
    /// date格式：YYYYMMDD
    pub fn request(msg_id: u32, date: &str, code: &str) -> Result<RequestFrame, MessageError> {
        let (exchange, number) = decode_code(code)?;
        let date_num: u32 = date.parse().map_err(|_| MessageError::ParseError("无效的日期格式".to_string()))?;

        let mut data = u32_to_bytes_le(date_num).to_vec();
        data.push(exchange.as_u8());
        data.extend_from_slice(number.as_bytes());

        Ok(RequestFrame::new(msg_id, MessageType::HistoryMinute, data))
    }

    /// 解码历史分时数据响应
    /// 与 MinuteMsg::decode_response 格式相同
    pub fn decode_response(data: &[u8]) -> Result<MinuteResponse, MessageError> {
        MinuteMsg::decode_response(data)
    }
}

// ==================== 分时交易消息 ====================

/// 分时交易消息
pub struct TradeMsg;

/// 交易缓存信息
#[derive(Debug, Clone)]
pub struct TradeCache {
    pub date: String,   // 日期 YYYYMMDD
    pub code: String,   // 股票代码
}

impl TradeMsg {
    /// 创建分时交易请求帧
    pub fn request(msg_id: u32, code: &str, start: u16, count: u16) -> Result<RequestFrame, MessageError> {
        let (exchange, number) = decode_code(code)?;

        let mut data = vec![exchange.as_u8(), 0x00];
        data.extend_from_slice(number.as_bytes());
        data.extend_from_slice(&u16_to_bytes_le(start));
        data.extend_from_slice(&u16_to_bytes_le(count));

        Ok(RequestFrame::new(msg_id, MessageType::MinuteTrade, data))
    }

    /// 解码分时交易响应
    pub fn decode_response(data: &[u8], cache: &TradeCache) -> Result<TradeResponse, MessageError> {
        if data.len() < 2 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[0..2]);
        let mut offset = 2;
        let mut list = Vec::with_capacity(count as usize);
        let mut last_price = Price(0);

        for _ in 0..count {
            if offset + 2 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            // 时间（2字节）
            let time_val = bytes_to_u16_le(&data[offset..offset + 2]);
            let hour = time_val / 60;
            let minute = time_val % 60;
            offset += 2;

            // 价格差值
            let (price_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;
            last_price = Price(last_price.0 + price_diff.0 * 10);

            // 成交量
            let (volume, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 单数
            let (number, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 状态
            let (status_val, consumed) = decode_varint(&data[offset..]);
            offset += consumed;
            let status = match status_val {
                0 => TradeStatus::Buy,
                1 => TradeStatus::Sell,
                _ => TradeStatus::Neutral,
            };

            // 未知字段
            let (_unknown, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 构造时间
            let time = parse_datetime(&cache.date, hour as u32, minute as u32, 0);

            list.push(Trade {
                time,
                price: last_price,
                volume,
                status,
                number,
            });
        }

        Ok(TradeResponse { count, list })
    }
}

// ==================== 历史分时交易消息 ====================

/// 历史分时交易消息
pub struct HistoryTradeMsg;

impl HistoryTradeMsg {
    /// 创建历史分时交易请求帧
    pub fn request(msg_id: u32, date: &str, code: &str, start: u16, count: u16) -> Result<RequestFrame, MessageError> {
        let (exchange, number) = decode_code(code)?;
        let date_num: u32 = date.parse().map_err(|_| MessageError::ParseError("无效的日期格式".to_string()))?;

        let mut data = u32_to_bytes_le(date_num).to_vec();
        data.push(exchange.as_u8());
        data.push(0x00);
        data.extend_from_slice(number.as_bytes());
        data.extend_from_slice(&u16_to_bytes_le(start));
        data.extend_from_slice(&u16_to_bytes_le(count));

        Ok(RequestFrame::new(msg_id, MessageType::HistoryMinuteTrade, data))
    }

    /// 解码历史分时交易响应
    pub fn decode_response(data: &[u8], cache: &TradeCache) -> Result<TradeResponse, MessageError> {
        if data.len() < 6 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[0..2]);
        let mut offset = 6; // 前2字节数量，2-6字节未知
        let mut list = Vec::with_capacity(count as usize);
        let mut last_price = Price(0);

        for _ in 0..count {
            if offset + 2 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            // 时间（2字节）
            let time_val = bytes_to_u16_le(&data[offset..offset + 2]);
            let hour = time_val / 60;
            let minute = time_val % 60;
            offset += 2;

            // 价格差值
            let (price_diff, consumed) = decode_price(&data[offset..]);
            offset += consumed;
            last_price = Price(last_price.0 + price_diff.0 * 10);

            // 成交量
            let (volume, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 状态
            let (status_val, consumed) = decode_varint(&data[offset..]);
            offset += consumed;
            let status = match status_val {
                0 => TradeStatus::Buy,
                1 => TradeStatus::Sell,
                _ => TradeStatus::Neutral,
            };

            // 未知字段
            let (_unknown, consumed) = decode_varint(&data[offset..]);
            offset += consumed;

            // 构造时间
            let time = parse_datetime(&cache.date, hour as u32, minute as u32, 0);

            list.push(Trade {
                time,
                price: last_price,
                volume,
                status,
                number: 0, // 历史数据无单数
            });
        }

        Ok(TradeResponse { count, list })
    }
}

// ==================== 集合竞价消息 ====================

/// 集合竞价消息
pub struct CallAuctionMsg;

impl CallAuctionMsg {
    /// 创建集合竞价请求帧
    pub fn request(msg_id: u32, code: &str) -> Result<RequestFrame, MessageError> {
        let (exchange, number) = decode_code(code)?;

        let mut data = vec![exchange.as_u8(), 0x00];
        data.extend_from_slice(number.as_bytes());
        data.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x00,
            0x03, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0xf4, 0x01, 0x00, 0x00,
        ]);

        Ok(RequestFrame::new(msg_id, MessageType::CallAuction, data))
    }

    /// 解码集合竞价响应
    pub fn decode_response(data: &[u8]) -> Result<CallAuctionResponse, MessageError> {
        if data.len() < 2 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[0..2]);
        let mut offset = 2;
        let mut list = Vec::with_capacity(count as usize);

        for _ in 0..count {
            if offset + 16 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            let n = bytes_to_u16_le(&data[offset..offset + 2]);
            let hour = n / 60;
            let minute = n % 60;

            // 价格（float32）
            let price_f32 = f32::from_le_bytes([data[offset + 2], data[offset + 3], data[offset + 4], data[offset + 5]]);
            let price = Price((price_f32 * 1000.0) as i64);

            // 匹配量
            let matched = bytes_to_u32_le(&data[offset + 6..offset + 10]) as i64;

            // 未匹配量（有符号）
            let unmatched_raw = bytes_to_u16_le(&data[offset + 10..offset + 12]) as i16;
            let (unmatched, flag) = if unmatched_raw < 0 {
                ((-unmatched_raw) as i64, -1i8)
            } else {
                (unmatched_raw as i64, 1i8)
            };

            let second = data[offset + 15] as u32;

            // 构造时间（使用当天日期）
            let now = SystemTime::now();
            let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
            let days = duration.as_secs() / 86400;
            let time = UNIX_EPOCH + Duration::from_secs(
                days * 86400 + hour as u64 * 3600 + minute as u64 * 60 + second as u64
            );

            list.push(CallAuction {
                time,
                price,
                matched,
                unmatched,
                flag,
            });

            offset += 16;
        }

        Ok(CallAuctionResponse { count, list })
    }
}

// ==================== 股本变迁消息 ====================

/// 股本变迁消息
pub struct GbbqMsg;

impl GbbqMsg {
    /// 创建股本变迁请求帧
    pub fn request(msg_id: u32, code: &str) -> Result<RequestFrame, MessageError> {
        let (exchange, number) = decode_code(code)?;

        let mut data = vec![0x01, 0x00];
        data.push(exchange.as_u8());
        data.extend_from_slice(number.as_bytes());

        Ok(RequestFrame::new(msg_id, MessageType::Gbbq, data))
    }

    /// 解码股本变迁响应
    pub fn decode_response(data: &[u8]) -> Result<GbbqResponse, MessageError> {
        if data.len() < 11 {
            return Err(MessageError::InsufficientData);
        }

        let count = bytes_to_u16_le(&data[9..11]);
        let mut offset = 11;
        let mut list = Vec::with_capacity(count as usize);

        for _ in 0..count {
            if offset + 29 > data.len() {
                return Err(MessageError::InsufficientData);
            }

            // 交易所 + 代码
            let exchange = Exchange::from_u8(data[offset]).unwrap_or(Exchange::SZ);
            let code_str = String::from_utf8_lossy(&data[offset + 1..offset + 7]).to_string();
            let code = format!("{}{}", exchange.as_str(), code_str);

            // 时间（4字节，日期格式）
            let time_val = bytes_to_u32_le(&data[offset + 8..offset + 12]);
            let year = (time_val / 10000) as i32;
            let month = ((time_val % 10000) / 100) as u32;
            let day = (time_val % 100) as u32;
            let days = days_from_date(year, month, day);
            let time = UNIX_EPOCH + Duration::from_secs((days as u64) * 86400 + 15 * 3600);

            let category = data[offset + 12] as i32;
            offset += 13;

            // 根据类别解析4个浮点数
            let (c1, c2, c3, c4) = match category {
                1 => {
                    // 除权除息：分红、配股价、送转股、配股
                    let c1 = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as f64;
                    let c2 = f32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]) as f64;
                    let c3 = f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]) as f64;
                    let c4 = f32::from_le_bytes([data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15]]) as f64;
                    (c1, c2, c3, c4)
                }
                11 | 12 => {
                    // 扩缩股
                    let c3 = f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]) as f64;
                    (0.0, 0.0, c3, 0.0)
                }
                13 | 14 => {
                    // 权证
                    let c1 = f32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as f64;
                    let c3 = f32::from_le_bytes([data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]]) as f64;
                    (c1, 0.0, c3, 0.0)
                }
                _ => {
                    // 股本变化：前流通、前总股本、后流通、后总股本
                    let c1 = decode_volume2(&data[offset..offset + 4]) * 1e4;
                    let c2 = decode_volume2(&data[offset + 4..offset + 8]) * 1e4;
                    let c3 = decode_volume2(&data[offset + 8..offset + 12]) * 1e4;
                    let c4 = decode_volume2(&data[offset + 12..offset + 16]) * 1e4;
                    (c1, c2, c3, c4)
                }
            };

            offset += 16;

            list.push(Gbbq {
                code,
                time,
                category,
                c1,
                c2,
                c3,
                c4,
            });
        }

        Ok(GbbqResponse { count, list })
    }
}

/// 解析日期时间字符串为 SystemTime
fn parse_datetime(date: &str, hour: u32, minute: u32, second: u32) -> SystemTime {
    if date.len() != 8 {
        return UNIX_EPOCH;
    }
    let year: i32 = date[0..4].parse().unwrap_or(1970);
    let month: u32 = date[4..6].parse().unwrap_or(1);
    let day: u32 = date[6..8].parse().unwrap_or(1);

    let days = days_from_date(year, month, day);
    UNIX_EPOCH + Duration::from_secs(
        (days as u64) * 86400 + hour as u64 * 3600 + minute as u64 * 60 + second as u64
    )
}
