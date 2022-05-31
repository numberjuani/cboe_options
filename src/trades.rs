use crate::others::round_to_decimals;
use crate::single_options::OptionData;
use crate::single_options::OptionType;
use crate::spreads::OptionSpread;
use crate::spreads::SpreadName;
use crate::spreads::SpreadType;
use serde::Deserialize;
use serde::Serialize;
use serde_repr::Deserialize_repr;
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OptionTrade {
    pub root: String,
    #[serde(default = "to_be_calculated_order_action")]
    pub order_action: OrderAction,
    pub option_trade_size: i64,
    pub strike: f64,
    pub expiry: String,
    #[serde(default = "to_be_calculated_int")]
    pub dte: i64,
    pub option_type: OptionType,
    #[serde(default = "to_be_calculated_float")]
    pub notional_value: f64,
    #[serde(default = "to_be_calculated_transaction_type")]
    pub transaction_estimate: TransactionType,
    pub option_trade_price: Option<f64>,
    #[serde(default = "to_be_calculated_float")]
    pub iv: f64,
    pub cancel_flag: i64,
    pub condition_id: ConditionID,
    #[serde(default = "to_be_calculated_float")]
    pub delta: f64,
    pub exchange_id: Exchange,
    #[serde(skip_serializing)]
    #[serde(default = "to_be_calculated_float")]
    pub implied_underlying_ask: f64,
    #[serde(skip_serializing)]
    #[serde(default = "to_be_calculated_int")]
    pub implied_underlying_ask_size: i64,
    #[serde(skip_serializing)]
    #[serde(default = "to_be_calculated_float")]
    pub implied_underlying_bid: f64,
    #[serde(skip_serializing)]
    #[serde(default = "to_be_calculated_int")]
    pub implied_underlying_bid_size: i64,
    #[serde(skip_serializing)]
    pub implied_underlying_indicator: Option<String>,
    #[serde(default = "to_be_calculated_float")]
    pub implied_underlying_mid: f64,
    #[serde(rename = "option")]
    #[serde(default = "to_be_calculated_string")]
    pub symbol: String,
    #[serde(rename = "option_bid")]
    pub bid_price: Option<f64>,
    #[serde(rename = "option_ask")]
    pub ask_price: Option<f64>,
    #[serde(skip_serializing)]
    pub option_ask_size: Option<i64>,
    #[serde(skip_serializing)]
    pub option_bid_size: Option<i64>,
    pub option_trade_at: OptionTradeAt,
    //chrono::NaiveTime::parse_from_str(&trade.timestamp, "%H:%M:%S.%3f").unwrap()
    pub timestamp: String,
    pub seq_no: i64,
    pub exchange_seq_no: i64,
    #[serde(skip_serializing)]
    pub underlying_ask: Option<f64>,
    #[serde(skip_serializing)]
    pub underlying_bid: Option<f64>,
    #[serde(default = "to_be_calculated_trade_price")]
    pub execution_price: ExecutionPrice,
    #[serde(default = "to_be_calculated_string")]
    pub symbol_date: String,
    #[serde(default = "Expectation::new")]
    pub expectation: Expectation,
    #[serde(default = "to_be_calculated_string")]
    pub description: String,
    #[serde(default = "to_be_calculated_float")]
    pub current_delta: f64,
}
impl OptionTrade {
    pub fn get_values(self, symbol_date: &str) -> Self {
        let mid_point = 0.5 * (self.bid_price.unwrap_or(0.0) + self.ask_price.unwrap_or(0.0));
        let price = self.option_trade_price.unwrap_or(0.0);
        let execution_price = match self.option_trade_at {
            OptionTradeAt::AboveAsk => ExecutionPrice::CloserToAsk,
            OptionTradeAt::OnAsk => ExecutionPrice::CloserToAsk,
            OptionTradeAt::OnBid => ExecutionPrice::CloserToBid,
            OptionTradeAt::BelowBid => ExecutionPrice::CloserToBid,
            _ => {
                if price < mid_point && price > 0.0 {
                    ExecutionPrice::CloserToBid
                } else if price == mid_point {
                    ExecutionPrice::ExactMidPrice
                } else {
                    ExecutionPrice::CloserToAsk
                }
            }
        };
        let notional_value = round_to_decimals(
            self.option_trade_price.unwrap_or(0.0) * 100.0 * (self.option_trade_size as f64),
            2,
        );
        let dte = (chrono::NaiveDate::parse_from_str(&self.expiry, "%F").unwrap()
            - chrono::Local::now().naive_local().date())
        .num_days();
        let mut order_action = match execution_price {
            ExecutionPrice::CloserToBid => OrderAction::Sold,
            ExecutionPrice::CloserToAsk => OrderAction::Bought,
            ExecutionPrice::ExactMidPrice => OrderAction::Unknown,
            ExecutionPrice::Unknown => OrderAction::Unknown,
        };
        if self.option_trade_at == OptionTradeAt::CrossedMarket {
            order_action = order_action.flip()
        }
        let expectation = match self.option_type {
            OptionType::Call => match order_action {
                OrderAction::Bought => Expectation::Bullish,
                OrderAction::Sold => Expectation::Bearish,
                OrderAction::Unknown => Expectation::Unknown,
            },
            OptionType::Put => match order_action {
                OrderAction::Bought => Expectation::Bearish,
                OrderAction::Sold => Expectation::Bullish,
                OrderAction::Unknown => Expectation::Unknown,
            },
        };
        // date is in yyyy-MM-dd format
        //let split_date = date.split('-').collect_vec();
        //symbol date format is SPY-10-20-2021
        //let symbol_date = format!("{}-{}-{}-{}",self.root,split_date[1],split_date[2],split_date[0]);
        Self {
            notional_value,
            execution_price,
            order_action,
            symbol_date: symbol_date.to_string(),
            expectation,
            dte,
            delta: self.delta * 100.0,
            ..self
        }
    }
    pub fn dealer_delta(&self) -> f64 {
        match self.transaction_estimate {
            TransactionType::BuyToOpen => -self.option_trade_size as f64 * self.current_delta,
            TransactionType::SellToOpen => -self.option_trade_size as f64 * self.current_delta,
            _ => 0.0,
        }
    }
    pub fn naive_dealer_delta(&self) -> f64 {
        match self.order_action {
            OrderAction::Unknown => 0.0,
            _ => -self.option_trade_size as f64 * self.delta,
        }
    }
    pub fn is_opening(&self) -> bool {
        match self.transaction_estimate {
            TransactionType::BuyToOpen => true,
            TransactionType::SellToOpen => true,
            TransactionType::MaybeBuyToClose => false,
            TransactionType::MaybeSellToClose => false,
            TransactionType::CouldNotDetermine => false,
            TransactionType::Uncalculated => false,
        }
    }
    /*
        pub fn make_opening_transaction(self) -> Self {
        let new_transaction_estimate = match self.execution_price {
            ExecutionPrice::CloserToBid => TransactionType::SellToOpen,
            ExecutionPrice::CloserToAsk => TransactionType::BuyToOpen,
            ExecutionPrice::ExactMidPrice => TransactionType::Uncalculated,
            ExecutionPrice::Unknown => TransactionType::Uncalculated,
        };
        Self {
            transaction_estimate: new_transaction_estimate,
            ..self
        }
    }
    */

    pub fn to_spread(self) -> OptionSpread {
        let spread_name = match self.option_type {
            OptionType::Call => match self.order_action {
                OrderAction::Bought => SpreadName::LongCall,
                OrderAction::Sold => SpreadName::ShortCall,
                OrderAction::Unknown => SpreadName::Unrecognized,
            },
            OptionType::Put => match self.order_action {
                OrderAction::Bought => SpreadName::LongPut,
                OrderAction::Sold => SpreadName::ShortPut,
                OrderAction::Unknown => SpreadName::Unrecognized,
            },
        };
        OptionSpread {
            symbol: self.root.clone(),
            spread_name,
            spread_type: if self.order_action == OrderAction::Bought {
                SpreadType::Debit
            } else if self.order_action == OrderAction::Sold {
                SpreadType::Credit
            } else {
                SpreadType::Unknown
            },
            net_value: self.notional_value,
            expiration_date: self.expiry.clone(),
            dte: self.dte,
            net_iv: self.net_iv(),
            expectation: self.expectation,
            timestamp: self.timestamp.clone(),
            condition_id: self.condition_id,
            exchange: self.exchange_id,
            leg_number: 1,
            summary: format!(
                "{:#?} {} of the {} {} {:#?}|",
                self.order_action,
                self.option_trade_size,
                self.strike,
                self.expiry,
                self.option_type
            ),
            opening_trade: if self.is_opening() { true } else { false },
            sequence_numbers: format!("seq no {}- ex seq no {}", self.seq_no, self.exchange_seq_no),
            delta_when_opened: self.net_delta(),
            current_delta: self.net_current_delta(),
        }
    }
    pub fn amount_paid(&self) -> f64 {
        match self.order_action {
            OrderAction::Bought => self.notional_value,
            OrderAction::Sold => -self.notional_value,
            OrderAction::Unknown => 0.0,
        }
    }
    pub fn net_iv(&self) -> f64 {
        let iv = match self.order_action {
            OrderAction::Bought => self.iv,
            OrderAction::Sold => -self.iv,
            OrderAction::Unknown => 0.0,
        };
        iv
    }
    pub fn net_delta(&self) -> f64 {
        let delta = match self.order_action {
            OrderAction::Bought => self.delta,
            OrderAction::Sold => -self.delta,
            OrderAction::Unknown => 0.0,
        };
        delta * self.option_trade_size as f64
    }
    pub fn net_current_delta(&self) -> f64 {
        let delta = match self.order_action {
            OrderAction::Bought => self.current_delta,
            OrderAction::Sold => -self.current_delta,
            OrderAction::Unknown => 0.0,
        };
        delta * self.option_trade_size as f64
    }
    pub fn is_call(&self) -> bool {
        self.option_type == OptionType::Call
    }
    pub fn is_put(&self) -> bool {
        self.option_type == OptionType::Put
    }
    pub fn is_buy(&self) -> bool {
        self.order_action == OrderAction::Bought
    }
    pub fn is_sell(&self) -> bool {
        self.order_action == OrderAction::Sold
    }
    pub fn is_call_buy(&self) -> bool {
        self.is_buy() && self.is_call()
    }
    pub fn is_put_buy(&self) -> bool {
        self.is_buy() && self.is_put()
    }
    pub fn is_call_sell(&self) -> bool {
        self.is_sell() && self.is_call()
    }
    pub fn is_put_sell(&self) -> bool {
        self.is_put() && self.is_sell()
    }
    /*
    pub fn was_otm(&self) -> bool {
    match self.option_type {
        OptionType::Call => self.strike > self.implied_underlying_mid,
        OptionType::Put => self.strike < self.implied_underlying_mid,
    }
    }
    */
}

pub fn to_be_calculated_order_action() -> OrderAction {
    OrderAction::Unknown
}

pub fn to_be_calculated_string() -> String {
    String::new()
}

pub fn to_be_calculated_float() -> f64 {
    0.0
}

pub fn to_be_calculated_int() -> i64 {
    0
}

pub fn to_be_calculated_trade_price() -> ExecutionPrice {
    ExecutionPrice::Unknown
}

pub fn to_be_calculated_transaction_type() -> TransactionType {
    TransactionType::Uncalculated
}
#[derive(Debug, Serialize, PartialEq, PartialOrd, Clone, Deserialize, Copy)]
pub enum TransactionType {
    BuyToOpen,
    SellToOpen,
    MaybeBuyToClose,
    MaybeSellToClose,
    CouldNotDetermine,
    Uncalculated,
}

impl TransactionType {
    pub fn is_opening(&self) -> bool {
        self == &TransactionType::BuyToOpen || self == &TransactionType::SellToOpen
    }
}
#[derive(Debug, Serialize, PartialEq, PartialOrd, Clone, Deserialize)]
pub enum ExecutionPrice {
    CloserToBid,
    CloserToAsk,
    ExactMidPrice,
    Unknown,
}
#[derive(Debug, Serialize, Clone, Copy, PartialEq, PartialOrd, Deserialize)]
pub enum OrderAction {
    Bought,
    Sold,
    Unknown,
}
impl OrderAction {
    pub fn flip(&self) -> Self {
        match self {
            OrderAction::Bought => OrderAction::Sold,
            OrderAction::Sold => OrderAction::Bought,
            OrderAction::Unknown => OrderAction::Unknown,
        }
    }
}
pub fn estimate_transaction(option: &OptionData, trade: &OptionTrade) -> TransactionType {
    if option.open_interest < trade.option_trade_size {
        match trade.execution_price {
            ExecutionPrice::CloserToBid => TransactionType::SellToOpen,
            ExecutionPrice::CloserToAsk => TransactionType::BuyToOpen,
            ExecutionPrice::ExactMidPrice => TransactionType::CouldNotDetermine,
            ExecutionPrice::Unknown => TransactionType::CouldNotDetermine,
        }
    } else {
        match trade.execution_price {
            ExecutionPrice::CloserToBid => TransactionType::MaybeSellToClose,
            ExecutionPrice::CloserToAsk => TransactionType::MaybeBuyToClose,
            ExecutionPrice::ExactMidPrice => TransactionType::CouldNotDetermine,
            ExecutionPrice::Unknown => TransactionType::CouldNotDetermine,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Copy)]
pub enum OptionTradeAt {
    #[serde(rename = "Above Ask")]
    AboveAsk,
    #[serde(rename = "On Ask")]
    OnAsk,
    #[serde(rename = "Mid Market")]
    MidMarket,
    #[serde(rename = "On Bid")]
    OnBid,
    #[serde(rename = "Below Bid")]
    BelowBid,
    #[serde(rename = "Crossed Market")]
    CrossedMarket,
    #[serde(rename = "No Market")]
    NoMarket,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Copy)]
pub enum Expectation {
    Bullish,
    Bearish,
    Neutral,
    Unknown,
}
impl Expectation {
    pub fn new() -> Self {
        Expectation::Unknown
    }
}

#[derive(Serialize, Deserialize_repr, PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum ConditionID {
    Regular = 0,
    #[serde(rename(serialize = "Form T"))]
    FormT = 1,
    #[serde(rename(serialize = "Out Of Sequence"))]
    OutOfSeq = 2,
    AvgPrc = 3,
    OpenReportLate = 5,
    OpenReportOutOfSeq = 6,
    OpenReportInSeq = 7,
    PriorReferencePrice = 8,
    NextDaySale = 9,
    Bunched = 10,
    CashSale = 11,
    Seller = 12,
    #[serde(rename(serialize = "Sold Last"))]
    SoldLast = 13,
    Rule127 = 14,
    BunchedSold = 15,
    #[serde(rename(serialize = "Single Leg Automated Execution"))]
    AutoExecution = 18,
    Reopen = 21,
    Acquisition = 22,
    Rule155 = 29,
    Distribution = 30,
    Split = 31,
    AdjTerms = 34,
    Spread = 35,
    Straddle = 36,
    BuyWrite = 37,
    Combo = 38,
    STPD = 39,
    CANC = 40,
    CANCLAST = 41,
    CANCOPEN = 42,
    CANCONLY = 43,
    CANCSTPD = 44,
    MatchCross = 45,
    InternalCross = 54,
    StoppedRegular = 55,
    StoppedSoldLast = 56,
    StoppedOutOfSeq = 57,
    OpenReport = 62,
    MarketOnClose = 63,
    OutOfSeqPreMkt = 65,
    MCOfficialOpen = 66,
    YellowFlag = 79,
    PreOpening = 89,
    #[serde(rename(serialize = "Inter Market Sweep"))]
    IntermarketSweep = 95,
    Derivative = 96,
    Reopening = 97,
    Closing = 98,
    OddLotTrade = 99,
    PriceVariation = 104,
    Contingent = 105,
    StoppedIM = 106,
    Benchmark = 107,
    TradeThroughExempt = 108,
    TradeCorrection = 111,
    Block = 112,
    ECRP = 113,
    #[serde(rename(serialize = "Single Leg Auction non Sweep Order"))]
    SingLegAuctNonISO = 114,
    #[serde(rename(serialize = "Single Leg Auction Sweep Order"))]
    SingLegAuctISO = 115,
    #[serde(rename(serialize = "Single Leg Cross non Sweep Order"))]
    SingLegCrossNonISO = 116,
    #[serde(rename(serialize = "Single Leg Cross Sweep Order"))]
    SingLegCrossISO = 117,
    #[serde(rename(serialize = "Single Leg Floor Trade"))]
    SingLegFlr = 118,
    #[serde(rename(serialize = "Multi Leg Algorithmic Execution"))]
    MultLegAutoEx = 119,
    #[serde(rename(serialize = "Multi Leg Auction"))]
    MultLegAuct = 120,
    #[serde(rename(serialize = "Multi Leg Cross"))]
    MultLegCross = 121,
    #[serde(rename(serialize = "Multi Leg Floor Trade"))]
    MultLegFlr = 122,
    #[serde(rename(serialize = "Multi Algorithmic vs Single Legs"))]
    MultLegAutoSingLeg = 123,
    #[serde(rename(serialize = "Multi Leg with Stock Auction"))]
    StkOptAuct = 124,
    #[serde(rename(serialize = "Multi Leg Auction vs Single Legs"))]
    MultLegAuctSingLeg = 125,
    #[serde(rename(serialize = "Multi Leg Floor Trade vs Single Legs"))]
    MultLegFlrSingLeg = 126,
    #[serde(rename(serialize = "Multi Leg with Stock Algorithmic Execution"))]
    StkOptAutoEx = 127,
    #[serde(rename(serialize = "Multi Leg with Stock Cross"))]
    StkOptCross = 128,
    #[serde(rename(serialize = "Multi Leg with Stock Floor Trade"))]
    StkOptFlr = 129,
    #[serde(rename(serialize = "Multi Leg with Stock Floor Trade"))]
    StkOptAutoExSingLeg = 130,
    #[serde(rename(serialize = "Multi Leg with Stock Auction vs Single Legs"))]
    StkOptAuctSingLeg = 131,
    #[serde(rename(serialize = "Multi Leg with Stock Auction Floor Trade vs Single Legs"))]
    StkOptFlrSingLeg = 132,
    #[serde(rename(serialize = "Multi Leg Floor Trade of Proprietary Products"))]
    MultLegFlrPropProd = 133,
    CorrConsClose = 134,
    QualContTrade = 135,
    MultiCompressProp = 136,
    ExtendedHours = 137,
}
impl ConditionID {
    pub fn is_multi_leg(&self) -> bool {
        use ConditionID::*;
        self == &MultLegAutoEx
            || self == &MultLegAuct
            || self == &MultLegCross
            || self == &MultLegFlr
            || self == &MultLegAutoSingLeg
            || self == &MultLegAuctSingLeg
            || self == &MultLegFlrSingLeg
            || self == &MultLegFlrPropProd
            || self == &StkOptAuct
            || self == &StkOptAutoEx
            || self == &StkOptCross
            || self == &StkOptFlr
            || self == &StkOptAutoExSingLeg
            || self == &StkOptAuctSingLeg
            || self == &StkOptFlrSingLeg
    }
    pub fn is_sweep(&self) -> bool {
        use ConditionID::*;
        self == &IntermarketSweep || self == &SingLegAuctISO || self == &SingLegCrossISO
    }
    pub fn is_cancel(&self) -> bool {
        use ConditionID::*;
        self == &CANC
            || self == &CANCLAST
            || self == &CANCONLY
            || self == &CANCOPEN
            || self == &CANCSTPD
    }
    pub fn includes_stock_trade(&self) -> bool {
        use ConditionID::*;
        self == &StkOptAuct
            || self == &StkOptAuctSingLeg
            || self == &StkOptAutoExSingLeg
            || self == &StkOptCross
            || self == &StkOptFlrSingLeg
            || self == &StkOptFlr
    }
}

#[derive(Deserialize_repr, PartialEq, Debug, Clone, Serialize, Copy)]
#[repr(u8)]
pub enum Exchange {
    #[serde(rename(serialize = "NASDAQ"))]
    NasdaqExchange = 1,
    #[serde(rename(serialize = "NASDAQ ADF"))]
    NasdaqAlternativeDisplayFacility = 2,
    #[serde(rename(serialize = "NYSE"))]
    NewYorkStockExchange = 3,
    #[serde(rename(serialize = "American Stock Exchange"))]
    AmericanStockExchange = 4,
    #[serde(rename(serialize = "CBOE"))]
    ChicagoBoardOptionsExchange = 5,
    #[serde(rename(serialize = "International Securities Exchange"))]
    InternationalSecuritiesExchange = 6,
    #[serde(rename(serialize = "NYSE ARCA"))]
    NYSEArcaExchange = 7,
    #[serde(rename(serialize = "NYSE National"))]
    NYSENational = 8,
    #[serde(rename(serialize = "Philadephia Stock Exchange"))]
    PhiladelphiaStockExchange = 9,
    #[serde(rename(serialize = "Boston Stock Exchange"))]
    BostonStockExchange = 11,
    #[serde(rename(serialize = "NASDAQ Bulletin Board"))]
    NasdaqBulletinBoard = 14,
    #[serde(rename(serialize = "NASDAQ OTC Pink Sheets"))]
    NasdaqOTCPinkSheets = 15,
    #[serde(rename(serialize = "Chicago Stock Exchange"))]
    ChicagoStockExchange = 17,
    #[serde(rename(serialize = "CME"))]
    ChicagoMercantileExchange = 20,
    #[serde(rename(serialize = "ISE Mercury"))]
    ISEMercury = 22,
    #[serde(rename(serialize = "Dow Jones Indices"))]
    DowJonesIndices = 30,
    #[serde(rename(serialize = "ISE Gemini"))]
    ISEGemini = 31,
    #[serde(rename(serialize = "C2"))]
    C2 = 42,
    #[serde(rename(serialize = "MIAX Options Exchange"))]
    MIAXOptionsExchange = 43,
    #[serde(rename(serialize = "NASDAQ OMX BX Options"))]
    NASDAQOMXBXOptions = 47,
    #[serde(rename(serialize = "CBOE Futures"))]
    CBOEFuturesExchange = 54,
    #[serde(rename(serialize = "NSX Trade Reporting"))]
    NSXTradeReportingFacility = 57,
    #[serde(rename(serialize = "NYSE Trade Reporting"))]
    NYSETradeReportingFacility = 59,
    #[serde(rename(serialize = "BATS Option & Equity"))]
    BATSTrading = 60,
    #[serde(rename(serialize = "BATS Equity"))]
    BATSTrading2 = 63,
    #[serde(rename(serialize = "Direct Edge A"))]
    DirectEdgeA = 64,
    #[serde(rename(serialize = "Direct Edge X"))]
    DirectEdgeX = 65,
    #[serde(rename(serialize = "IEX Stock Exchange"))]
    IEXStockExchange = 68,
    #[serde(rename(serialize = "MIAX Pearl"))]
    MIAXPEARL = 69,
    #[serde(rename(serialize = "MIAX Emerald Options"))]
    MIAXEmeraldOptionsExchange = 71,
    #[serde(rename(serialize = "CHI-X Europe"))]
    CHIXEurope = 115,
    #[serde(rename(serialize = "Long Term Stock Exchange"))]
    LongTermStockExchange = 117,
    #[serde(rename(serialize = "FINRA ADF"))]
    FINRAAlternativeDisplayFacility = 118,
    #[serde(rename(serialize = "FINRA NASDAQ TRF Chicago"))]
    FINRANasdaqTRFChicago = 119,
    #[serde(rename(serialize = "Members Exchange"))]
    MembersExchange = 120,
}
