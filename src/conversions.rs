use crate::{
    models::{DividendInformation, OptionChain},
    others::{get_margin_loan_cost, get_short_fee_cost, round_to_decimals},
    single_options::{OptionData, OptionType},
    OPTION_COMMISSION, STOCK_COMMISSION,
};
use serde::Serialize;
use std::sync::{Arc, RwLock};

#[derive(Debug, Serialize, Clone)]
pub struct Conversion {
    pub timestamp: String,
    pub symbol: String,
    pub underlying_bid_or_ask: f64,
    pub expiration_date: String,
    pub dte: i64,
    pub strike: f64,
    pub sell_type: OptionType,
    pub sell_bid_price: f64,
    pub buy_ask_price: f64,
    pub option_credit: f64,
    pub strike_diff: f64,
    pub dividend_impact: f64,
    pub natural_price: f64,
    pub gross_profit: f64,
    pub options_commissions: f64,
    pub stock_commission: f64,
    pub margin_loan_interest: f64,
    pub short_fee_cost: f64,
    pub projected_net_profit: f64,
    pub cash_required: f64,
    pub net_return: f64,
    pub net_iv: f64,
    pub difficulty: f64,
    pub ranking: f64,
    pub max_size: i64,
    #[serde(skip_serializing)]
    pub symbols: [String; 3],
    pub div_ex_date: Option<String>,
    pub days_to_ex_date: Option<i64>,
    pub div_info_estimated: bool,
    pub short_fee: f64,
    pub annualized_ror: f64,
}
impl Conversion {
    pub fn from_pair(
        sell_option: &OptionData,
        buy_option: &&OptionData,
        underlying_bid_or_ask: f64,
        timestamp: &str,
        dividend_info: &Option<DividendInformation>,
        short_fee: f64,
    ) -> Self {
        let dividend_impact: f64 = if let Some(divi) = dividend_info {
            if divi.days_to_ex_date() < sell_option.dte {
                match sell_option.kind {
                    OptionType::Call => 0.0,
                    OptionType::Put => -divi.value,
                }
            } else {
                0.0
            }
        } else {
            0.0
        };
        let option_credit = round_to_decimals(
            sell_option.bid_price.unwrap() - buy_option.ask_price.unwrap(),
            2,
        );
        let mut strike_diff = if sell_option.kind == OptionType::Put {
            underlying_bid_or_ask - sell_option.strike
        } else {
            sell_option.strike - underlying_bid_or_ask
        };
        strike_diff = round_to_decimals(strike_diff, 2);
        let cash_required = round_to_decimals(
            100.0 * (underlying_bid_or_ask - option_credit - dividend_impact),
            2,
        );
        let gross_profit = round_to_decimals(100.0 * (option_credit + strike_diff), 2);
        let margin_loan_interest = if sell_option.kind == OptionType::Put {
            get_margin_loan_cost(underlying_bid_or_ask * 100.0, sell_option.dte)
        } else {
            0.0
        };
        let short_fee_cost = if sell_option.kind == OptionType::Put {
            get_short_fee_cost(short_fee, 1.25 * underlying_bid_or_ask, sell_option.dte)
        } else {
            0.0
        };
        let options_commissions = OPTION_COMMISSION * 4.0;
        let stock_commission = STOCK_COMMISSION * 2.0;
        let projected_net_profit = round_to_decimals(
            gross_profit
                - (options_commissions + margin_loan_interest + stock_commission + short_fee_cost),
            2,
        );
        let net_return = 100.0 * (projected_net_profit) / (cash_required);
        let mut size_vec = [
            sell_option.option_bid_size.unwrap_or(0),
            buy_option.option_ask_size.unwrap_or(0),
        ];
        let adj_short_fee = if sell_option.kind == OptionType::Put {
            short_fee
        } else {
            1.0
        };
        let difficulty = sell_option.dte as f64 * adj_short_fee;
        let natural_price = if sell_option.kind == OptionType::Put {
            -underlying_bid_or_ask - sell_option.bid_price.unwrap() + buy_option.ask_price.unwrap()
        } else {
            underlying_bid_or_ask + sell_option.bid_price.unwrap() - buy_option.ask_price.unwrap()
        };
        let net_iv = 100.0 * (sell_option.iv - buy_option.iv);
        size_vec.sort_unstable();
        Self {
            symbol: sell_option.root.clone(),
            underlying_bid_or_ask,
            expiration_date: sell_option.expiration_date.clone(),
            dte: sell_option.dte,
            strike: sell_option.strike,
            sell_type: sell_option.kind,
            sell_bid_price: sell_option.bid_price.unwrap(),
            buy_ask_price: buy_option.ask_price.unwrap(),
            option_credit,
            strike_diff,
            gross_profit,
            net_return,
            max_size: size_vec[0],
            cash_required,
            timestamp: timestamp.to_string(),
            options_commissions,
            margin_loan_interest,
            projected_net_profit,
            stock_commission,
            div_ex_date: dividend_info.as_ref().map(|divi| divi.ex_div_date.clone()),
            days_to_ex_date: dividend_info.as_ref().map(|divi| divi.days_to_ex_date()),
            dividend_impact,
            div_info_estimated: if let Some(divi) = dividend_info {
                divi.estimated
            } else {
                false
            },
            short_fee,
            annualized_ror: (net_return / sell_option.dte as f64) * 365.0,
            symbols: [
                sell_option.symbol.clone(),
                buy_option.symbol.clone(),
                sell_option.root.clone(),
            ],
            difficulty,
            ranking: net_return / difficulty,
            short_fee_cost,
            natural_price,
            net_iv,
        }
    }
}
pub async fn get_conversions_mt(
    data: Arc<RwLock<OptionChain>>,
    output: Arc<RwLock<Vec<Conversion>>>,
) {
    let chain = data.read().unwrap();
    if chain.symbol.contains('^') {
        return;
    }
    let atm_strikes = chain.get_at_the_money_strikes();
    let mut output_vec: Vec<Conversion> = Vec::new();
    for strike in atm_strikes {
        for expiration in &chain.expirations {
            let itm_options: Vec<&OptionData> = chain
                .options
                .iter()
                .filter(|option| {
                    !option.otm && option.dte == *expiration && option.strike == strike
                })
                .collect();
            let otm_options: Vec<&OptionData> = chain
                .options
                .iter()
                .filter(|option| option.otm && option.dte == *expiration && option.strike == strike)
                .collect();
            for sell_option in otm_options {
                let possible_buys: Vec<&&OptionData> = itm_options
                    .iter()
                    .filter(|buy_option| {
                        buy_option.kind != sell_option.kind && buy_option.strike == strike
                    })
                    .collect();
                for buy_option in possible_buys {
                    let bid_or_ask = match sell_option.kind {
                        OptionType::Call => chain.underlying_ask,
                        OptionType::Put => chain.underlying_bid,
                    };
                    let conversion = Conversion::from_pair(
                        sell_option,
                        buy_option,
                        bid_or_ask,
                        &chain.data_timestamp,
                        &chain.dividend_info,
                        chain.short_fee,
                    );
                    if conversion.projected_net_profit > 0.0 {
                        match &chain.dividend_info {
                            Some(divi) => {
                                if divi.days_to_ex_date() > conversion.dte && !divi.poisoned {
                                    output_vec.push(conversion)
                                }
                            }
                            None => output_vec.push(conversion),
                        }
                    }
                }
            }
        }
    }
    let mut write = output.write().unwrap();
    write.append(&mut output_vec);
}
