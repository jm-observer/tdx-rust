//! 协议数据类型定义

use crate::protocol::constants::Exchange;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 格式化 SystemTime 为可读字符串
fn format_time(time: &SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = duration.as_secs();
    
    // 简化的日期时间计算
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    
    // 计算年月日（简化算法）
    let mut year = 1970i32;
    let mut remaining_days = days as i32;
    
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    
    let mut month = 1;
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    
    let day = remaining_days + 1;
    
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// 价格类型，单位为厘（1元 = 1000厘）
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Price(pub i64);

impl Price {
    pub fn from_yuan(yuan: f64) -> Self {
        Price((yuan * 1000.0) as i64)
    }

    pub fn to_yuan(self) -> f64 {
        self.0 as f64 / 1000.0
    }

    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl fmt::Debug for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}", self.to_yuan())
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}元", self.to_yuan())
    }
}

/// 价格档位（5档买卖盘）
#[derive(Clone, Copy)]
pub struct PriceLevel {
    pub buy: bool,      // 是否为买盘
    pub price: Price,   // 价格
    pub number: i32,    // 数量（手）
}

impl fmt::Debug for PriceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side = if self.buy { "买" } else { "卖" };
        write!(f, "{}:{:.2}x{}", side, self.price.to_yuan(), self.number)
    }
}

/// 5档价格档位
pub type PriceLevels = [PriceLevel; 5];

/// K线数据
#[derive(Clone)]
pub struct K {
    pub last: Price,   // 昨天收盘价
    pub open: Price,   // 今日开盘价
    pub high: Price,   // 今日最高价
    pub low: Price,    // 今日最低价
    pub close: Price,  // 今日收盘价
}

impl fmt::Debug for K {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "K{{昨收:{:.2} 开:{:.2} 高:{:.2} 低:{:.2} 收:{:.2}}}",
            self.last.to_yuan(), self.open.to_yuan(), self.high.to_yuan(),
            self.low.to_yuan(), self.close.to_yuan())
    }
}

/// K线数据项
#[derive(Clone)]
pub struct Kline {
    pub last: Price,        // 昨日收盘价
    pub open: Price,        // 开盘价
    pub high: Price,        // 最高价
    pub low: Price,         // 最低价
    pub close: Price,       // 收盘价
    pub order: i32,         // 成交单数
    pub volume: i64,        // 成交量
    pub amount: Price,      // 成交额
    pub time: SystemTime,   // 时间
    pub up_count: i32,      // 上涨数量（指数有效）
    pub down_count: i32,    // 下跌数量（指数有效）
}

impl Kline {
    /// 格式化时间
    pub fn time_str(&self) -> String {
        format_time(&self.time)
    }
}

impl fmt::Debug for Kline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} 昨收:{:.2} 开:{:.2} 高:{:.2} 低:{:.2} 收:{:.2} 量:{} 额:{:.0} 单数:{}",
            format_time(&self.time),
            self.last.to_yuan(),
            self.open.to_yuan(), self.high.to_yuan(), self.low.to_yuan(),
            self.close.to_yuan(), self.volume, self.amount.to_yuan(), self.order)?;
        
        // 如果是指数，显示上涨/下跌数量
        if self.up_count > 0 || self.down_count > 0 {
            write!(f, " 涨:{}/跌:{}", self.up_count, self.down_count)?;
        }
        
        Ok(())
    }
}

/// 分时数据项
#[derive(Clone)]
pub struct PriceNumber {
    pub time: String,   // 时间字符串（HH:MM格式）
    pub price: Price,   // 价格
    pub number: i32,    // 成交量（手）
}

impl fmt::Debug for PriceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:.2} {}手", self.time, self.price.to_yuan(), self.number)
    }
}

/// 分时成交数据项
#[derive(Clone)]
pub struct Trade {
    pub time: SystemTime,  // 时间
    pub price: Price,      // 价格
    pub volume: i32,       // 成交量（手）
    pub status: TradeStatus, // 状态
    pub number: i32,       // 单数（历史数据无效）
}

impl fmt::Debug for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:.2} {}手 {:?} 单数:{}",
            format_time(&self.time), self.price.to_yuan(), self.volume, self.status, self.number)
    }
}

/// 成交状态
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TradeStatus {
    Buy = 0,      // 买入
    Sell = 1,     // 卖出
    Neutral = 2,  // 中性/汇总
}

impl fmt::Debug for TradeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeStatus::Buy => write!(f, "买"),
            TradeStatus::Sell => write!(f, "卖"),
            TradeStatus::Neutral => write!(f, "中"),
        }
    }
}

/// 股票代码信息
#[derive(Clone)]
pub struct StockCode {
    pub name: String,       // 股票名称
    pub code: String,       // 股票代码
    pub multiple: u16,      // 倍数，基本是100
    pub decimal: i8,        // 小数点，基本是2
    pub last_price: f64,    // 昨收价格（单位元，对个股无效，对指数有效）
}

impl fmt::Debug for StockCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} 倍数:{} 小数:{}", self.code, self.name, self.multiple, self.decimal)?;
        if self.last_price > 0.0 {
            write!(f, " 昨收:{:.2}", self.last_price)?;
        }
        Ok(())
    }
}

/// 行情信息
#[derive(Clone)]
pub struct QuoteInfo {
    pub exchange: Exchange,        // 市场
    pub code: String,              // 股票代码
    pub active1: u16,              // 活跃度
    pub k: K,                      // K线
    pub server_time: String,        // 服务器时间
    pub total_hand: i32,            // 总手
    pub intuition: i32,             // 现量
    pub amount: f64,                // 金额
    pub inside_dish: i32,           // 内盘
    pub outer_disc: i32,            // 外盘
    pub buy_level: PriceLevels,    // 5档买盘
    pub sell_level: PriceLevels,   // 5档卖盘
    pub rate: f64,                  // 涨速
    pub active2: u16,               // 活跃度
}

impl fmt::Debug for QuoteInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let change = self.k.close.to_yuan() - self.k.last.to_yuan();
        let change_pct = if self.k.last.0 != 0 {
            change / self.k.last.to_yuan() * 100.0
        } else {
            0.0
        };
        
        // 基本信息
        write!(f, "{}{} 现价:{:.2} 涨跌:{:+.2}({:+.2}%) 量:{}手 额:{:.0}万",
            self.exchange.as_str(), self.code,
            self.k.close.to_yuan(), change, change_pct,
            self.total_hand, self.amount / 10000.0)?;
        
        // K线数据
        write!(f, " 开:{:.2} 高:{:.2} 低:{:.2} 昨收:{:.2}",
            self.k.open.to_yuan(), self.k.high.to_yuan(),
            self.k.low.to_yuan(), self.k.last.to_yuan())?;
        
        // 交易信息
        write!(f, " 现量:{} 内盘:{} 外盘:{} 涨速:{:.2}",
            self.intuition, self.inside_dish, self.outer_disc, self.rate)?;
        
        // 活跃度（如果非零）
        if self.active1 > 0 || self.active2 > 0 {
            write!(f, " 活跃度:{}/{}", self.active1, self.active2)?;
        }
        
        // 服务器时间（如果有）
        if !self.server_time.is_empty() {
            write!(f, " 服务器:{}", self.server_time)?;
        }
        
        // 5档买卖盘（简化显示：只显示第一档和第五档）
        let buy1 = &self.buy_level[0];
        let buy5 = &self.buy_level[4];
        let sell1 = &self.sell_level[0];
        let sell5 = &self.sell_level[4];
        
        if buy1.number > 0 || sell1.number > 0 {
            write!(f, " 买1:{:.2}x{} 买5:{:.2}x{} 卖1:{:.2}x{} 卖5:{:.2}x{}",
                buy1.price.to_yuan(), buy1.number,
                buy5.price.to_yuan(), buy5.number,
                sell1.price.to_yuan(), sell1.number,
                sell5.price.to_yuan(), sell5.number)?;
        }
        
        Ok(())
    }
}

/// 集合竞价数据项
#[derive(Clone)]
pub struct CallAuction {
    pub time: SystemTime,   // 时间
    pub price: Price,       // 价格
    pub matched: i64,       // 匹配量（match 是关键字，改用 matched）
    pub unmatched: i64,     // 未匹配量
    pub flag: i8,           // 标志，1表示未匹配量是买单，-1表示未匹配量是卖单
}

impl fmt::Debug for CallAuction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let side = if self.flag > 0 { "买" } else { "卖" };
        write!(f, "{} {:.2} 匹配:{} 未匹配:{}{}", 
            format_time(&self.time), self.price.to_yuan(),
            self.matched, self.unmatched, side)
    }
}

/// 股本变迁/除权除息数据项
#[derive(Clone)]
pub struct Gbbq {
    pub code: String,       // 股票代码（带交易所前缀）
    pub time: SystemTime,   // 时间
    pub category: i32,      // 类别
    pub c1: f64,            // 分红(10股分n元) / 行权价 / 前流通
    pub c2: f64,            // 配股价 / 前总股本
    pub c3: f64,            // 送转股 / 缩股 / 后流通
    pub c4: f64,            // 配股 / 后总股本
}

impl Gbbq {
    /// 获取类别名称
    pub fn category_name(&self) -> &'static str {
        match self.category {
            1 => "除权除息",
            2 => "送配股上市",
            3 => "非流通股上市",
            4 => "未知股本变动",
            5 => "股本变化",
            6 => "增发新股",
            7 => "股份回购",
            8 => "增发新股上市",
            9 => "转配股上市",
            10 => "可转债上市",
            11 => "扩缩股",
            12 => "非流通股缩股",
            13 => "送认购权证",
            14 => "送认沽权证",
            _ => "未知",
        }
    }

    /// 是否为股本变化类型
    pub fn is_equity(&self) -> bool {
        matches!(self.category, 2 | 3 | 5 | 7 | 8 | 9 | 10)
    }

    /// 是否为除权除息类型
    pub fn is_xrxd(&self) -> bool {
        self.category == 1
    }
}

impl Gbbq {
    /// 返回与 Go 版本一致的格式字符串（用于对比调试）
    pub fn to_go_format(&self) -> String {
        format!("&{{{} {} {} {} {} {} {}}}",
            self.code,
            format_time(&self.time),
            self.category,
            self.c1,
            self.c2,
            self.c3,
            self.c4)
    }
}

impl fmt::Debug for Gbbq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 根据类别显示不同的字段含义
        match self.category {
            1 => {
                // 除权除息：分红、配股价、送转股、配股
                write!(f, "{} {} {} 分红:{:.2} 配股价:{:.2} 送转股:{:.2} 配股:{:.2}",
                    format_time(&self.time), self.code, self.category_name(),
                    self.c1, self.c2, self.c3, self.c4)
            }
            11 | 12 => {
                // 扩缩股：缩股
                write!(f, "{} {} {} 缩股:{:.2}",
                    format_time(&self.time), self.code, self.category_name(), self.c3)
            }
            13 | 14 => {
                // 权证：行权价、份数
                write!(f, "{} {} {} 行权价:{:.2} 份数:{:.2}",
                    format_time(&self.time), self.code, self.category_name(), self.c1, self.c3)
            }
            _ => {
                // 其他：前流通、前总股本、后流通、后总股本
                write!(f, "{} {} {} 前流通:{:.0} 前总股本:{:.0} 后流通:{:.0} 后总股本:{:.0}",
                    format_time(&self.time), self.code, self.category_name(),
                    self.c1, self.c2, self.c3, self.c4)
            }
        }
    }
}

/// K线响应数据
#[derive(Clone)]
pub struct KlineResponse {
    pub count: u16,
    pub list: Vec<Kline>,
}

impl fmt::Debug for KlineResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "K线数据({}):", self.count)?;
        for (i, k) in self.list.iter().take(10).enumerate() {
            writeln!(f, "  {:>3}. {:?}", i + 1, k)?;
        }
        if self.list.len() > 10 {
            writeln!(f, "  ... 还有 {} 条", self.list.len() - 10)?;
        }
        Ok(())
    }
}

/// 分时数据响应
#[derive(Clone)]
pub struct MinuteResponse {
    pub count: u16,
    pub list: Vec<PriceNumber>,
}

impl fmt::Debug for MinuteResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "分时数据({}):", self.count)?;
        for (i, m) in self.list.iter().take(10).enumerate() {
            writeln!(f, "  {:>3}. {:?}", i + 1, m)?;
        }
        if self.list.len() > 10 {
            writeln!(f, "  ... 还有 {} 条", self.list.len() - 10)?;
        }
        Ok(())
    }
}

/// 交易数据响应
#[derive(Clone)]
pub struct TradeResponse {
    pub count: u16,
    pub list: Vec<Trade>,
}

impl fmt::Debug for TradeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "交易数据({}):", self.count)?;
        for (i, t) in self.list.iter().take(10).enumerate() {
            writeln!(f, "  {:>3}. {:?}", i + 1, t)?;
        }
        if self.list.len() > 10 {
            writeln!(f, "  ... 还有 {} 条", self.list.len() - 10)?;
        }
        Ok(())
    }
}

/// 集合竞价响应
#[derive(Clone)]
pub struct CallAuctionResponse {
    pub count: u16,
    pub list: Vec<CallAuction>,
}

impl fmt::Debug for CallAuctionResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "集合竞价数据({}):", self.count)?;
        for (i, a) in self.list.iter().take(10).enumerate() {
            writeln!(f, "  {:>3}. {:?}", i + 1, a)?;
        }
        if self.list.len() > 10 {
            writeln!(f, "  ... 还有 {} 条", self.list.len() - 10)?;
        }
        Ok(())
    }
}

/// 股本变迁响应
#[derive(Clone)]
pub struct GbbqResponse {
    pub count: u16,
    pub list: Vec<Gbbq>,
}

impl fmt::Debug for GbbqResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "股本变迁数据({}):", self.count)?;
        for (i, g) in self.list.iter().take(10).enumerate() {
            writeln!(f, "  {:>3}. {:?}", i + 1, g)?;
        }
        if self.list.len() > 10 {
            writeln!(f, "  ... 还有 {} 条", self.list.len() - 10)?;
        }
        Ok(())
    }
}

/// K线缓存信息（用于解码时的上下文）
#[derive(Clone, Copy)]
pub struct KlineCache {
    pub kline_type: u8,     // K线类型
    pub is_index: bool,     // 是否为指数
}

impl fmt::Debug for KlineCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = match self.kline_type {
            0 => "5分钟",
            1 => "15分钟",
            2 => "30分钟",
            3 => "60分钟",
            4 => "日线2",
            5 => "周线",
            6 => "月线",
            7 => "1分钟",
            8 => "1分钟2",
            9 => "日线",
            10 => "季线",
            11 => "年线",
            _ => "未知",
        };
        let kind = if self.is_index { "指数" } else { "股票" };
        write!(f, "{}K线({})", type_name, kind)
    }
}
