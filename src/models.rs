use crate::single_options::OptionData;
use crate::spreads::OptionSpread;
use crate::strategies::remove_decimals;
use crate::trades::Expectation;
use crate::trades::OptionTrade;
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
pub type OptionRW = Arc<std::sync::RwLock<OptionsLock>>;
pub type DividendRW = Arc<std::sync::RwLock<DividendsLock>>;
pub type TradesRW = Arc<std::sync::RwLock<TradesLock>>;

#[derive(Debug, Serialize, Clone)]
pub struct OptionChain {
    pub symbol: String,
    pub symbol_date: String,
    pub date: String,
    #[serde(skip_serializing)]
    pub options: Vec<OptionData>,
    pub underlying_mid: f64,
    pub data_timestamp: String,
    pub ex_div_date: String,
    #[serde(skip_serializing)]
    pub declaration_date: Option<String>,
    #[serde(skip_serializing)]
    pub payment_date: Option<String>,
    #[serde(skip_serializing)]
    pub period: DividendPeriod,
    pub record_date: String,
    #[serde(skip_serializing)]
    pub unadjusted_value: f64,
    #[serde(skip_serializing)]
    pub value: f64,
    pub estimated: bool,
    #[serde(skip_serializing)]
    pub dividend_info: Option<DividendInformation>,
    pub short_fee: f64,
    #[serde(skip_serializing)]
    pub shares_available: String,
    pub dealer_delta: f64,
    pub naive_dealer_delta: f64,
    pub put_call_oi_ratio: f64,
    pub put_call_volume_ratio: f64,
    pub insider_net_transaction: f64,
    pub bias: i64,
    #[serde(skip_serializing)]
    pub spreads: Vec<OptionSpread>,
    pub short_interest_percent: f64,
    pub large_trader_delta: f64,
    pub large_trader_opening_delta: f64,
    pub large_trader_expectation: Expectation,
    pub large_trader_absolute_value: f64,
    pub large_trader_net_value: f64,
    pub large_trader_opening_net_value: f64,
    pub large_trader_opening_absolute_value: f64,
    pub shares_to_trade: i64,
}
impl OptionChain {
    pub fn to_signal(&self, quantity_1: f64, quantity_2: f64) -> Signal {
        Signal {
            symbol: self.symbol.clone(),
            side: if self.large_trader_net_value > 0.0 {
                SignalType::Buy
            } else {
                SignalType::Sell
            },
            quantity_1: remove_decimals(quantity_1 / self.underlying_mid),
            quantity_2: remove_decimals(quantity_2 / self.underlying_mid),
            large_trader_net_value: self.large_trader_net_value,
        }
    }
}
pub fn to_be_calculated_float() -> f64 {
    0.0
}

pub fn to_be_calculated_bool() -> bool {
    false
}
pub fn to_be_calculated_string() -> String {
    String::new()
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerResponse {
    pub implied_underlying_ask: Option<f64>,
    pub implied_underlying_ask_size: Option<i64>,
    pub implied_underlying_bid: Option<f64>,
    pub implied_underlying_bid_size: Option<i64>,
    pub implied_underlying_indicator: Option<String>,
    pub implied_underlying_mid: Option<f64>,
    #[serde(default = "to_be_calculated_float")]
    pub iv30: f64,
    #[serde(default = "to_be_calculated_float")]
    pub iv30_change: f64,
    #[serde(default = "to_be_calculated_float")]
    pub iv30_change_percent: f64,
    pub options: Vec<OptionData>,
    pub seq_no: Option<i64>,
    #[serde(default = "to_be_calculated_string")]
    pub symbol: String,
    pub timestamp: Option<String>,
    pub underlying_ask: Option<f64>,
    pub underlying_ask_size: Option<i64>,
    pub underlying_bid: Option<f64>,
    pub underlying_bid_size: Option<i64>,
    pub underlying_close: Option<f64>,
    pub underlying_high: Option<f64>,
    pub underlying_last_trade_price: Option<f64>,
    pub underlying_last_trade_size: Option<i64>,
    pub underlying_low: Option<f64>,
    pub underlying_mid: Option<f64>,
    pub underlying_open: Option<f64>,
    pub underlying_prev_day_close: Option<f64>,
    pub underlying_volume: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DividendInformation {
    pub currency: Option<String>,
    #[serde(rename = "date")]
    pub ex_div_date: String,
    #[serde(rename = "declarationDate")]
    pub declaration_date: Option<String>,
    #[serde(rename = "paymentDate")]
    pub payment_date: Option<String>,
    pub period: Option<DividendPeriod>,
    #[serde(rename = "recordDate")]
    pub record_date: Option<String>,
    #[serde(rename = "unadjustedValue")]
    pub unadjusted_value: f64,
    pub value: f64,
    #[serde(default = "to_be_calculated_bool")]
    pub estimated: bool,
    #[serde(default = "to_be_calculated_bool")]
    pub poisoned: bool,
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Copy)]
pub enum DividendPeriod {
    Quarterly,
    Annual,
    Monthly,
    Other,
    SemiAnnual,
    None,
    Unknown,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShortStockData {
    #[serde(rename(deserialize = "#SYM"))]
    pub symbol: String,
    #[serde(rename(deserialize = "CUR"))]
    pub currency: String,
    #[serde(rename(deserialize = "NAME"))]
    pub name: String,
    #[serde(rename(deserialize = "CON"))]
    pub con: String,
    #[serde(rename(deserialize = "ISIN"))]
    pub isin: String,
    #[serde(rename(deserialize = "REBATERATE"))]
    pub rebate_rate: String,
    #[serde(rename(deserialize = "FEERATE"))]
    pub fee_rate: String,
    #[serde(rename(deserialize = "AVAILABLE"))]
    pub available: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShortStockInfo {
    pub date: String,
    pub time: String,
    pub data: Vec<ShortStockData>,
}

impl ServerResponse {
    pub fn new() -> Self {
        Self {
            implied_underlying_ask: None,
            implied_underlying_ask_size: None,
            implied_underlying_bid: None,
            implied_underlying_bid_size: None,
            implied_underlying_indicator: None,
            implied_underlying_mid: None,
            iv30: 0.0,
            iv30_change: 0.0,
            iv30_change_percent: 0.0,
            options: Vec::new(),
            seq_no: None,
            symbol: String::new(),
            timestamp: None,
            underlying_ask: None,
            underlying_ask_size: None,
            underlying_bid: None,
            underlying_bid_size: None,
            underlying_close: None,
            underlying_high: None,
            underlying_last_trade_price: None,
            underlying_last_trade_size: None,
            underlying_low: None,
            underlying_mid: None,
            underlying_open: None,
            underlying_prev_day_close: None,
            underlying_volume: None,
        }
    }
}
impl Default for DividendInformation {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug)]
pub struct OptionsLock {
    pub cboe_token: String,
    pub symbol: String,
    pub options: ServerResponse,
    pub insider_net: f64,
}
impl OptionsLock {
    pub fn new(symbol: &str, cboe_token: &str) -> Self {
        Self {
            cboe_token: cboe_token.to_string(),
            symbol: symbol.to_string(),
            options: ServerResponse::new(),
            insider_net: 0.0,
        }
    }
}
#[derive(Debug)]
pub struct DividendsLock {
    pub symbol: String,
    pub dividends: Option<DividendInformation>,
}
impl DividendsLock {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            dividends: None,
        }
    }
}
#[derive(Debug)]
pub struct TradesLock {
    pub symbol: String,
    pub token: String,
    pub trades: Vec<OptionTrade>,
}
impl TradesLock {
    pub fn new(symbol: &str, cboe_token: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            trades: Vec::new(),
            token: cboe_token.to_string(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsiderTransaction {
    pub code: String,
    pub date: String,
    pub exchange: String,
    pub link: String,
    pub owner_cik: ::serde_json::Value,
    pub owner_name: String,
    pub owner_relationship: ::serde_json::Value,
    pub owner_title: String,
    pub post_transaction_amount: Option<i64>,
    pub report_date: ::serde_json::Value,
    pub transaction_acquired_disposed: String,
    pub transaction_amount: i64,
    pub transaction_code: String,
    pub transaction_date: String,
    pub transaction_price: f64,
}
impl InsiderTransaction {
    pub fn net_result(&self) -> f64 {
        match self.transaction_acquired_disposed.as_ref() {
            "A" => self.transaction_price * self.transaction_amount as f64,
            "D" => -self.transaction_price * self.transaction_amount as f64,
            _ => {
                println!("{}", self.transaction_acquired_disposed);
                0.0
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Signal {
    pub symbol: String,
    pub side: SignalType,
    pub quantity_1: i64,
    pub quantity_2: i64,
    pub large_trader_net_value: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SignalType {
    Buy,
    Sell,
}

pub fn get_signals(chains: &Vec<OptionChain>, quantity_1: f64, quantity_2: f64) -> Vec<Signal> {
    let qqq_price = chains
        .iter()
        .find(|chain| chain.symbol == "QQQ")
        .unwrap()
        .underlying_mid;
    let spy_price = chains
        .iter()
        .find(|chain| chain.symbol == "SPY")
        .unwrap()
        .underlying_mid;
    let mut signals: Vec<Signal> = Vec::new();
    let mut large = chains
        .iter()
        .filter(|chain| chain.large_trader_net_value.abs() > 0.0)
        .collect_vec();
    large.sort_unstable_by_key(|chain| chain.large_trader_net_value.abs() as i64);
    let sp_500 = chains
        .iter()
        .filter(|chain| chain.symbol == "SPY" || chain.symbol == "^SPX")
        .collect_vec();
    let nasdaq = chains
        .iter()
        .filter(|chain| chain.symbol == "QQQ" || chain.symbol == "^NDX")
        .collect_vec();
    let all_others = chains
        .iter()
        .filter(|chain| !matches!(chain.symbol.as_ref(), "SPY" | "QQQ" | "^SPX" | "^NDX"))
        .collect_vec();
    let mut sp_large_trader_net = 0.0;
    for chain in sp_500 {
        sp_large_trader_net += chain.large_trader_net_value;
    }
    let mut nq_large_trader_net = 0.0;
    for chain in nasdaq {
        nq_large_trader_net += chain.large_trader_net_value;
    }
    for chain in &all_others {
        if chain.large_trader_net_value > 0.0 && chain.bias > 2 {
            signals.push(chain.to_signal(quantity_1, quantity_2))
        } else if chain.large_trader_net_value < 0.0 && chain.bias < -2 {
            signals.push(chain.to_signal(quantity_1, quantity_2))
        }
    }
    let spy_signal_type = if sp_large_trader_net > 0.0 {
        SignalType::Buy
    } else {
        SignalType::Sell
    };
    let spy_signal = Signal {
        symbol: "SPY".to_string(),
        side: spy_signal_type,
        quantity_1: remove_decimals(quantity_1 / spy_price),
        quantity_2: remove_decimals(quantity_2 / spy_price),
        large_trader_net_value: sp_large_trader_net,
    };
    let qqq_signal_type = if nq_large_trader_net > 0.0 {
        SignalType::Buy
    } else {
        SignalType::Sell
    };
    let qqq_signal = Signal {
        symbol: "QQQ".to_string(),
        side: qqq_signal_type,
        quantity_1: remove_decimals(quantity_1 / qqq_price),
        quantity_2: remove_decimals(quantity_2 / qqq_price),
        large_trader_net_value: nq_large_trader_net,
    };
    signals.push(spy_signal);
    signals.push(qqq_signal);
    signals.sort_unstable_by_key(|signal| -signal.large_trader_net_value as i64);
    let mut final_vec = signals.clone().into_iter().take(4).collect_vec();
    signals.sort_unstable_by_key(|signal| signal.large_trader_net_value as i64);
    final_vec.append(&mut signals.into_iter().take(4).collect_vec());
    final_vec
}
