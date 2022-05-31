use std::sync::{Arc, RwLock};
use itertools::Itertools;
use serde::Serialize;
use crate::{OPTION_COMMISSION, models::OptionChain, others::round_to_decimals, single_options::{OptionData, OptionType}};
#[derive(Debug, Serialize, Clone)]
pub struct VerticalSpread {
    pub underlying_symbol: String,
    pub option_type: OptionType,
    pub expiration_date: String,
    pub dte: i64,
    pub sell_strike: f64,
    pub buy_strike: f64,
    pub natural_price: f64,
    pub max_profit: f64,
    pub max_loss: f64,
    pub net_iv: f64,
    pub risk_reward_ratio: f64,
    pub position_delta: f64,
    pub difficulty: f64,
    pub rank: f64,
}
impl VerticalSpread {
    pub fn from_options(sell_option:&OptionData, buy_option:&OptionData) -> Self {
        let natural_price = round_to_decimals(sell_option.bid_price.unwrap() - buy_option.ask_price.unwrap(),2);
        let max_profit = round_to_decimals(100.0*natural_price - (2.0*OPTION_COMMISSION),2);
        let strike_diff = (buy_option.strike - sell_option.strike).abs();
        let max_loss = round_to_decimals(100.0*strike_diff - max_profit,2);
        let delta_sum = buy_option.delta-sell_option.delta;
        let position_delta = delta_sum*100.0;
        let risk_reward_ratio =max_profit/max_loss;
        let net_iv = 100.0*(sell_option.iv-buy_option.iv);
        let difficulty = sell_option.dte as f64*position_delta.abs();
        Self {
            underlying_symbol: sell_option.root.clone(),
            expiration_date: sell_option.expiration_date.clone(),
            dte: sell_option.dte,
            sell_strike: sell_option.strike,
            buy_strike: buy_option.strike,
            max_profit,
            max_loss,
            risk_reward_ratio,
            position_delta,
            difficulty,
            rank: net_iv/difficulty,
            option_type: sell_option.kind,
            natural_price,
            net_iv,
        }
    }
}

pub async fn get_vertical_spreads(data: Arc<RwLock<OptionChain>>, output: Arc<RwLock<Vec<VerticalSpread>>>,condoors_lock:Arc<RwLock<Vec<IronCondoor>>>) {
    let mut output_vec:Vec<VerticalSpread> = Vec::new();
    let option_chain = data.read().unwrap();
    for expiration in &option_chain.expirations {
        let otm_options_in_this_expiration:Vec<&OptionData> = option_chain.options.iter().filter(|option| &option.dte == expiration &&option.otm).collect();
        for sell_option in &otm_options_in_this_expiration {
            let mut strikes_in_this_exp_int = otm_options_in_this_expiration.iter().map(|option|(100.0*option.strike) as i64).unique().collect_vec();
            strikes_in_this_exp_int.sort_unstable();
            let strikes_in_this_exp:Vec<f64> = strikes_in_this_exp_int.into_iter().map(|strike|(strike/100) as f64).collect_vec();
            let sell_strike_position = strikes_in_this_exp.iter().position(|strike|strike == &sell_option.strike);
            if let Some(position) = sell_strike_position {
                if position != 0 {
                    let next_strike = if sell_option.kind == OptionType::Call {position+1} else {position-1};
                    if strikes_in_this_exp.len() > next_strike {
                        let other_option = otm_options_in_this_expiration.iter().find(|option|option.kind == sell_option.kind && option.strike == strikes_in_this_exp[next_strike]);
                        if let Some(buy_option) = other_option {
                            let spread = VerticalSpread::from_options(sell_option, buy_option);
                            if spread.max_profit > 0.0 && spread.net_iv > 0.0 {output_vec.push(spread)};
                        }  
                    }
                }   
            }
        }
    }
    output_vec.sort_unstable_by_key(|vert|-(vert.rank*10000.0) as i64);
    let mut condoors:Vec<IronCondoor> = get_condoors(&output_vec,option_chain.underlying_mid);
    let mut write = output.write().unwrap();
    write.append(&mut output_vec);
    let mut condoor_write = condoors_lock.write().unwrap();
    condoor_write.append(&mut condoors)
}

#[derive(Debug,Clone,Serialize)]
pub struct IronCondoor {
    pub underlying_symbol: String,
    pub underlying_mid: f64,
    pub expiration_date: String,
    pub dte: i64,
    pub strikes: String,
    pub net_iv: f64,
    pub natural_price: f64,
    pub max_profit: f64,
    pub max_loss: f64,
    pub risk_reward_ratio: f64,
    pub position_delta: f64,
    pub rank: f64,
}
impl IronCondoor {
    pub fn from_spreads(put_spread:&VerticalSpread, call_spread:&VerticalSpread,underlying_mid:f64) -> Self {
        let natural_price = put_spread.natural_price+call_spread.natural_price;
        let max_profit = put_spread.max_profit + call_spread.max_profit;
        let max_loss = if put_spread.max_loss > call_spread.max_loss {put_spread.max_loss} else {call_spread.max_loss};
        let risk_reward_ratio = (100.0*natural_price)/max_loss;
        let position_delta = put_spread.position_delta + call_spread.position_delta;
        let net_iv = put_spread.net_iv + call_spread.net_iv;
        Self {
            underlying_symbol: put_spread.underlying_symbol.clone(),
            expiration_date: put_spread.expiration_date.clone(),
            strikes: format!("{}-{}-{}-{}",put_spread.buy_strike,put_spread.sell_strike,call_spread.sell_strike,call_spread.buy_strike),
            natural_price,
            max_profit,
            max_loss,
            risk_reward_ratio,
            rank: (risk_reward_ratio)/(put_spread.dte as f64*position_delta),
            position_delta,
            underlying_mid,
            dte: put_spread.dte,
            net_iv,
        }
    }
}
pub fn get_condoors(spreads:&[VerticalSpread],underlying_mid:f64) -> Vec<IronCondoor> {
    let mut output_vec:Vec<IronCondoor> = Vec::new();
    let expirations= spreads.iter().map(|spread|spread.dte).unique().collect_vec();
    for expiration in expirations {
        let mut calls = spreads.iter().filter(|spread|spread.dte == expiration && spread.option_type == OptionType::Call).collect_vec();
        let mut puts = spreads.iter().filter(|spread|spread.dte == expiration && spread.option_type == OptionType::Put).collect_vec();
        calls.sort_by_key(|spread| -(spread.rank*10000.0) as i64);
        puts.sort_by_key(|spread| -(spread.rank*10000.0) as i64);
        if !calls.is_empty() && !puts.is_empty() {
            let condoor = IronCondoor::from_spreads(calls[0],puts[0],underlying_mid);
            if condoor.net_iv > 0.0 {output_vec.push(condoor)};
        } 
    };
    output_vec
}