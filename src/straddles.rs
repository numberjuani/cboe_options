use std::{sync::{Arc, RwLock}};
use serde::Serialize;
use itertools::Itertools;

use crate::{STRADDLE_LENGTH, models::OptionChain, others::round_to_decimals, single_options::OptionData};

#[derive(Debug, Serialize, Clone)]
pub struct Straddle {
    pub underlying_symbol: String,
    pub expiration_date:String,
    pub underlying_std_dev: f64,
    pub dte: i64,
    pub strike: f64,
    pub asking_price: f64,
    pub bid_price: f64,
    pub ask_dev_ratio:f64,
    pub bid_dev_ratio: f64,  
    pub net_iv: f64,  
    pub top_breakeven: f64,
    pub bottom_breakeven:f64,
    pub min_move_profit:f64,
    pub rank: f64,
}
impl Straddle {
    pub fn from_options(options:Vec<&&OptionData>, dev:f64) -> Self {
        let asking_price = round_to_decimals(options.iter().map(|option|option.ask_price.unwrap()).sum::<f64>()+0.04,2);
        let bid_price = round_to_decimals(options.iter().map(|option|option.bid_price.unwrap()).sum::<f64>()-0.04,2);
        let top_breakeven = options[0].strike + asking_price;
        let bottom_breakeven = options[0].strike - asking_price;
        let net_iv = 100.0*(options.iter().map(|option|option.iv).sum::<f64>());
        let ask_dev_ratio = 100.0*(asking_price/dev);
        let rank = 0.5*(ask_dev_ratio+net_iv);
        Self {
            expiration_date: options[0].expiration_date.clone(),
            dte: options[0].dte,
            strike: options[0].strike,
            asking_price,
            bid_price,
            underlying_symbol: options[0].root.clone(),
            underlying_std_dev: round_to_decimals(dev,2),
            ask_dev_ratio,
            bid_dev_ratio: 100.0*(bid_price/dev),
            min_move_profit: 100.0*(asking_price/options[0].strike),
            top_breakeven,
            bottom_breakeven,
            net_iv,
            rank,
        }
    }
}
pub async fn get_straddles(data: Arc<RwLock<OptionChain>>, output: Arc<RwLock<Vec<Straddle>>>) {
    let mut output_vec:Vec<Straddle> = Vec::new();
    let option_chain = data.read().unwrap();
    if option_chain.underlying_std_dev == f64::INFINITY {return}
    for expiration in &option_chain.expirations {
        if expiration >= &STRADDLE_LENGTH {
            let options_in_this_exp = option_chain.options.iter().filter(|option|&option.dte == expiration).collect_vec();
            let mut strikes_in_this_exp:Vec<f64> = Vec::new();
            for option in &options_in_this_exp {
                if !strikes_in_this_exp.contains(&option.strike) {strikes_in_this_exp.push(option.strike)}
            }
            strikes_in_this_exp.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
            let mut call_atm_strike = 0.0;
            let mut put_atm_strike = 0.0;
            if !strikes_in_this_exp.is_empty() {
                for n in 1..strikes_in_this_exp.len()-1 {
                    if strikes_in_this_exp[n] > option_chain.underlying_mid {
                        put_atm_strike = strikes_in_this_exp[n];
                        call_atm_strike = strikes_in_this_exp[n-1];
                        break;  
                    }
                }
            }
            let call_straddle_options = options_in_this_exp.iter().filter(|option|option.strike == call_atm_strike).collect_vec();
            let put_straddle_options = options_in_this_exp.iter().filter(|option|option.strike == put_atm_strike).collect_vec();
            if call_straddle_options.len() == 2 {
                let call_straddle = Straddle::from_options(call_straddle_options, option_chain.underlying_std_dev);
                if call_straddle.ask_dev_ratio < 100.0 {output_vec.push(call_straddle)};
            };
            if put_straddle_options.len() == 2 {
                let put_straddle = Straddle::from_options(put_straddle_options, option_chain.underlying_std_dev);
                if put_straddle.ask_dev_ratio < 100.0 {output_vec.push(put_straddle)};
            };
        }
    }
    if !output_vec.is_empty() {
        let mut write = output.write().unwrap();
        write.append(&mut output_vec)
    }
    
}

