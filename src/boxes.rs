use std::sync::{Arc, RwLock};

use crate::{
    models::{DividendInformation, OptionChain},
    others::{get_margin_loan_cost, get_short_fee_cost, round_to_decimals},
    single_options::{OptionData, OptionType},
    MAX_BOX_DTE, MAX_SHORT_BOX_SHORT_FEE, OPTION_COMMISSION,
};
use serde::Serialize;
#[derive(Debug, Clone, Serialize, Copy, PartialEq, PartialOrd)]
pub enum BoxType {
    LongBox,
    ShortBox,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionsBox {
    pub timestamp: String,
    pub underlying_symbol: String,
    pub expiration_date: String,
    pub dte: i64,
    pub box_type: BoxType,
    pub low_strike: f64,
    pub high_strike: f64,
    pub expiration_value: f64,
    pub natural_price: f64,
    pub gross_profit: f64,
    pub dividend_impact: f64,
    pub options_commissions: f64,
    pub max_margin_loan_cost: f64,
    pub short_fee_estimate: f64,
    pub max_fees: f64,
    pub net_profit: f64,
    pub cash_requiered: f64,
    pub net_return: f64,
    pub net_iv: f64,
    pub difficulty: f64,
    pub max_size: i64,
    pub dividend_ex_date: Option<String>,
    pub days_to_ex_date: Option<i64>,
    pub div_info_estimated: bool,
    pub short_fee: f64,
    pub rank: f64,
}

impl OptionsBox {
    pub fn from_options(
        options: [&OptionData; 4],
        timestamp: &str,
        dividend_info: &Option<DividendInformation>,
        short_fee: f64,
        underlying_last: f64,
    ) -> Self {
        let low_strike = options
            .iter()
            .min_by_key(|option| (option.strike * 100.0) as i64)
            .unwrap()
            .strike;
        let high_strike = options
            .iter()
            .max_by_key(|option| (option.strike * 100.0) as i64)
            .unwrap()
            .strike;
        let expiration_value = high_strike - low_strike;
        let otm: Vec<&&OptionData> = options.iter().filter(|option| option.otm).collect();
        let itm: Vec<&&OptionData> = options.iter().filter(|option| !option.otm).collect();
        let mut otm_credit = 0.0;
        let mut otm_debit = 0.0;
        let mut itm_credit = 0.0;
        let mut itm_debit = 0.0;
        for option in &options {
            if option.otm {
                otm_credit += option.bid_price.unwrap_or(0.0);
                otm_debit += option.ask_price.unwrap_or(0.0);
            } else {
                itm_credit += option.bid_price.unwrap_or(0.0);
                itm_debit += option.ask_price.unwrap_or(0.0);
            }
        }
        let long_box_cost = round_to_decimals(otm_credit - itm_debit, 2);
        let short_box_credit = round_to_decimals(itm_credit - otm_debit, 2);
        let long_max_profit = round_to_decimals(100.0 * (expiration_value + long_box_cost), 2);
        let short_max_profit = round_to_decimals(100.0 * (short_box_credit - expiration_value), 2);
        let box_type = if long_max_profit > short_max_profit {
            BoxType::LongBox
        } else {
            BoxType::ShortBox
        };
        match box_type {
            BoxType::LongBox => {
                let options_commissions = OPTION_COMMISSION * 4.0;
                let max_margin_loan_cost = 0.0;
                let max_fees = options_commissions + max_margin_loan_cost;
                let mut size_vec: Vec<i64> = itm
                    .iter()
                    .map(|option| option.option_ask_size.unwrap())
                    .collect();
                let mut bid_size_vec: Vec<i64> = otm
                    .iter()
                    .map(|option| option.option_bid_size.unwrap())
                    .collect();
                size_vec.append(&mut bid_size_vec);
                let net_profit = long_max_profit - max_fees;
                let net_return = (net_profit) / (long_box_cost.abs() + max_fees);
                let net_iv: f64 = 100.0
                    * (-itm.iter().map(|option| option.iv).sum::<f64>()
                        + otm.iter().map(|option| option.iv).sum::<f64>());
                let difficulty = options[0].dte as f64 * net_iv;
                size_vec.sort_unstable();
                Self {
                    high_strike,
                    low_strike,
                    expiration_value,
                    natural_price: long_box_cost,
                    gross_profit: long_max_profit,
                    net_return,
                    underlying_symbol: options[0].root.clone(),
                    expiration_date: options[0].expiration_date.clone(),
                    box_type: BoxType::LongBox,
                    dte: options[0].dte,
                    cash_requiered: (long_box_cost.abs() * 100.0) + options_commissions,
                    max_size: size_vec[0],
                    net_profit,
                    timestamp: timestamp.to_string(),
                    max_margin_loan_cost,
                    options_commissions,
                    max_fees,
                    dividend_ex_date: dividend_info.as_ref().map(|divi| divi.ex_div_date.clone()),
                    days_to_ex_date: dividend_info.as_ref().map(|divi| divi.days_to_ex_date()),
                    dividend_impact: 0.0,
                    div_info_estimated: if let Some(divi) = dividend_info {
                        divi.estimated
                    } else {
                        false
                    },
                    short_fee,
                    difficulty,
                    short_fee_estimate: 0.0,
                    rank: net_return / difficulty,
                    net_iv,
                }
            }
            BoxType::ShortBox => {
                let max_margin_loan_cost =
                    get_margin_loan_cost(high_strike * 100.0, options[0].dte);
                let short_fee_cost = get_short_fee_cost(short_fee, underlying_last, options[0].dte);
                let dividend_impact = if let Some(divi) = dividend_info {
                    if divi.days_to_ex_date() < options[0].dte {
                        100.0 * (divi.value)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                let mut size_vec: Vec<i64> = itm
                    .iter()
                    .map(|option| option.option_bid_size.unwrap())
                    .collect();
                let mut ask_size_vec: Vec<i64> = otm
                    .iter()
                    .map(|option| option.option_ask_size.unwrap())
                    .collect();
                size_vec.append(&mut ask_size_vec);
                let options_commissions = OPTION_COMMISSION * 4.0;
                let max_fees =
                    options_commissions + max_margin_loan_cost + dividend_impact + short_fee_cost;
                let net_profit = short_max_profit - max_fees;
                let cash_requiered = 100.0 * (high_strike - short_box_credit);
                let net_return = 100.0 * (net_profit / cash_requiered);
                size_vec.sort_unstable();
                let difficulty = options[0].dte as f64 * short_fee;
                let net_iv: f64 = 100.0
                    * (itm.iter().map(|option| option.iv).sum::<f64>()
                        - otm.iter().map(|option| option.iv).sum::<f64>());
                Self {
                    high_strike,
                    low_strike,
                    expiration_value,
                    natural_price: short_box_credit,
                    gross_profit: short_max_profit,
                    net_return,
                    underlying_symbol: options[0].root.clone(),
                    expiration_date: options[0].expiration_date.clone(),
                    box_type: BoxType::ShortBox,
                    dte: options[0].dte,
                    cash_requiered,
                    max_size: size_vec[0],
                    options_commissions,
                    net_profit,
                    timestamp: timestamp.to_string(),
                    max_margin_loan_cost,
                    max_fees,
                    dividend_ex_date: dividend_info.as_ref().map(|divi| divi.ex_div_date.clone()),
                    days_to_ex_date: dividend_info.as_ref().map(|divi| divi.days_to_ex_date()),
                    dividend_impact,
                    div_info_estimated: if let Some(divi) = dividend_info {
                        divi.estimated
                    } else {
                        false
                    },
                    short_fee,
                    difficulty,
                    short_fee_estimate: short_fee_cost,
                    rank: (net_return * net_iv) / difficulty,
                    net_iv,
                }
            }
        }
    }
}
pub async fn get_boxes_mt(data: Arc<RwLock<OptionChain>>, output: Arc<RwLock<Vec<OptionsBox>>>) {
    let mut output_vec: Vec<OptionsBox> = Vec::new();
    let chain = data.read().unwrap();
    for expiration in &chain.expirations {
        if expiration < &MAX_BOX_DTE {
            let this_expiration: Vec<&OptionData> = chain
                .options
                .iter()
                .filter(|option| &option.dte == expiration)
                .collect();
            let otm_puts_in_this_expiration: Vec<&&OptionData> = this_expiration
                .iter()
                .filter(|option| option.otm && option.kind == OptionType::Put)
                .collect();
            for otm_put in otm_puts_in_this_expiration {
                if let Some(itm_call) = this_expiration.iter().find(|option| {
                    option.kind == OptionType::Call
                        && option.strike == otm_put.strike
                        && option.root == otm_put.root
                }) {
                    let possible_sell_calls: Vec<&&OptionData> = this_expiration
                        .iter()
                        .filter(|option| {
                            option.kind == OptionType::Call
                                && option.otm
                                && option.root == otm_put.root
                        })
                        .collect();
                    for otm_call in possible_sell_calls {
                        if let Some(itm_put) = this_expiration.iter().find(|option| {
                            option.kind == OptionType::Put
                                && option.strike == otm_call.strike
                                && option.root == otm_put.root
                        }) {
                            let short_fee = if chain.symbol.contains('^') {
                                0.0
                            } else {
                                chain.short_fee
                            };
                            let boxx = OptionsBox::from_options(
                                [otm_call, itm_call, otm_put, itm_put],
                                &chain.data_timestamp,
                                &chain.dividend_info,
                                short_fee,
                                chain.underlying_mid,
                            );
                            if boxx.net_profit > 0.0 {
                                if !chain.symbol.contains('^') {
                                    match &chain.dividend_info {
                                        Some(divi) => match boxx.box_type {
                                            BoxType::LongBox => output_vec.push(boxx),
                                            BoxType::ShortBox => {
                                                if !divi.poisoned
                                                    && chain.short_fee < MAX_SHORT_BOX_SHORT_FEE
                                                {
                                                    output_vec.push(boxx)
                                                }
                                            }
                                        },
                                        None => {
                                            if chain.short_fee < MAX_SHORT_BOX_SHORT_FEE
                                                || chain.symbol.contains('^')
                                            {
                                                output_vec.push(boxx);
                                            }
                                        }
                                    }
                                } else {
                                    output_vec.push(boxx)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut write = output.write().unwrap();
    write.append(&mut output_vec)
}
