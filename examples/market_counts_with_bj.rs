//! 打印各市场、各类型的代码数量示例（包含改进的北京交易所错误处理，异步版）
//!
//! 此示例展示了如何优雅地处理北京交易所可能不被支持的情况。
//! 当遇到北交所不支持的错误时，会显示友好的提示信息而不是原始的IO错误。

use tdx_rust::*;

#[derive(Default)]
struct CountSummary {
    stocks: usize,
    etfs: usize,
    indexes: usize,
}

impl CountSummary {
    fn add(&mut self, other: &CountSummary) {
        self.stocks += other.stocks;
        self.etfs += other.etfs;
        self.indexes += other.indexes;
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), ClientError> {
    const ADDR: &str = "124.71.187.122";

    let markets = [
        ("上海", Exchange::SH),
        ("深圳", Exchange::SZ),
        ("北京", Exchange::BJ),
    ];

    let mut totals = CountSummary::default();
    let mut unsupported_markets = Vec::new();

    println!("正在查询市场数据...\n");

    for (name, exchange) in markets {
        println!("=== {} ({:?}) ===", name, exchange);

        let client = match dial(ADDR).await {
            Ok(c) => c,
            Err(err) => {
                println!("连接失败: {}", err);
                println!();
                continue;
            }
        };

        let mut market_summary = CountSummary::default();
        let mut has_error = false;

        match client.get_market_stocks(exchange).await {
            Ok(stocks) => {
                market_summary.stocks = stocks.len();
                println!("股票: {}", stocks.len());
            }
            Err(ClientError::UnsupportedMarket(msg)) => {
                println!("股票: 不支持");
                if !unsupported_markets.contains(&name) {
                    unsupported_markets.push(name);
                }
                has_error = true;
                if matches!(exchange, Exchange::BJ) {
                    println!("  提示: {}", msg);
                }
            }
            Err(err) => {
                println!("股票: 查询失败 - {}", err);
                has_error = true;
            }
        }

        match client.get_market_etfs(exchange).await {
            Ok(etfs) => {
                market_summary.etfs = etfs.len();
                println!("ETF : {}", etfs.len());
            }
            Err(ClientError::UnsupportedMarket(_msg)) => {
                println!("ETF : 不支持");
                if !unsupported_markets.contains(&name) {
                    unsupported_markets.push(name);
                }
                has_error = true;
            }
            Err(err) => {
                println!("ETF : 查询失败 - {}", err);
                has_error = true;
            }
        }

        match client.get_market_indexes(exchange).await {
            Ok(indexes) => {
                market_summary.indexes = indexes.len();
                println!("指数: {}", indexes.len());
            }
            Err(ClientError::UnsupportedMarket(_msg)) => {
                println!("指数: 不支持");
                if !unsupported_markets.contains(&name) {
                    unsupported_markets.push(name);
                }
                has_error = true;
            }
            Err(err) => {
                println!("指数: 查询失败 - {}", err);
                has_error = true;
            }
        }

        if !has_error {
            totals.add(&market_summary);
        }

        println!();
    }

    println!("=== 汇总（成功的市场） ===");
    println!("股票: {}", totals.stocks);
    println!("ETF : {}", totals.etfs);
    println!("指数: {}", totals.indexes);

    if !unsupported_markets.is_empty() {
        println!("\n=== 不支持的市场 ===");
        for market in unsupported_markets {
            println!("- {}", market);
        }
        println!("\n建议:");
        println!("1. 使用 `cargo run --example market_counts_no_bj` 跳过北交所查询");
        println!("2. 或者尝试更换支持北交所的通达信服务器");
        println!("3. 参考 docs/beijing_exchange.md 了解更多信息");
    }

    Ok(())
}
