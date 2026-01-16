//! 打印各市场、各类型的代码数量示例（异步）

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

    let markets = [("上海", Exchange::SH), ("深圳", Exchange::SZ)];

    let mut totals = CountSummary::default();

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

        let stocks = client.get_market_stocks(exchange).await;
        let etfs = client.get_market_etfs(exchange).await;
        let indexes = client.get_market_indexes(exchange).await;

        match (stocks, etfs, indexes) {
            (Ok(stocks), Ok(etfs), Ok(indexes)) => {
                println!("股票: {}", stocks.len());
                println!("ETF : {}", etfs.len());
                println!("指数: {}", indexes.len());

                totals.add(&CountSummary {
                    stocks: stocks.len(),
                    etfs: etfs.len(),
                    indexes: indexes.len(),
                });
            }
            (s, e, i) => {
                println!("获取失败：");
                if let Err(err) = s {
                    println!("  股票: {}", err);
                }
                if let Err(err) = e {
                    println!("  ETF : {}", err);
                }
                if let Err(err) = i {
                    println!("  指数: {}", err);
                }
            }
        }
        println!();
    }

    println!("=== 汇总（成功的市场） ===");
    println!("股票: {}", totals.stocks);
    println!("ETF : {}", totals.etfs);
    println!("指数: {}", totals.indexes);

    Ok(())
}
