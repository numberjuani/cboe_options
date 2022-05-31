use crate::models::DividendInformation;
use crate::models::DividendPeriod;
use crate::models::DividendsLock;
use crate::models::OptionChain;
use crate::models::OptionsLock;
use crate::models::ShortStockData;
use crate::models::TradesLock;
use crate::others::get_new_york_time;
use crate::requests::get_dividend_info_mt;
use crate::requests::get_insider_data_mt;
use crate::requests::get_options_mt;
use crate::requests::get_short_ratio_mt;
use crate::requests::get_trades_mt;
use crate::single_options::OptionData;
use crate::single_options::OptionType;
use crate::spreads::get_spreads;
use crate::trades::estimate_transaction;
use crate::trades::Expectation;
use crate::AMOUNT_IN_ACCOUNT;
use crate::MONSTER_SIZE;
use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use itertools::Itertools;
use std::sync::Arc;

impl OptionChain {
    pub async fn get(
        symbol: &str,
        token: &str,
        short_fee_data: Option<&ShortStockData>,
    ) -> Option<Self> {
        let options_lock = Arc::new(std::sync::RwLock::new(OptionsLock::new(symbol, token)));
        let divi_lock = Arc::new(std::sync::RwLock::new(DividendsLock::new(symbol)));
        let trades_lock = Arc::new(std::sync::RwLock::new(TradesLock::new(symbol, token)));
        let short_ratio = Arc::new(std::sync::RwLock::new((symbol, 0.0)));
        let insider_data = Arc::new(std::sync::RwLock::new((symbol, 0.0)));
        tokio::join!(
            get_options_mt(options_lock.clone()),
            get_dividend_info_mt(divi_lock.clone()),
            get_trades_mt(trades_lock.clone()),
            get_insider_data_mt(insider_data.clone()),
            get_short_ratio_mt(short_ratio.clone()),
        );
        let dividend_info = divi_lock.read().unwrap().dividends.clone();
        let mut trades = trades_lock.read().unwrap().trades.clone();
        let options_lock = options_lock.read().unwrap();
        let insiders = insider_data.read().unwrap().1;
        let data = options_lock.options.clone();
        let short_interest_percent = short_ratio.read().unwrap().1;
        if data.options.is_empty() || trades.is_empty() {
            return None;
        }
        println!("Obtained all data");
        let options = data.options;
        let mut options_with_calculated_values: Vec<OptionData> = Vec::new();
        let mut put_oi = 0;
        let mut call_oi = 0;
        let mut call_volume = 0;
        let mut put_volume = 0;
        let datetime = get_new_york_time();
        let symbol_date = format!(
            "{}-{}-{}-{}",
            symbol,
            datetime.month(),
            datetime.day(),
            datetime.year()
        );
        for option in options {
            match option.kind {
                OptionType::Call => {
                    call_oi += option.open_interest;
                    call_volume += option.option_volume;
                }
                OptionType::Put => {
                    put_oi += option.open_interest;
                    put_volume += option.option_volume;
                }
            }
            let trades_in_this_option = trades
                .iter()
                .positions(|trade| option.symbol == trade.symbol)
                .collect_vec();
            for position in &trades_in_this_option {
                trades[*position] = trades[*position].clone().get_values(&symbol_date);
                trades[*position].transaction_estimate =
                    estimate_transaction(&option, &trades[*position]);
                trades[*position].current_delta = option.delta;
            }
            if option.valid_option() {
                let calculated = option.calculate_values(data.implied_underlying_mid.unwrap_or(
                    0.5 * (data.implied_underlying_ask.unwrap_or(0.0)
                        + data.implied_underlying_bid.unwrap_or(0.0)),
                ));
                options_with_calculated_values.push(calculated);
            };
        }
        let mut dealer_delta = 0.0;
        let mut naive_dealer_delta = 0.0;
        for trade in &trades {
            dealer_delta += trade.dealer_delta();
            naive_dealer_delta += trade.naive_dealer_delta();
        }
        let mut spreads = get_spreads(trades.clone());
        let single_legs = trades
            .into_iter()
            .filter(|trade| !trade.condition_id.is_multi_leg())
            .map(|trade| trade.to_spread())
            .collect_vec();
        spreads.extend(single_legs);
        let large_trades = spreads
            .iter()
            .filter(|spread| spread.net_value.abs() > MONSTER_SIZE)
            .collect_vec();
        let mut large_trader_delta = 0.0;
        let mut large_trader_opening_delta = 0.0;
        let mut large_trader_absolute_value = 0.0;
        let mut large_trader_net_value = 0.0;
        let mut large_trader_opening_net_value = 0.0;
        let mut large_trader_opening_absolute_value = 0.0;
        for trade in large_trades {
            large_trader_delta += trade.current_delta;
            large_trader_absolute_value += trade.net_value.abs();
            large_trader_net_value += trade.net_value;
            if trade.opening_trade {
                large_trader_opening_delta += trade.current_delta;
                large_trader_opening_net_value += trade.net_value;
                large_trader_opening_absolute_value += trade.net_value.abs();
            }
        }
        let large_trader_expectation = if large_trader_opening_delta > 0.0 {
            Expectation::Bullish
        } else if large_trader_opening_delta < 0.0 {
            Expectation::Bearish
        } else {
            Expectation::Neutral
        };
        let mut bias = 0;
        let put_call_oi_ratio = put_oi as f64 / call_oi as f64;
        let put_call_volume_ratio = put_volume as f64 / call_volume as f64;
        if put_call_volume_ratio > 1.0 {
            bias += 1
        } else {
            bias -= 1
        };
        if insiders > 0.0 {
            bias += 1
        } else if insiders < 0.0 {
            bias -= 1
        };
        if dealer_delta > 0.0 {
            bias += 1
        } else {
            bias -= 1
        };
        if naive_dealer_delta > 0.0 {
            bias += 1
        } else {
            bias -= 1
        };
        if large_trader_delta > 0.0 {
            bias += 1
        } else if large_trader_delta < 0.0 {
            bias -= 1
        };
        if large_trader_opening_delta > 0.0 {
            bias += 1
        } else if large_trader_opening_delta < 0.0 {
            bias -= 1
        };
        let shares_to_trade: i64 = if !symbol.contains('^') {
            remove_decimals((AMOUNT_IN_ACCOUNT / 10.0) / data.implied_underlying_mid.unwrap_or(0.0))
        } else {
            0
        };
        Some(OptionChain {
            symbol: data.symbol.clone(),
            underlying_mid: 0.5
                * (data.implied_underlying_ask.unwrap_or(0.0)
                    + data.implied_underlying_bid.unwrap_or(0.0)),
            data_timestamp: datetime.format("%v %r %Z").to_string(),
            ex_div_date: if let Some(divi) = &dividend_info {
                divi.ex_div_date.clone()
            } else {
                String::new()
            },
            declaration_date: if let Some(divi) = &dividend_info {
                divi.declaration_date.clone()
            } else {
                None
            },
            payment_date: if let Some(divi) = &dividend_info {
                divi.payment_date.clone()
            } else {
                None
            },
            period: if let Some(divi) = &dividend_info {
                divi.period.unwrap_or(DividendPeriod::Unknown)
            } else {
                DividendPeriod::None
            },
            record_date: if let Some(divi) = &dividend_info {
                divi.record_date
                    .as_ref()
                    .unwrap_or(&String::new())
                    .to_string()
            } else {
                String::new()
            },
            unadjusted_value: if let Some(divi) = &dividend_info {
                divi.unadjusted_value
            } else {
                0.0
            },
            value: if let Some(divi) = &dividend_info {
                divi.unadjusted_value
            } else {
                0.0
            },
            estimated: if let Some(divi) = &dividend_info {
                divi.estimated
            } else {
                false
            },
            dividend_info,
            short_fee: if let Some(short) = short_fee_data {
                short.fee_rate.parse().unwrap()
            } else if !symbol.contains('^') {
                f64::INFINITY
            } else {
                0.0
            },
            shares_available: if let Some(short) = short_fee_data {
                short.available.parse().unwrap()
            } else {
                "0".to_string()
            },
            options: options_with_calculated_values,
            dealer_delta,
            put_call_oi_ratio,
            insider_net_transaction: insiders,
            naive_dealer_delta,
            short_interest_percent,
            bias,
            symbol_date,
            date: datetime.date().format("%D").to_string(),
            spreads,
            large_trader_delta,
            large_trader_expectation,
            shares_to_trade,
            put_call_volume_ratio,
            large_trader_opening_delta,
            large_trader_absolute_value,
            large_trader_net_value,
            large_trader_opening_net_value,
            large_trader_opening_absolute_value,
        })
    }
}

impl DividendInformation {
    pub fn days_to_ex_date(&self) -> i64 {
        (chrono::NaiveDate::parse_from_str(&self.ex_div_date, "%F").unwrap()
            - chrono::Local::now().naive_local().date())
        .num_days()
    }
    pub fn estimate_next_date(self) -> Self {
        let days_to_add: i64 = match self.period {
            Some(per) => match per {
                crate::models::DividendPeriod::Quarterly => 84,
                crate::models::DividendPeriod::Annual => 365,
                crate::models::DividendPeriod::Monthly => 29,
                crate::models::DividendPeriod::Other => 0,
                crate::models::DividendPeriod::SemiAnnual => 182,
                crate::models::DividendPeriod::None => 0,
                crate::models::DividendPeriod::Unknown => 0,
            },
            None => 0,
        };
        let mut projected_date =
            chrono::NaiveDate::parse_from_str(&add_to_date(&self.ex_div_date, days_to_add), "%F")
                .unwrap();
        let projected_weekday = projected_date.weekday();
        let previous_weekday = chrono::NaiveDate::parse_from_str(&self.ex_div_date, "%F")
            .unwrap()
            .weekday();
        let mut counting_up = 1;
        let mut counting_down = 1;
        if projected_weekday != previous_weekday && days_to_add == 29 {
            loop {
                projected_date += Duration::days(1);
                counting_up += 1;
                if projected_date.weekday() == previous_weekday {
                    break;
                }
            }
            projected_date = chrono::NaiveDate::parse_from_str(
                &add_to_date(&self.ex_div_date, days_to_add),
                "%F",
            )
            .unwrap();
            loop {
                projected_date -= Duration::days(1);
                counting_down += 1;
                if projected_date.weekday() == previous_weekday {
                    break;
                }
            }
            if counting_up > counting_down {
                projected_date -= Duration::days(counting_down)
            } else {
                projected_date += Duration::days(counting_up)
            }
        }
        Self {
            ex_div_date: projected_date.format("%F").to_string(),
            declaration_date: option_add_to_date(self.declaration_date, days_to_add),
            payment_date: option_add_to_date(self.payment_date, days_to_add),
            record_date: self.record_date.map(|date| add_to_date(&date, days_to_add)),
            estimated: true,
            poisoned: false,
            ..self
        }
    }
    pub fn new() -> Self {
        Self {
            currency: None,
            ex_div_date: String::new(),
            declaration_date: None,
            payment_date: None,
            period: None,
            record_date: None,
            unadjusted_value: 0.0,
            value: 0.0,
            estimated: true,
            poisoned: true,
        }
    }
    pub fn mark_poisoned() -> Self {
        let mut new_self = Self::new();
        new_self.poisoned = true;
        new_self
    }
}

pub fn option_add_to_date(date: Option<String>, days_to_add: i64) -> Option<String> {
    date.map(|date| {
        (NaiveDate::parse_from_str(&date, "%F").unwrap() + Duration::days(days_to_add))
            .format("%F")
            .to_string()
    })
}

pub fn add_to_date(date: &str, days_to_add: i64) -> String {
    (NaiveDate::parse_from_str(date, "%F").unwrap() + Duration::days(days_to_add))
        .format("%F")
        .to_string()
}

pub fn remove_decimals(float: f64) -> i64 {
    let string_float = float.to_string();
    let split = string_float.split('.').collect_vec();
    match split[0].parse::<i64>() {
        Ok(out) => out,
        Err(_) => 0,
    }
}
