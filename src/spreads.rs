use crate::single_options::OptionType;
use crate::trades::ConditionID;
use crate::trades::Exchange;
use crate::trades::Expectation;
use crate::trades::OptionTrade;
use crate::trades::OrderAction;
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
#[derive(Debug, Serialize, PartialEq, PartialOrd, Clone, Deserialize, Copy)]
pub enum SpreadType {
    Credit,
    Debit,
    Unknown,
}

#[derive(Debug, Serialize, Clone, Deserialize, PartialEq)]
pub enum SpreadName {
    //1 leg
    #[serde(rename(serialize = "Covered Call"))]
    CoveredCall,
    CoveredPut,
    LongPut,
    ShortPut,
    LongCall,
    ShortCall,
    // 2 legs
    Vertical,
    Straddle,
    Diagonal,
    Strangle,
    Calendar,
    Synthetic,
    Ladder,
    #[serde(rename(serialize = "Synthetic Call"))]
    SyntheticCall,
    #[serde(rename(serialize = "Synthetic Put"))]
    SyntheticPut,
    #[serde(rename(serialize = "Risk Reversal"))]
    RiskReversal,
    Collar,
    //3 legs
    Butterfly,
    #[serde(rename(serialize = "Unbalanced Butterfly"))]
    UnbalancedButterfly,
    Ratio,
    // 4 legs
    #[serde(rename(serialize = "Iron Condoor"))]
    IronCondoor,
    #[serde(rename(serialize = "Iron Butterfly"))]
    IronButterfly,
    Box,
    // with stock
    Conversion,
    Reversal,
    Unrecognized,
    UnrecognizedWithStock,
}
#[derive(Debug, Serialize, Clone, Deserialize, PartialEq)]
pub struct OptionSpread {
    pub symbol: String,
    pub spread_name: SpreadName,
    pub spread_type: SpreadType,
    pub net_value: f64,
    pub expiration_date: String,
    pub dte: i64,
    pub net_iv: f64,
    pub current_delta: f64,
    pub delta_when_opened: f64,
    pub expectation: Expectation,
    pub timestamp: String,
    pub condition_id: ConditionID,
    pub exchange: Exchange,
    pub leg_number: usize,
    pub summary: String,
    pub opening_trade: bool,
    pub sequence_numbers: String,
}
pub fn get_spreads(trades: Vec<OptionTrade>) -> Vec<OptionSpread> {
    use SpreadName::*;
    let mut output_vec: Vec<OptionSpread> = Vec::new();
    let mut spreads: HashMap<String, Vec<&OptionTrade>> = HashMap::new();
    let mut multilegs = trades
        .iter()
        .filter(|trade| trade.condition_id.is_multi_leg())
        .collect_vec();
    multilegs.sort_unstable_by_key(|trade| {
        chrono::NaiveTime::parse_from_str(&trade.timestamp, "%H:%M:%S.%3f").unwrap()
    });
    for trade in multilegs {
        let key = format!(
            "{}{:#?}{:#?}{}",
            trade.timestamp, trade.exchange_id, trade.condition_id, trade.option_trade_size
        );
        if !spreads.contains_key(&key) {
            let mut trades_in_this_key = trades
                .iter()
                .filter(|matching_trade| {
                    trade.timestamp == matching_trade.timestamp
                        && trade.condition_id == matching_trade.condition_id
                        && trade.exchange_id == matching_trade.exchange_id
                        && trade.option_trade_size == matching_trade.option_trade_size
                })
                .collect_vec();
            trades_in_this_key.sort_unstable_by_key(|trade| {
                chrono::NaiveTime::parse_from_str(&trade.timestamp, "%H:%M:%S.%3f").unwrap()
            });
            if !verify_spread_legs(&mut trades_in_this_key) {
                for second_trade in &trades_in_this_key {
                    let by_size = trades_in_this_key
                        .clone()
                        .into_iter()
                        .filter(|second_match_trade| {
                            second_trade.option_trade_size == second_match_trade.option_trade_size
                        })
                        .collect_vec();
                    let secondary_key = format!("{}{}", key, second_trade.option_trade_size);
                    if !spreads.contains_key(&secondary_key) {
                        spreads.insert(secondary_key, by_size);
                    }
                }
            } else {
                spreads.insert(key, trades_in_this_key);
            }
        }
    }
    for trades_in_spread in spreads.values() {
        let mut net_value: f64 = 0.0;
        let mut expiration_date = chrono::Local::now().naive_local().date();
        let mut dte = 0;
        let mut net_iv = 0.0;
        let mut delta: f64 = 0.0;
        let mut all_call = true;
        let mut all_put = true;
        let mut summary = String::new();
        let mut opening_trade = false;
        let mut poisoned = false;
        let mut same_date = true;
        let mut same_strike = true;
        let mut all_different_strikes = true;
        let mut same_action = true;
        let mut same_amount = true;
        let mut current_delta = 0.0;
        for trade in trades_in_spread {
            net_value += trade.amount_paid();
            if chrono::NaiveDate::parse_from_str(&trade.expiry, "%F").unwrap() > expiration_date {
                expiration_date = chrono::NaiveDate::parse_from_str(&trade.expiry, "%F").unwrap();
            }
            dte = std::cmp::max(trade.dte, dte);
            net_iv += trade.net_iv();
            delta += trade.net_delta();
            current_delta += trade.net_current_delta();
            all_call = all_call && trade.option_type == OptionType::Call;
            all_put = all_put && trade.option_type == OptionType::Put;
            summary.push_str(&format!(
                "{:#?} {} of the {} {} {:#?}|",
                trade.order_action,
                trade.option_trade_size,
                trade.strike,
                trade.expiry,
                trade.option_type
            ));
            opening_trade = trade.transaction_estimate.is_opening() || opening_trade;
            poisoned = trade.order_action == OrderAction::Unknown || poisoned;
            same_date = same_date && trade.expiry == trades_in_spread[0].expiry;
            same_strike = same_strike && trade.strike == trades_in_spread[0].strike;
            all_different_strikes =
                all_different_strikes && trade.strike != trades_in_spread[0].strike;
            same_action = same_action && trade.order_action == trades_in_spread[0].order_action;
            same_amount =
                same_amount && trade.option_trade_size == trades_in_spread[0].option_trade_size;
        }
        let same_type = all_call || all_put;
        let spread_name: SpreadName = match trades_in_spread.len() {
            1 => {
                if trades_in_spread[0].condition_id.includes_stock_trade() {
                    if trades_in_spread[0].is_call_sell() {
                        CoveredCall
                    } else if trades_in_spread[0].is_put_sell() {
                        CoveredPut
                    } else if trades_in_spread[0].is_put_buy() {
                        SyntheticCall
                    } else if trades_in_spread[0].is_call_buy() {
                        SyntheticPut
                    } else {
                        UnrecognizedWithStock
                    }
                } else {
                    match trades_in_spread[0].option_type {
                        OptionType::Call => match trades_in_spread[0].order_action {
                            OrderAction::Bought => LongCall,
                            OrderAction::Sold => ShortCall,
                            OrderAction::Unknown => Unrecognized,
                        },
                        OptionType::Put => match trades_in_spread[0].order_action {
                            OrderAction::Bought => LongPut,
                            OrderAction::Sold => ShortPut,
                            OrderAction::Unknown => Unrecognized,
                        },
                    }
                }
            }
            2 => match trades_in_spread[0].condition_id.includes_stock_trade() {
                true => {
                    if (trades_in_spread[0].is_call_sell() && trades_in_spread[1].is_put_buy())
                        || (trades_in_spread[1].is_call_sell() && trades_in_spread[0].is_put_buy())
                    {
                        if same_strike {
                            Conversion
                        } else {
                            Collar
                        }
                    } else if (trades_in_spread[0].is_put_sell()
                        && trades_in_spread[1].is_call_buy())
                        || (trades_in_spread[1].is_put_sell() && trades_in_spread[0].is_call_buy())
                    {
                        if same_strike {
                            Reversal
                        } else {
                            Collar
                        }
                    } else if trades_in_spread
                        .iter()
                        .filter(|trade| trade.is_call_sell())
                        .collect_vec()
                        .len()
                        == trades_in_spread.len()
                    {
                        CoveredCall
                    } else if trades_in_spread
                        .iter()
                        .filter(|trade| trade.is_put_sell())
                        .collect_vec()
                        .len()
                        == trades_in_spread.len()
                    {
                        CoveredPut
                    } else if (trades_in_spread[0].is_put_buy()
                        && trades_in_spread[1].is_put_sell())
                        || (trades_in_spread[1].is_put_buy() && trades_in_spread[0].is_put_sell())
                    {
                        SyntheticCall
                    } else if (trades_in_spread[0].is_call_buy()
                        && trades_in_spread[1].is_call_sell())
                        || (trades_in_spread[1].is_call_buy() && trades_in_spread[0].is_call_sell())
                    {
                        SyntheticPut
                    } else {
                        UnrecognizedWithStock
                    }
                }
                false => {
                    if !same_strike && same_date && !same_action {
                        Vertical
                    } else if same_strike && !same_date && !same_action {
                        Calendar
                    } else if same_strike && same_date && same_action && !same_type {
                        Straddle
                    } else if !same_strike && same_date && same_action && !same_type {
                        Strangle
                    } else if !same_strike && same_date && !same_action && !same_type {
                        RiskReversal
                    } else if !same_strike && !same_date && !same_action {
                        Diagonal
                    } else if same_action && same_type {
                        Ladder
                    } else {
                        Unrecognized
                    }
                }
            },
            3 => {
                let mut sizes = trades_in_spread
                    .iter()
                    .map(|trade| trade.option_trade_size)
                    .collect_vec();
                sizes.sort_unstable();
                let total_sizes = sizes.iter().dedup().collect_vec().len();
                if !same_action && total_sizes == 2 {
                    Butterfly
                } else if !same_action && !same_amount && total_sizes == 3 {
                    UnbalancedButterfly
                } else if same_action && same_type {
                    Ladder
                } else {
                    Unrecognized
                }
            }
            4 => {
                let mut strikes = trades_in_spread
                    .iter()
                    .map(|trade| trade.strike)
                    .collect_vec();
                strikes.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
                if all_different_strikes && !same_action && same_date && !same_type {
                    IronCondoor
                } else if same_date && !same_action {
                    if let Some(inner_call) =
                        trades_in_spread.iter().find(|trade| trade.is_call_sell())
                    {
                        if let Some(inner_put) =
                            trades_in_spread.iter().find(|trade| trade.is_put_sell())
                        {
                            if inner_call.strike == inner_put.strike {
                                IronButterfly
                            } else if strikes.iter().dedup().collect_vec().len() == 2 {
                                Box
                            } else {
                                Unrecognized
                            }
                        } else {
                            Unrecognized
                        }
                    } else {
                        Unrecognized
                    }
                } else {
                    Unrecognized
                }
            }
            _ => {
                if same_action && same_type {
                    Ladder
                } else {
                    Unrecognized
                }
            }
        };
        let mut leg_number = trades_in_spread.len();
        if trades_in_spread[0].condition_id.includes_stock_trade() {
            let total_contracts: f64 = trades_in_spread
                .iter()
                .map(|trade| trade.option_trade_size)
                .sum::<i64>() as f64;
            let dollar_value_of_shares = match spread_name {
                CoveredCall | Conversion | SyntheticCall => {
                    total_contracts * 100.0 * trades_in_spread[0].implied_underlying_ask
                }
                CoveredPut | Reversal | SyntheticPut => {
                    -total_contracts * 100.0 * trades_in_spread[0].implied_underlying_bid
                }
                Unrecognized => 0.0,
                UnrecognizedWithStock => 0.0,
                _ => 0.0,
            };
            net_value += dollar_value_of_shares;
            let stock_summary = match spread_name {
                CoveredCall | Conversion | SyntheticCall => format!(
                    "Bought {} shares of stock at {}|",
                    100 * trades_in_spread[0].option_trade_size,
                    trades_in_spread[0].implied_underlying_ask
                ),
                CoveredPut | Reversal | SyntheticPut => format!(
                    "Shorted {} shares of stock at {}|",
                    100 * trades_in_spread[0].option_trade_size,
                    trades_in_spread[0].implied_underlying_bid
                ),
                _ => format!(
                    "Traded {} shares of stock at {}|",
                    100 * trades_in_spread[0].option_trade_size,
                    trades_in_spread[0].implied_underlying_mid
                ),
            };
            let delta_of_shares = total_contracts * 100.0;
            let delta_adjustment: f64 = match spread_name {
                CoveredCall | Conversion | SyntheticCall => delta_of_shares,
                CoveredPut | Reversal | SyntheticPut => -delta_of_shares,
                Collar => {
                    if let Some(sold_option) = trades_in_spread.iter().find(|trade| trade.is_sell())
                    {
                        match sold_option.option_type {
                            OptionType::Call => delta_of_shares,
                            OptionType::Put => -delta_of_shares,
                        }
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            delta += delta_adjustment;
            current_delta += delta_adjustment;
            summary.push_str(&stock_summary);
            leg_number += 1;
        }
        let spread_type = if net_value > 0.0 {
            SpreadType::Debit
        } else {
            SpreadType::Credit
        };
        let expectation: Expectation = if delta > 0.0 {
            Expectation::Bullish
        } else if delta < 0.0 {
            Expectation::Bearish
        } else {
            Expectation::Neutral
        };
        let spread = OptionSpread {
            symbol: trades_in_spread[0].root.clone(),
            spread_name,
            spread_type,
            net_value,
            expiration_date: expiration_date.format("%F").to_string(),
            dte,
            net_iv,
            delta_when_opened: delta,
            expectation,
            timestamp: trades_in_spread[0].timestamp.clone(),
            condition_id: trades_in_spread[0].condition_id.clone(),
            summary,
            leg_number,
            opening_trade,
            sequence_numbers: get_consecutive_summary(trades_in_spread.to_vec()),
            exchange: trades_in_spread[0].exchange_id,
            current_delta,
        };
        if !poisoned {
            output_vec.push(spread)
        }
    }
    output_vec.into_iter().dedup().collect_vec()
}

fn verify_spread_legs(legs: &mut Vec<&OptionTrade>) -> bool {
    legs.sort_unstable_by_key(|trade| trade.seq_no);
    let mut consecutive = true;
    for n in 1..legs.len() - 1 {
        consecutive = consecutive && legs[n].seq_no == legs[n - 1].seq_no + 1;
    }
    if consecutive {
        return true;
    } else {
        legs.sort_unstable_by_key(|trade| trade.exchange_seq_no);
        consecutive = true;
        for n in 1..legs.len() - 1 {
            consecutive = consecutive && legs[n].exchange_seq_no == legs[n - 1].exchange_seq_no + 1;
        }
        consecutive
    }
}

fn get_consecutive_summary(legs: Vec<&OptionTrade>) -> String {
    let mut output = String::new();
    let mut legs = legs.clone();
    legs.sort_unstable_by_key(|trade| trade.seq_no);
    let mut consecutive = true;
    for n in 1..legs.len() - 1 {
        consecutive = consecutive && legs[n].seq_no == legs[n - 1].seq_no - 1;
    }
    if consecutive {
        output.push_str("seq no ");
        legs.iter()
            .map(|trade| output.push_str(&format!("{}-", trade.seq_no)))
            .collect_vec();
        output
    } else {
        output.push_str("ex seq no ");
        legs.sort_unstable_by_key(|trade| trade.exchange_seq_no);
        legs.iter()
            .map(|trade| output.push_str(&format!("{}-", trade.exchange_seq_no)))
            .collect_vec();
        output
    }
}
