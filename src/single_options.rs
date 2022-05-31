use chrono::Local;
use chrono::NaiveDate;
use serde::Deserialize;
use serde::Serialize;
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Copy)]
pub enum OptionType {
    #[serde(rename(deserialize = "C"))]
    Call,
    #[serde(rename(deserialize = "P"))]
    Put,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OptionData {
    #[serde(default = "to_be_calculated_float")]
    pub delta: f64,
    #[serde(rename = "expiry")]
    #[serde(default = "to_be_calculated_string")]
    pub expiration_date: String,
    #[serde(default = "to_be_calculated_int")]
    pub dte: i64,
    #[serde(default = "to_be_calculated_float")]
    pub gamma: f64,
    #[serde(default = "to_be_calculated_float")]
    pub iv: f64,
    pub open_interest: i64,
    #[serde(rename = "option")]
    #[serde(default = "to_be_calculated_string")]
    pub symbol: String,
    #[serde(rename = "option_ask")]
    pub ask_price: Option<f64>,
    pub option_ask_size: Option<i64>,
    #[serde(rename = "option_bid")]
    pub bid_price: Option<f64>,
    pub option_bid_size: Option<i64>,
    pub option_close: Option<f64>,
    pub option_high: Option<f64>,
    pub option_last_trade_price: f64,
    pub option_low: Option<f64>,
    #[serde(rename = "option_mid")]
    pub mid_price: Option<f64>,
    pub option_open: Option<f64>,
    pub option_prev_day_close: Option<f64>,
    pub option_trade_count: Option<i64>,
    #[serde(rename = "option_type")]
    pub kind: OptionType,
    pub option_volume: i64,
    #[serde(default = "to_be_calculated_float")]
    pub rho: f64,
    pub root: String,
    #[serde(default = "to_be_calculated_float")]
    pub strike: f64,
    #[serde(default = "to_be_calculated_float")]
    pub theta: f64,
    pub timestamp: Option<String>,
    #[serde(default = "to_be_calculated_float")]
    pub vega: f64,
    #[serde(default = "to_be_calculated_float")]
    pub intrinsic_value: f64,
    #[serde(default = "to_be_calculated_float")]
    pub extrinsic_value: f64,
    #[serde(default = "to_be_calculated_bool")]
    pub otm: bool,
}
impl OptionData {
    fn intrinsic_value(&self, underlying_mid: f64) -> f64 {
        let mut value = match self.kind {
            OptionType::Call => underlying_mid - self.strike,
            OptionType::Put => self.strike - underlying_mid,
        };
        if value < 0.0 {
            value = 0.0
        }
        value
    }
    fn extrinsic_value(&self, underlying_mid: f64) -> f64 {
        let num = self.mid_price.unwrap_or(0.0) - self.intrinsic_value(underlying_mid);
        if num > 0.0 {
            num
        } else {
            0.0
        }
    }
    fn otm(&self, underlying_mid: f64) -> bool {
        match self.kind {
            OptionType::Call => underlying_mid < self.strike,
            OptionType::Put => underlying_mid > self.strike,
        }
    }
    fn dte(&self) -> i64 {
        (NaiveDate::parse_from_str(&self.expiration_date, "%F").unwrap()
            - Local::now().naive_local().date())
        .num_days()
    }
    pub fn valid_option(&self) -> bool {
        self.ask_price.is_some()
            && self.bid_price.is_some()
            && self.ask_price.unwrap() > 0.0
            && self.bid_price.unwrap() > 0.0
            && self.open_interest > 0
            && NaiveDate::parse_from_str(&self.expiration_date, "%F").unwrap()
                > Local::now().naive_local().date()
    }
    pub fn calculate_values(self, underlying_mid: f64) -> Self {
        Self {
            dte: self.dte(),
            intrinsic_value: self.intrinsic_value(underlying_mid),
            extrinsic_value: self.extrinsic_value(underlying_mid),
            otm: self.otm(underlying_mid),
            ..self
        }
    }
    pub fn display(&self) -> String {
        format!(
            "X:{}-S:{}-B:{}-A:{}",
            self.expiration_date,
            self.strike,
            self.bid_price.unwrap(),
            self.ask_price.unwrap()
        )
    }
}

pub fn to_be_calculated_int() -> i64 {
    0
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
