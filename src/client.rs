//! TDX 客户端实现（异步）

use crate::protocol::*;
use chrono::{FixedOffset, Utc};
use log::debug;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time;

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
    #[error("不支持的市场: {0}")]
    UnsupportedMarket(String),
    #[error("其他错误: {0}")]
    Other(String),
}

/// TDX 客户端（异步）
pub struct Client {
    stream: Arc<Mutex<TcpStream>>,
    msg_id: AtomicU32,
    timeout: Duration,
}

impl Client {
    /// 连接到指定地址
    pub async fn connect(addr: &str) -> Result<Self, ClientError> {
        let addr = if addr.contains(':') {
            addr.to_string()
        } else {
            format!("{}:7709", addr)
        };

        let stream = TcpStream::connect(&addr).await?;
        stream.set_nodelay(true)?;

        let client = Self {
            stream: Arc::new(Mutex::new(stream)),
            msg_id: AtomicU32::new(0),
            timeout: Duration::from_secs(10),
        };

        client.send_connect().await?;
        Ok(client)
    }

    /// 发送连接请求并读取响应
    async fn send_connect(&self) -> Result<(), ClientError> {
        let frame = Connect::request(1);
        let data = frame.encode();
        let mut stream = self.stream.lock().await;
        self.write_all_locked(&mut stream, &data).await?;
        let _response = self.read_response_locked(&mut stream).await?;
        Ok(())
    }

    async fn write_all_locked(
        &self,
        stream: &mut TcpStream,
        data: &[u8],
    ) -> Result<(), ClientError> {
        debug!("发送请求帧 ({} 字节): {:02X?}", data.len(), data);

        stream.write_all(data).await?;
        stream.flush().await?;
        Ok(())
    }

    async fn read_response_locked(
        &self,
        stream: &mut TcpStream,
    ) -> Result<ResponseFrame, ClientError> {
        let timeout = self.timeout;
        let fut = async {
            let mut header = [0u8; 16];
            stream.read_exact(&mut header).await?;

            // 前缀是大端序：B1CB7400
            let prefix = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
            if prefix != PREFIX_RESP {
                return Err(ClientError::Protocol(FrameError::InvalidPrefix));
            }

            let msg_type_val = bytes_to_u16_le(&header[10..12]);
            let zip_length = bytes_to_u16_le(&header[12..14]);
            let length = bytes_to_u16_le(&header[14..16]);

            let msg_type = MessageType::from_u16(msg_type_val).ok_or_else(|| {
                ClientError::Protocol(FrameError::UnknownMessageType(msg_type_val))
            })?;

            let mut compressed_data = vec![0u8; zip_length as usize];
            stream.read_exact(&mut compressed_data).await?;

            debug!(
                "接收响应: 类型={:?}, 压缩长度={}, 长度={}",
                msg_type, zip_length, length
            );

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

            response.decompress()?;
            Ok(response)
        };

        match time::timeout(timeout, fut).await {
            Ok(res) => res,
            Err(_) => Err(ClientError::Timeout),
        }
    }

    /// 发送帧并等待响应
    pub async fn send_frame(&self, frame: RequestFrame) -> Result<ResponseFrame, ClientError> {
        let msg_id = self.next_msg_id();

        let mut frame = frame;
        frame.msg_id = msg_id;

        let data = frame.encode();
        let mut stream = self.stream.lock().await;

        self.write_all_locked(&mut stream, &data).await?;
        let response = self.read_response_locked(&mut stream).await?;

        if response.msg_id != msg_id {
            return Err(ClientError::Other(format!(
                "消息ID不匹配: 期望 {}, 得到 {}",
                msg_id, response.msg_id
            )));
        }

        Ok(response)
    }

    /// 获取股票数量
    pub async fn get_count(&self, exchange: Exchange) -> Result<u16, ClientError> {
        let frame = Count::request(self.next_msg_id(), exchange);
        let response = self.send_frame(frame).await?;
        let count = Count::decode_response(response.data())?;
        Ok(count)
    }

    /// 获取股票代码列表（单次最多1000条）
    pub async fn get_code(
        &self,
        exchange: Exchange,
        start: u16,
    ) -> Result<CodeResponse, ClientError> {
        let frame = Code::request(self.next_msg_id(), exchange, start);
        let response = self.send_frame(frame).await?;
        let codes = Code::decode_response(response.data())?;
        Ok(codes)
    }

    /// 获取所有股票代码（从0开始）
    pub async fn get_code_all(&self, exchange: Exchange) -> Result<CodeResponse, ClientError> {
        self.get_code_all_from(exchange, 0).await
    }

    /// 获取所有股票代码（从指定位置开始）
    pub async fn get_code_all_from(
        &self,
        exchange: Exchange,
        from_start: u16,
    ) -> Result<CodeResponse, ClientError> {
        let mut all_codes = CodeResponse {
            count: 0,
            codes: Vec::new(),
        };
        let batch_size = 1000u16;
        let mut start = from_start;

        loop {
            let resp = self.get_code(exchange, start).await?;
            all_codes.count += resp.count;
            all_codes.codes.extend(resp.codes);

            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_codes)
    }

    /// 根据交易所与类型筛选代码
    async fn filter_market_codes(
        &self,
        exchange: Exchange,
        predicate: fn(&str) -> bool,
    ) -> Result<Vec<StockCode>, ClientError> {
        let resp = self.get_code_all(exchange).await?;
        Ok(resp
            .codes
            .into_iter()
            .filter(|c| predicate(&c.code))
            .collect())
    }

    /// 获取指定市场的股票代码
    pub async fn get_market_stocks(
        &self,
        exchange: Exchange,
    ) -> Result<Vec<StockCode>, ClientError> {
        self.filter_market_codes(exchange, is_stock).await
    }

    /// 获取指定市场的ETF代码
    pub async fn get_market_etfs(&self, exchange: Exchange) -> Result<Vec<StockCode>, ClientError> {
        self.filter_market_codes(exchange, is_etf).await
    }

    /// 获取指定市场的指数代码
    pub async fn get_market_indexes(
        &self,
        exchange: Exchange,
    ) -> Result<Vec<StockCode>, ClientError> {
        self.filter_market_codes(exchange, is_index).await
    }

    /// 获取深圳股票
    pub async fn get_sz_stocks(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_stocks(Exchange::SZ).await
    }

    /// 获取上海股票
    pub async fn get_sh_stocks(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_stocks(Exchange::SH).await
    }

    /// 获取北京股票
    ///
    /// 注意：某些通达信服务器可能不支持北京交易所数据
    pub async fn get_bj_stocks(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_stocks(Exchange::BJ)
            .await
            .map_err(|e| match e {
                ClientError::Io(_) => ClientError::UnsupportedMarket(
                    "当前服务器可能不支持北京交易所数据查询，请尝试更换服务器或使用其他市场"
                        .to_string(),
                ),
                _ => e,
            })
    }

    /// 获取全部市场股票
    ///
    /// 注意：如果某个市场不支持（如北京交易所），会跳过该市场继续查询
    pub async fn get_all_stocks(&self) -> Result<Vec<StockCode>, ClientError> {
        let mut all = Vec::new();
        for ex in [Exchange::SZ, Exchange::SH, Exchange::BJ] {
            match self.get_market_stocks(ex).await {
                Ok(stocks) => all.extend(stocks),
                Err(ClientError::UnsupportedMarket(_)) => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(all)
    }

    /// 获取深圳ETF
    pub async fn get_sz_etfs(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_etfs(Exchange::SZ).await
    }

    /// 获取上海ETF
    pub async fn get_sh_etfs(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_etfs(Exchange::SH).await
    }

    /// 获取北京ETF
    ///
    /// 注意：某些通达信服务器可能不支持北京交易所数据
    pub async fn get_bj_etfs(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_etfs(Exchange::BJ)
            .await
            .map_err(|e| match e {
                ClientError::Io(_) => ClientError::UnsupportedMarket(
                    "当前服务器可能不支持北京交易所数据查询，请尝试更换服务器或使用其他市场"
                        .to_string(),
                ),
                _ => e,
            })
    }

    /// 获取全部市场ETF
    ///
    /// 注意：如果某个市场不支持（如北京交易所），会跳过该市场继续查询
    pub async fn get_all_etfs(&self) -> Result<Vec<StockCode>, ClientError> {
        let mut all = Vec::new();
        for ex in [Exchange::SZ, Exchange::SH, Exchange::BJ] {
            match self.get_market_etfs(ex).await {
                Ok(etfs) => all.extend(etfs),
                Err(ClientError::UnsupportedMarket(_)) => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(all)
    }

    /// 获取深圳指数
    pub async fn get_sz_indexes(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_indexes(Exchange::SZ).await
    }

    /// 获取上海指数
    pub async fn get_sh_indexes(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_indexes(Exchange::SH).await
    }

    /// 获取北京指数
    ///
    /// 注意：某些通达信服务器可能不支持北京交易所数据
    pub async fn get_bj_indexes(&self) -> Result<Vec<StockCode>, ClientError> {
        self.get_market_indexes(Exchange::BJ)
            .await
            .map_err(|e| match e {
                ClientError::Io(_) => ClientError::UnsupportedMarket(
                    "当前服务器可能不支持北京交易所数据查询，请尝试更换服务器或使用其他市场"
                        .to_string(),
                ),
                _ => e,
            })
    }

    /// 获取全部市场指数
    ///
    /// 注意：如果某个市场不支持（如北京交易所），会跳过该市场继续查询
    pub async fn get_all_indexes(&self) -> Result<Vec<StockCode>, ClientError> {
        let mut all = Vec::new();
        for ex in [Exchange::SZ, Exchange::SH, Exchange::BJ] {
            match self.get_market_indexes(ex).await {
                Ok(indexes) => all.extend(indexes),
                Err(ClientError::UnsupportedMarket(_)) => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(all)
    }

    /// 获取行情信息（五档报价）
    pub async fn get_quote(&self, codes: &[String]) -> Result<Vec<QuoteInfo>, ClientError> {
        let frame = Quote::request(self.next_msg_id(), codes)?;
        let response = self.send_frame(frame).await?;
        let quotes = Quote::decode_response(response.data())?;
        Ok(quotes)
    }

    /// 发送心跳
    pub async fn send_heartbeat(&self) -> Result<(), ClientError> {
        let frame = Heartbeat::request(self.next_msg_id());
        let _response = self.send_frame(frame).await?;
        Ok(())
    }

    // ==================== K线数据 ====================

    /// 获取K线数据（单次最多800条）
    pub async fn get_kline(
        &self,
        kline_type: KlineType,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        let code = add_prefix(code);
        let frame = KlineMsg::request(self.next_msg_id(), kline_type, &code, start, count)?;
        let response = self.send_frame(frame).await?;
        let cache = KlineCache {
            kline_type: kline_type as u8,
            is_index: is_index(&code),
        };
        let klines = KlineMsg::decode_response(response.data(), cache)?;
        Ok(klines)
    }

    /// 获取所有K线数据（从0开始，通过多次请求拼接）
    pub async fn get_kline_all(
        &self,
        kline_type: KlineType,
        code: &str,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(kline_type, code, 0).await
    }

    /// 获取所有K线数据（从指定位置开始，通过多次请求拼接）
    pub async fn get_kline_all_from(
        &self,
        kline_type: KlineType,
        code: &str,
        from_start: u16,
    ) -> Result<KlineResponse, ClientError> {
        let mut all_klines = KlineResponse {
            count: 0,
            list: Vec::new(),
        };
        let batch_size = 800u16;
        let mut start = from_start;

        loop {
            let resp = self.get_kline(kline_type, code, start, batch_size).await?;
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

    /// 获取所有K线数据（支持自定义过滤）
    ///
    /// util_fn: 过滤函数，返回 true 表示保留，返回 false 表示停止后续查询（break）
    pub async fn get_kline_all_util<F>(
        &self,
        kline_type: KlineType,
        code: &str,
        util_fn: F,
    ) -> Result<KlineResponse, ClientError>
    where
        F: Fn(&Kline) -> bool,
    {
        let mut all_klines = KlineResponse {
            count: 0,
            list: Vec::new(),
        };
        let batch_size = 800u16;
        let mut start = 0;

        'outer: loop {
            let mut resp = self.get_kline(kline_type, code, start, batch_size).await?;
            let len = resp.list.len();

            // 扫描当前批次数据（从新到旧，即倒序）
            // 如果遇到不满足条件的，则该点之前（更旧）的数据也认为不满足（根据时间连续性假设）
            let mut fully_match = true;
            let mut cut_index = 0;

            for (i, k) in resp.list.iter().enumerate().rev() {
                if !util_fn(k) {
                    cut_index = i + 1;
                    fully_match = false;
                    break;
                }
            }

            if fully_match {
                // 全部满足，将整个列表加到结果的前面
                let mut new_list = resp.list;
                new_list.append(&mut all_klines.list);
                all_klines.list = new_list;
                all_klines.count += len as u16;
            } else {
                // 部分满足，截取满足的部分
                // split_offAt cut_index, valid parts are [cut_index..len]
                let mut valid_part = resp.list.split_off(cut_index);
                all_klines.count += valid_part.len() as u16;
                valid_part.append(&mut all_klines.list);
                all_klines.list = valid_part;

                // 既然已经遇到不满足的了，更旧的数据肯定也不满足，退出循环
                break 'outer;
            }

            if resp.count < batch_size {
                break;
            }
            start += batch_size;
        }

        Ok(all_klines)
    }

    /// 获取1分钟K线数据
    pub async fn get_kline_minute(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute, code, start, count).await
    }

    /// 获取5分钟K线数据
    pub async fn get_kline_5minute(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute5, code, start, count).await
    }

    /// 获取15分钟K线数据
    pub async fn get_kline_15minute(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute15, code, start, count)
            .await
    }

    /// 获取30分钟K线数据
    pub async fn get_kline_30minute(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute30, code, start, count)
            .await
    }

    /// 获取60分钟K线数据
    pub async fn get_kline_60minute(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Minute60, code, start, count)
            .await
    }

    /// 获取日K线数据
    pub async fn get_kline_day(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Day, code, start, count).await
    }

    /// 获取所有日K线数据
    pub async fn get_kline_day_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all(KlineType::Day, code).await
    }

    /// 获取所有日K线数据（从指定位置开始）
    pub async fn get_kline_day_all_from(
        &self,
        code: &str,
        from_start: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(KlineType::Day, code, from_start)
            .await
    }

    /// 获取周K线数据
    pub async fn get_kline_week(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Week, code, start, count).await
    }

    /// 获取所有周K线数据
    pub async fn get_kline_week_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all(KlineType::Week, code).await
    }

    /// 获取所有周K线数据（从指定位置开始）
    pub async fn get_kline_week_all_from(
        &self,
        code: &str,
        from_start: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(KlineType::Week, code, from_start)
            .await
    }

    /// 获取月K线数据
    pub async fn get_kline_month(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Month, code, start, count).await
    }

    /// 获取所有月K线数据
    pub async fn get_kline_month_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_kline_all(KlineType::Month, code).await
    }

    /// 获取所有月K线数据（从指定位置开始）
    pub async fn get_kline_month_all_from(
        &self,
        code: &str,
        from_start: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline_all_from(KlineType::Month, code, from_start)
            .await
    }

    /// 获取季K线数据
    pub async fn get_kline_quarter(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Quarter, code, start, count).await
    }

    /// 获取年K线数据
    pub async fn get_kline_year(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_kline(KlineType::Year, code, start, count).await
    }

    // ==================== 指数K线数据 ====================

    /// 获取指数K线数据
    pub async fn get_index(
        &self,
        kline_type: KlineType,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        let code = add_prefix(code);
        let frame = KlineMsg::request(self.next_msg_id(), kline_type, &code, start, count)?;
        let response = self.send_frame(frame).await?;
        let cache = KlineCache {
            kline_type: kline_type as u8,
            is_index: true,
        };
        let klines = KlineMsg::decode_response(response.data(), cache)?;
        Ok(klines)
    }

    /// 获取所有指数K线数据（从0开始）
    pub async fn get_index_all(
        &self,
        kline_type: KlineType,
        code: &str,
    ) -> Result<KlineResponse, ClientError> {
        self.get_index_all_from(kline_type, code, 0).await
    }

    /// 获取所有指数K线数据（从指定位置开始）
    pub async fn get_index_all_from(
        &self,
        kline_type: KlineType,
        code: &str,
        from_start: u16,
    ) -> Result<KlineResponse, ClientError> {
        let mut all_klines = KlineResponse {
            count: 0,
            list: Vec::new(),
        };
        let batch_size = 800u16;
        let mut start = from_start;

        loop {
            let resp = self.get_index(kline_type, code, start, batch_size).await?;
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
    pub async fn get_index_day(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_index(KlineType::Day, code, start, count).await
    }

    /// 获取所有指数日K线数据
    pub async fn get_index_day_all(&self, code: &str) -> Result<KlineResponse, ClientError> {
        self.get_index_all(KlineType::Day, code).await
    }

    /// 获取所有指数日K线数据（从指定位置开始）
    pub async fn get_index_day_all_from(
        &self,
        code: &str,
        from_start: u16,
    ) -> Result<KlineResponse, ClientError> {
        self.get_index_all_from(KlineType::Day, code, from_start)
            .await
    }

    // ==================== 分时数据 ====================

    /// 获取分时数据（使用历史分时接口，与 Go 版本一致）
    pub async fn get_minute(&self, code: &str) -> Result<MinuteResponse, ClientError> {
        let today = Self::today_str();
        self.get_history_minute(&today, code).await
    }

    /// 获取当前日期字符串（YYYYMMDD格式，北京时间）
    fn today_str() -> String {
        let beijing_offset = FixedOffset::east_opt(8 * 3600).unwrap();
        Utc::now()
            .with_timezone(&beijing_offset)
            .format("%Y%m%d")
            .to_string()
    }

    /// 获取历史分时数据
    /// date格式：YYYYMMDD
    pub async fn get_history_minute(
        &self,
        date: &str,
        code: &str,
    ) -> Result<MinuteResponse, ClientError> {
        let code = add_prefix(code);
        let frame = HistoryMinuteMsg::request(self.next_msg_id(), date, &code)?;
        let response = self.send_frame(frame).await?;
        let minute = HistoryMinuteMsg::decode_response(response.data(), date)?;
        Ok(minute)
    }

    // ==================== 交易数据 ====================

    /// 获取分时交易详情（单次最多1800条）
    pub async fn get_trade(
        &self,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<TradeResponse, ClientError> {
        let code = add_prefix(code);
        let frame = TradeMsg::request(self.next_msg_id(), &code, start, count)?;
        let response = self.send_frame(frame).await?;

        // 获取当天日期
        let beijing_offset = FixedOffset::east_opt(8 * 3600).unwrap();
        let now = Utc::now().with_timezone(&beijing_offset);
        let date = now.format("%Y%m%d").to_string();

        let cache = TradeCache {
            date,
            code: code.clone(),
        };
        let trades = TradeMsg::decode_response(response.data(), &cache)?;
        Ok(trades)
    }

    /// 获取所有分时交易详情（从0开始）
    pub async fn get_trade_all(&self, code: &str) -> Result<TradeResponse, ClientError> {
        self.get_trade_all_from(code, 0).await
    }

    /// 获取所有分时交易详情（从指定位置开始）
    pub async fn get_trade_all_from(
        &self,
        code: &str,
        from_start: u16,
    ) -> Result<TradeResponse, ClientError> {
        let mut all_trades = TradeResponse {
            count: 0,
            list: Vec::new(),
        };
        let batch_size = 1800u16;
        let mut start = from_start;

        loop {
            let resp = self.get_trade(code, start, batch_size).await?;
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
    pub async fn get_history_trade(
        &self,
        date: &str,
        code: &str,
        start: u16,
        count: u16,
    ) -> Result<TradeResponse, ClientError> {
        let code = add_prefix(code);
        let frame = HistoryTradeMsg::request(self.next_msg_id(), date, &code, start, count)?;
        let response = self.send_frame(frame).await?;
        let cache = TradeCache {
            date: date.to_string(),
            code: code.clone(),
        };
        let trades = HistoryTradeMsg::decode_response(response.data(), &cache)?;
        Ok(trades)
    }

    /// 获取历史某天全部分时交易（从0开始）
    pub async fn get_history_trade_day(
        &self,
        date: &str,
        code: &str,
    ) -> Result<TradeResponse, ClientError> {
        self.get_history_trade_day_from(date, code, 0).await
    }

    /// 获取历史某天全部分时交易（从指定位置开始）
    pub async fn get_history_trade_day_from(
        &self,
        date: &str,
        code: &str,
        from_start: u16,
    ) -> Result<TradeResponse, ClientError> {
        let mut all_trades = TradeResponse {
            count: 0,
            list: Vec::new(),
        };
        let batch_size = 2000u16;
        let mut start = from_start;

        loop {
            let resp = self
                .get_history_trade(date, code, start, batch_size)
                .await?;
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
    pub async fn get_call_auction(&self, code: &str) -> Result<CallAuctionResponse, ClientError> {
        let code = add_prefix(code);
        let frame = CallAuctionMsg::request(self.next_msg_id(), &code)?;
        let response = self.send_frame(frame).await?;
        let auction = CallAuctionMsg::decode_response(response.data())?;
        Ok(auction)
    }

    // ==================== 股本变迁/除权除息 ====================

    /// 获取股本变迁/除权除息数据
    pub async fn get_gbbq(&self, code: &str) -> Result<GbbqResponse, ClientError> {
        let code = add_prefix(code);
        let frame = GbbqMsg::request(self.next_msg_id(), &code)?;
        let response = self.send_frame(frame).await?;
        let gbbq = GbbqMsg::decode_response(response.data())?;
        Ok(gbbq)
    }

    /// 获取下一个消息ID
    fn next_msg_id(&self) -> u32 {
        self.msg_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 设置超时时间
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }
}

impl Drop for Client {
    fn drop(&mut self) {}
}
