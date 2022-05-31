use crate::{
    models::{get_signals, ShortStockInfo, Signal},
    others::{create_csv_file, get_list},
    spreads::OptionSpread,
};
use models::{OptionChain, ShortStockData};
use requests::get_auth;
mod credentials;
mod models;
mod others;
mod requests;
mod single_options;
mod spreads;
mod strategies;
mod trades;
pub const OPTION_COMMISSION: f64 = 2.0;
pub const MARGIN_LOAN_RATE: f64 = 1.6;
pub const STOCK_COMMISSION: f64 = 0.55;
pub const SHORT_STOCK_DATA_FP: &str = "ftp3.interactivebrokers.com";
pub const SHORT_FEE_MARGIN_SAFETY: f64 = 1.2;
pub const STRADDLE_LENGTH: i64 = 90;
pub const MAX_BOX_DTE: i64 = 60;
pub const MAX_SHORT_BOX_SHORT_FEE: f64 = 10.0;
pub const TRADES_TO_INCLUDE: &str = "10000";
pub const MONSTER_SIZE: f64 = 10000000.0;
pub const DESCRIPTIONS_FILEPATH: &str = "ConditionDescriptions.csv";
pub const LIST_LOCATION: &str = "new-list.csv";
pub const AMOUNT_IN_ACCOUNT: f64 = 25000.0;
#[tokio::main]
async fn main() {
    let short_fees = ShortStockInfo::get().await;
    let start = tokio::time::Instant::now();
    if let Ok(symbol_list) = get_list(LIST_LOCATION) {
        let mut all_option_chains: Vec<OptionChain> = Vec::new();
        let mut n = 1;
        println!("Starting up...");
        for symbol in &symbol_list {
            let start_time = tokio::time::Instant::now();
            let short_data = short_fees.data.iter().find(|item| item.symbol == *symbol);
            if let Some(chain) = get_chain_for_one_symbol(symbol, short_data).await {
                all_option_chains.push(chain);
            }
            let one_thousand: u64 = 1000;
            if start_time.elapsed().as_millis() < 1000 {
                let time_to_wait = one_thousand - start_time.elapsed().as_millis() as u64;
                tokio::time::sleep(tokio::time::Duration::from_millis(time_to_wait)).await
            }
            println!(
                "{} - {}/{}, took {} secs",
                symbol,
                n,
                symbol_list.len(),
                start_time.elapsed().as_secs_f64()
            );
            n += 1;
        }
        all_option_chains.sort_unstable_by_key(|chain| -chain.bias);
        let signals: Vec<Signal> = get_signals(&all_option_chains, 2500.0, 25600.0);
        create_csv_file(&signals, "Trade-Signals");
        create_csv_file(&all_option_chains, "ALL-ChainData");
        let mut all_spreads: Vec<OptionSpread> = Vec::new();
        for mut chain in all_option_chains {
            all_spreads.append(&mut chain.spreads);
        }
        all_spreads.sort_unstable_by_key(|spread| -spread.net_value.abs() as i64);
        create_csv_file(
            &all_spreads[0..std::cmp::min(10000, all_spreads.len() - 1)],
            "ALL-Trades",
        );
        create_csv_file(&short_fees.data, "ALL-ShortFee");
    } else {
        println!("Symbol List file not found.")
    }
    println!("Completed in {} seconds", start.elapsed().as_secs())
}

pub async fn get_chain_for_one_symbol(
    symbol: &str,
    short_data: Option<&ShortStockData>,
) -> Option<OptionChain> {
    if let Ok(token) = get_auth().await {
        if let Some(option_chain) = OptionChain::get(symbol, &token, short_data).await {
            Some(option_chain)
        } else {
            None
        }
    } else {
        None
    }
}
