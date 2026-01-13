//! TDX 客户端实现

use crate::protocol::*;
use log::debug;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    None,
    Error,
    Info,
    Debug,
}

/// TDX 客户端
pub struct Client {
    stream: Arc<Mutex<TcpStream>>,
    msg_id: Arc<Mutex<u32>>,
    timeout: Duration,
    log_level: LogLevel,
}

/// 客户端错误
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("IO错误: {0}")]
    Io(#[from] io::Error),
    #[error("协议错误: {0}")]
    Protocol(#[from] FrameError),
    #[error("消息错误: {0}")]
    Message(#[from] MessageError),
    #[error("超时")]
    Timeout,
    #[error("连接已关闭")]
    Disconnected,
    #[error("其他错误: {0}")]
    Other(String),
}

impl Client {
    /// 连接到指定地址
    pub fn connect(addr: &str) -> Result<Self, ClientError> {
        Self::connect_with_log(addr, LogLevel::None)
    }

    /// 连接到指定地址（带日志级别）
    pub fn connect_with_log(addr: &str, log_level: LogLevel) -> Result<Self, ClientError> {
        let addr = if addr.contains(':') {
            addr.to_string()
        } else {
            format!("{}:7709", addr)
        };

        let stream = TcpStream::connect(&addr)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;

        let client = Self {
            stream: Arc::new(Mutex::new(stream)),
            msg_id: Arc::new(Mutex::new(0)),
            timeout: Duration::from_secs(10),
            log_level,
        };

        // 发送连接请求并读取响应
        client.send_connect()?;

        Ok(client)
    }

    /// 设置日志级别
    pub fn set_log_level(&mut self, level: LogLevel) {
        self.log_level = level;
    }

    /// 发送连接请求并读取响应
    fn send_connect(&self) -> Result<(), ClientError> {
        let frame = Connect::request(1);
        let data = frame.encode();
        self.write_all(&data)?;
        
        // 读取连接响应（但不处理，只是消费掉）
        let _response = self.read_response()?;
        Ok(())
    }

    /// 写入数据
    fn write_all(&self, data: &[u8]) -> Result<(), ClientError> {
        if self.log_level == LogLevel::Debug {
            debug!("发送请求帧 ({} 字节): {:02X?}", data.len(), data);
        }
        
        let mut stream = self.stream.lock().unwrap();
        stream.write_all(data)?;
        stream.flush()?;
        Ok(())
    }

    /// 读取响应帧
    fn read_response(&self) -> Result<ResponseFrame, ClientError> {
        let mut stream = self.stream.lock().unwrap();
        
        // 读取响应帧头（16字节）
        let mut header = [0u8; 16];
        stream.read_exact(&mut header)?;
        
        // 前缀是大端序：B1CB7400
        let prefix = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        if prefix != PREFIX_RESP {
            return Err(ClientError::Protocol(FrameError::InvalidPrefix));
        }
        
        let msg_type_val = bytes_to_u16_le(&header[10..12]);
        let zip_length = bytes_to_u16_le(&header[12..14]);
        let length = bytes_to_u16_le(&header[14..16]);
        
        let msg_type = MessageType::from_u16(msg_type_val)
            .ok_or_else(|| ClientError::Protocol(FrameError::UnknownMessageType(msg_type_val)))?;
        
        // 读取压缩数据
        let mut compressed_data = vec![0u8; zip_length as usize];
        stream.read_exact(&mut compressed_data)?;
        
        if self.log_level == LogLevel::Debug {
            debug!("接收响应: 类型={:?}, 压缩长度={}, 长度={}", msg_type, zip_length, length);
        }
        
        // 构建响应帧
        let mut response = ResponseFrame::new(
            prefix,
            header[4],
            bytes_to_u32_le(&header[5..9]),
            header[9],
            msg_type,
            zip_length,
            length,
            compressed_data,
        );
        
        // 解压数据
        response.decompress()?;
        
        Ok(response)
    }

    /// 发送帧并等待响应
    pub fn send_frame(&self, frame: RequestFrame) -> Result<ResponseFrame, ClientError> {
        let msg_id = {
            let mut id = self.msg_id.lock().unwrap();
            *id += 1;
            *id
        };

        // 设置消息ID
        let mut frame = frame;
        frame.msg_id = msg_id;

        // 发送请求
        let data = frame.encode();
        self.write_all(&data)?;

        // 读取响应
        let response = self.read_response()?;
        
        // 验证消息ID匹配
        if response.msg_id != msg_id {
            return Err(ClientError::Other(format!(
                "消息ID不匹配: 期望 {}, 得到 {}",
                msg_id, response.msg_id
            )));
        }

        Ok(response)
    }

    /// 获取股票数量
    pub fn get_count(&self, exchange: Exchange) -> Result<u16, ClientError> {
        let frame = Count::request(self.next_msg_id(), exchange);
        let response = self.send_frame(frame)?;
        let count = Count::decode_response(response.data())?;
        Ok(count)
    }

    /// 获取股票代码列表（单次最多1000条）
    pub fn get_code(&self, exchange: Exchange, start: u16) -> Result<CodeResponse, ClientError> {
        let frame = Code::request(self.next_msg_id(), exchange, start);
        let response = self.send_frame(frame)?;
        let codes = Code::decode_response(response.data())?;
        Ok(codes)
    }

    /// 获取所有股票代码（从0开始）
    pub fn get_code_all(&self, exchange: Exchange) -> Result<CodeResponse, ClientError> {
        self.get_code_all_from(exchange, 0)
    }

    /// 获取所有股票代码（从指定位置开始）
    pub fn get_code_all_from(&self, exchange: Exchange, from_start: u16) -> Result<CodeResponse, ClientError> {
        let mut all_codes = CodeResponse { count: 0, codes: Vec::new() };
        let batch_size = 1000u16;
        let mut start = from_start;

        loop {
            let resp = self.get_code(exchange, start)?;
            all_codes.count += resp.count;
            all_codes.codes.extend(resp.codes);
            
            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_codes)
    }

    /// 获取行情信息（五档报价）
    pub fn get_quote(&self, codes: &[String]) -> Result<Vec<QuoteInfo>, ClientError> {
        let frame = Quote::request(self.next_msg_id(), codes)?;
        let response = self.send_frame(frame)?;
        let quotes = Quote::decode_response(response.data())?;
        Ok(quotes)
    }

    /// 发送心跳
    pub fn send_heartbeat(&self) -> Result<(), ClientError> {
        let frame = Heartbeat::request(self.next_msg_id());
        let _response = self.send_frame(frame)?;
        Ok(())
    }

    // ==================== K线数据 ====================

    /// 获取K线数据（单次最多800条）
    pub fn get_kline(&self, kline_type: KlineType, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        let code = add_prefix(code);
        let frame = KlineMsg::request(self.next_msg_id(), kline_type, &code, start, count)?;
        let response = self.send_frame(frame)?;
        let cache = KlineCache { kline_type: kline_type as u8, is_index: is_index(&code) };
        let klines = KlineMsg::decode_response(response.data(), cache)?;
        Ok(klines)
    }

    /// 获取所有K线数据（从0开始，通过多次请求拼接）
    pub fn get_kline_all(&self, kline_type: KlineType, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(kline_type, code, 0)
    }

    /// 获取所有K线数据（从指定位置开始，通过多次请求拼接）
    pub fn get_kline_all_from(&self, kline_type: KlineType, code: &str, from_start: u16) -> Result<KlineResponse, ClientError> {
        let mut all_klines = KlineResponse { count: 0, list: Vec::new() };
        let batch_size = 800u16;
        let mut start = from_start;

        loop {
            let resp = self.get_kline(kline_type, code, start, batch_size)?;
            all_klines.count += resp.count;
            // 新数据在前，旧数据在后
            let mut new_list = resp.list;
            new_list.append(&mut all_klines.list);
            all_klines.list = new_list;
            
            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_klines)
    }

    /// 获取1分钟K线数据
    pub fn get_kline_minute(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute, code, start, count)
    }

    /// 获取5分钟K线数据
    pub fn get_kline_5minute(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute5, code, start, count)
    }

    /// 获取15分钟K线数据
    pub fn get_kline_15minute(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute15, code, start, count)
    }

    /// 获取30分钟K线数据
    pub fn get_kline_30minute(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute30, code, start, count)
    }

    /// 获取60分钟K线数据
    pub fn get_kline_60minute(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute60, code, start, count)
    }

    /// 获取日K线数据
    pub fn get_kline_day(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Day, code, start, count)
    }

    /// 获取所有日K线数据
    pub fn get_kline_day_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all(KlineType::Day, code)
    }

    /// 获取所有日K线数据（从指定位置开始）
    pub fn get_kline_day_all_from(&self, code: &str, from_start: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(KlineType::Day, code, from_start)
    }

    /// 获取周K线数据
    pub fn get_kline_week(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Week, code, start, count)
    }

    /// 获取所有周K线数据
    pub fn get_kline_week_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all(KlineType::Week, code)
    }

    /// 获取所有周K线数据（从指定位置开始）
    pub fn get_kline_week_all_from(&self, code: &str, from_start: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(KlineType::Week, code, from_start)
    }

    /// 获取月K线数据
    pub fn get_kline_month(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Month, code, start, count)
    }

    /// 获取所有月K线数据
    pub fn get_kline_month_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all(KlineType::Month, code)
    }

    /// 获取所有月K线数据（从指定位置开始）
    pub fn get_kline_month_all_from(&self, code: &str, from_start: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(KlineType::Month, code, from_start)
    }

    /// 获取季K线数据
    pub fn get_kline_quarter(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Quarter, code, start, count)
    }

    /// 获取年K线数据
    pub fn get_kline_year(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Year, code, start, count)
    }

    // ==================== 指数K线数据 ====================

    /// 获取指数K线数据
    pub fn get_index(&self, kline_type: KlineType, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        let code = add_prefix(code);
        let frame = KlineMsg::request(self.next_msg_id(), kline_type, &code, start, count)?;
        let response = self.send_frame(frame)?;
        let cache = KlineCache { kline_type: kline_type as u8, is_index: true };
        let klines = KlineMsg::decode_response(response.data(), cache)?;
        Ok(klines)
    }

    /// 获取所有指数K线数据（从0开始）
    pub fn get_index_all(&self, kline_type: KlineType, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_index_all_from(kline_type, code, 0)
    }

    /// 获取所有指数K线数据（从指定位置开始）
    pub fn get_index_all_from(&self, kline_type: KlineType, code: &str, from_start: u16) -> Result<KlineResponse, ClientError> {
        let mut all_klines = KlineResponse { count: 0, list: Vec::new() };
        let batch_size = 800u16;
        let mut start = from_start;

        loop {
            let resp = self.get_index(kline_type, code, start, batch_size)?;
            all_klines.count += resp.count;
            let mut new_list = resp.list;
            new_list.append(&mut all_klines.list);
            all_klines.list = new_list;
            
            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_klines)
    }

    /// 获取指数日K线数据
    pub fn get_index_day(&self, code: &str, start: u16, count: u16) -> Result<KlineResponse, ClientError> {
        self.get_index(KlineType::Day, code, start, count)
    }

    /// 获取所有指数日K线数据
    pub fn get_index_day_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_index_all(KlineType::Day, code)
    }

    /// 获取所有指数日K线数据（从指定位置开始）
    pub fn get_index_day_all_from(&self, code: &str, from_start: u16) -> Result<KlineResponse, ClientError> {
        self.get_index_all_from(KlineType::Day, code, from_start)
    }

    // ==================== 分时数据 ====================

    /// 获取分时数据
    /// 获取分时数据（使用历史分时接口，与 Go 版本一致）
    pub fn get_minute(&self, code: &str) -> Result<MinuteResponse, ClientError> {
        // Go 版本的 GetMinute 实际调用的是 GetHistoryMinute(today, code)
        let today = Self::today_str();
        self.get_history_minute(&today, code)
    }

    /// 获取当前日期字符串（YYYYMMDD格式）
    fn today_str() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let days = secs / 86400;
        
        // 简单计算年月日
        let mut year = 1970i32;
        let mut remaining = days as i32;
        loop {
            let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
            if remaining < days_in_year { break; }
            remaining -= days_in_year;
            year += 1;
        }
        let days_in_month = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        let mut month = 1u32;
        for d in days_in_month.iter() {
            if remaining < *d { break; }
            remaining -= d;
            month += 1;
        }
        let day = remaining + 1;
        
        format!("{:04}{:02}{:02}", year, month, day)
    }

    /// 获取历史分时数据
    /// date格式：YYYYMMDD
    pub fn get_history_minute(&self, date: &str, code: &str) -> Result<MinuteResponse, ClientError> {
        let code = add_prefix(code);
        let frame = HistoryMinuteMsg::request(self.next_msg_id(), date, &code)?;
        let response = self.send_frame(frame)?;
        let minute = HistoryMinuteMsg::decode_response(response.data())?;
        Ok(minute)
    }

    // ==================== 交易数据 ====================

    /// 获取分时交易详情（单次最多1800条）
    pub fn get_trade(&self, code: &str, start: u16, count: u16) -> Result<TradeResponse, ClientError> {
        let code = add_prefix(code);
        let frame = TradeMsg::request(self.next_msg_id(), &code, start, count)?;
        let response = self.send_frame(frame)?;
        
        // 获取当天日期
        let now = std::time::SystemTime::now();
        let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let days = duration.as_secs() / 86400;
        let year = 1970 + (days / 365) as u32; // 简化计算
        let date = format!("{:04}0101", year); // 简化，实际应该用真实日期
        
        let cache = TradeCache { date, code: code.clone() };
        let trades = TradeMsg::decode_response(response.data(), &cache)?;
        Ok(trades)
    }

    /// 获取所有分时交易详情（从0开始）
    pub fn get_trade_all(&self, code: &str) -> Result<TradeResponse, ClientError> {
        self.get_trade_all_from(code, 0)
    }

    /// 获取所有分时交易详情（从指定位置开始）
    pub fn get_trade_all_from(&self, code: &str, from_start: u16) -> Result<TradeResponse, ClientError> {
        let mut all_trades = TradeResponse { count: 0, list: Vec::new() };
        let batch_size = 1800u16;
        let mut start = from_start;

        loop {
            let resp = self.get_trade(code, start, batch_size)?;
            all_trades.count += resp.count;
            // 新数据在前
            let mut new_list = resp.list;
            new_list.append(&mut all_trades.list);
            all_trades.list = new_list;
            
            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_trades)
    }

    /// 获取历史分时交易（单次最多2000条）
    /// date格式：YYYYMMDD
    pub fn get_history_trade(&self, date: &str, code: &str, start: u16, count: u16) -> Result<TradeResponse, ClientError> {
        let code = add_prefix(code);
        let frame = HistoryTradeMsg::request(self.next_msg_id(), date, &code, start, count)?;
        let response = self.send_frame(frame)?;
        let cache = TradeCache { date: date.to_string(), code: code.clone() };
        let trades = HistoryTradeMsg::decode_response(response.data(), &cache)?;
        Ok(trades)
    }

    /// 获取历史某天全部分时交易（从0开始）
    pub fn get_history_trade_day(&self, date: &str, code: &str) -> Result<TradeResponse, ClientError> {
        self.get_history_trade_day_from(date, code, 0)
    }

    /// 获取历史某天全部分时交易（从指定位置开始）
    pub fn get_history_trade_day_from(&self, date: &str, code: &str, from_start: u16) -> Result<TradeResponse, ClientError> {
        let mut all_trades = TradeResponse { count: 0, list: Vec::new() };
        let batch_size = 2000u16;
        let mut start = from_start;

        loop {
            let resp = self.get_history_trade(date, code, start, batch_size)?;
            all_trades.count += resp.count;
            let mut new_list = resp.list;
            new_list.append(&mut all_trades.list);
            all_trades.list = new_list;
            
            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_trades)
    }

    // ==================== 集合竞价 ====================

    /// 获取集合竞价数据
    pub fn get_call_auction(&self, code: &str) -> Result<CallAuctionResponse, ClientError> {
        let code = add_prefix(code);
        let frame = CallAuctionMsg::request(self.next_msg_id(), &code)?;
        let response = self.send_frame(frame)?;
        let auction = CallAuctionMsg::decode_response(response.data())?;
        Ok(auction)
    }

    // ==================== 股本变迁/除权除息 ====================

    /// 获取股本变迁/除权除息数据
    pub fn get_gbbq(&self, code: &str) -> Result<GbbqResponse, ClientError> {
        let code = add_prefix(code);
        let frame = GbbqMsg::request(self.next_msg_id(), &code)?;
        let response = self.send_frame(frame)?;
        let gbbq = GbbqMsg::decode_response(response.data())?;
        Ok(gbbq)
    }

    // ==================== 辅助方法 ====================

    /// 获取下一个消息ID
    fn next_msg_id(&self) -> u32 {
        let mut id = self.msg_id.lock().unwrap();
        *id += 1;
        *id
    }

    /// 设置超时时间
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // 关闭连接
        if let Ok(stream) = self.stream.lock() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}
