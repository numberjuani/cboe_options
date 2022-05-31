use crate::{
    credentials::EOD_API_TOKEN,
    models::{
        DividendInformation, DividendRW, InsiderTransaction, OptionRW, ServerResponse, TradesRW,
    },
    others::{create_json_file, delete_file, open_json},
    trades::OptionTrade,
    TRADES_TO_INCLUDE,
};
use reqwest::Response;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
//AUTH
pub async fn get_auth() -> Result<String, reqwest::Error> {
    match open_json("cboe_auth.json") {
        Ok(auth_file) => {
            if chrono::Local::now().timestamp() >= auth_file["expiry_time"].as_i64().unwrap() {
                delete_file("cboe_auth.json");
                request_token().await
            } else {
                Ok(auth_file["access_token"].as_str().unwrap().to_string())
            }
        }
        Err(_) => request_token().await,
    }
}

async fn request_token() -> Result<String, reqwest::Error> {
    let start_time = tokio::time::Instant::now();
    let response = reqwest::Client::new()
        .post("https://id.livevol.com/connect/token")
        .basic_auth(
            crate::credentials::USERNAME,
            Some(crate::credentials::PASSWORD),
        )
        .body("grant_type=client_credentials".to_string())
        .send()
        .await?
        .json::<Value>()
        .await?;
    let mut as_object = response.as_object().unwrap().to_owned();
    let expiry_time = (chrono::Local::now()
        + chrono::Duration::seconds(response["expires_in"].as_i64().unwrap()))
    .timestamp();
    as_object.insert("expiry_time".to_string(), json!(expiry_time));
    create_json_file("cboe_auth", &as_object);
    println!(
        "Obtained Auth Token in {} ms.",
        start_time.elapsed().as_millis()
    );
    Ok(as_object["access_token"].as_str().unwrap().to_string())
}

// OPTIONS

async fn get_options(symbol: &str, token: &str) -> Result<reqwest::Response, reqwest::Error> {
    let date = chrono::Local::now().date().format("%F").to_string();
    let mut query = vec![("symbol", symbol), ("date", &date)];
    if !symbol.contains('^') {
        query.push(("root", symbol))
    }
    Ok(reqwest::Client::new()
        .get("https://api.livevol.com/v1/live/allaccess/market/option-and-underlying-quotes")
        .bearer_auth(token)
        .query(&query)
        .send()
        .await?)
}

pub async fn get_options_mt(data: OptionRW) {
    let reader = data.read().unwrap();
    let token = reader.cboe_token.clone();
    let symbol = reader.symbol.clone();
    drop(reader);
    if let Ok(response) = get_options(&symbol, &token).await {
        println!(
            "Options Request {} HTTP Status: {}, CBOE Request points used {}",
            symbol,
            &response.status(),
            &response.headers()["x-monthly-points-used"]
                .to_str()
                .unwrap()
                .parse::<i64>()
                .unwrap()
        );
        if let Ok(text) = response.text().await {
            let parsed_try: Result<ServerResponse, serde_json::Error> = serde_json::from_str(&text);
            match parsed_try {
                Ok(parsed) => {
                    if data.try_write().is_err() {
                        println!("no write on options")
                    }
                    let mut writer = data.write().unwrap();
                    writer.options = parsed;
                }
                Err(e) => {
                    println!("{} {} {}", symbol, e, text);
                }
            }
        }
    } else {
        println!("Could not get options for {}", symbol)
    }
}

// TRADES
pub async fn get_trades_mt(data: TradesRW) {
    let read = data.read().unwrap();
    let symbol = read.symbol.clone();
    let token = read.token.clone();
    drop(read);
    match get_trades(&symbol, &token).await {
        Ok(response) => {
            println!(
                "Trades Request {} HTTP Status: {}, CBOE Request points used {}",
                symbol,
                &response.status(),
                &response.headers()["x-monthly-points-used"]
                    .to_str()
                    .unwrap()
                    .parse::<i64>()
                    .unwrap()
            );
            match response.text().await {
                Ok(text) => {
                    let parsed: Result<Vec<OptionTrade>, serde_json::Error> =
                        serde_json::from_str(&text);
                    match parsed {
                        Ok(raw_trades) => {
                            if data.try_write().is_err() {
                                println!("no write on trades")
                            }
                            let mut writer = data.write().unwrap();
                            writer.trades = raw_trades;
                        }
                        Err(e) => println!("error {} parsing {}", e, text),
                    }
                }
                Err(e) => println!("no text on response {}", e),
            }
        }
        Err(e) => println!("could not obtain trades: {:#?}", e),
    }
}

pub async fn get_trades(symbol: &str, token: &str) -> Result<Response, reqwest::Error> {
    let query = vec![
        ("symbol", symbol),
        ("order_by", "SIZE_DESC"),
        ("limit", TRADES_TO_INCLUDE),
    ];
    Ok(reqwest::Client::new()
        .get("https://api.livevol.com/v1/live/allaccess/market/all-option-trades")
        .bearer_auth(token)
        .query(&query)
        .send()
        .await?)
}
// DIVIDENDS
async fn get_dividend_info(symbol: &str) -> Result<Value, reqwest::Error> {
    let today_date = chrono::Local::now().naive_local().date();
    let one_year_ago = (today_date - chrono::Duration::days(365))
        .format("%F")
        .to_string();
    Ok(reqwest::Client::new()
        .get(format!(
            "https://eodhistoricaldata.com/api/div/{}.US",
            symbol
        ))
        .query(&[
            ("api_token", EOD_API_TOKEN),
            ("fmt", "json"),
            ("from", &one_year_ago),
        ])
        .send()
        .await?
        .json::<Value>()
        .await?)
}

pub async fn get_dividend_info_mt(data: DividendRW) {
    let reader = data.read().unwrap();
    if reader.symbol.contains('^') {
        return;
    }
    let symbol = reader.symbol.clone();
    drop(reader);
    if let Ok(all_divs) = get_dividend_info(&symbol).await {
        if data.try_write().is_err() {
            println!("no write on dividends")
        }
        let mut write = data.write().unwrap();
        let parsed: Result<Vec<DividendInformation>, serde_json::Error> =
            serde_json::from_value(all_divs.clone());
        match parsed {
            Ok(good) => {
                let mut upcoming: Vec<&DividendInformation> = good
                    .iter()
                    .filter(|item| item.days_to_ex_date() > 0)
                    .collect();
                if !upcoming.is_empty() {
                    upcoming.sort_unstable_by_key(|item| item.days_to_ex_date());
                    let div = upcoming[0].clone();
                    write.dividends = Some(div);
                } else {
                    match good.last() {
                        Some(latest_div) => {
                            let projection = latest_div.clone().estimate_next_date();
                            write.dividends = Some(projection);
                        }
                        None => {
                            write.dividends = None;
                        }
                    }
                }
            }
            Err(e) => {
                println!(
                    "Error parsing this data: {}\n{}",
                    serde_json::to_string_pretty(&all_divs).unwrap(),
                    e
                );
                write.dividends = Some(DividendInformation::mark_poisoned());
            }
        };
    } else {
        println!("Could not obtain dividend info for {}", symbol)
    };
}

pub async fn get_insider_transactions(
    symbol: &str,
) -> Result<Vec<InsiderTransaction>, reqwest::Error> {
    let today_date = chrono::Local::now().naive_local().date();
    let thirty_days_ago = (today_date - chrono::Duration::days(30))
        .format("%F")
        .to_string();
    Ok(reqwest::Client::new()
        .get("https://eodhistoricaldata.com/api/insider-transactions")
        .query(&[
            ("api_token", EOD_API_TOKEN),
            ("from", &thirty_days_ago),
            ("code", symbol),
        ])
        .send()
        .await?
        .json::<Vec<InsiderTransaction>>()
        .await?)
}

pub async fn get_insider_data_mt(data: Arc<RwLock<(&str, f64)>>) {
    let read = data.read().unwrap();
    let symbol = read.0;
    drop(read);
    if symbol.contains('^') {
        return;
    }
    if let Ok(transactions) = get_insider_transactions(symbol).await {
        let net_result: f64 = transactions
            .iter()
            .map(|transaction| transaction.net_result())
            .sum();
        if data.try_write().is_err() {
            println!("no write on insiders")
        }
        let mut write = data.write().unwrap();
        write.1 = net_result;
    } else {
        println!("Could not obtain insider transaction data")
    }
}

pub async fn get_short_ratio(symbol: &str) -> Result<f64, reqwest::Error> {
    let value = reqwest::Client::new()
        .get(format!(
            "https://eodhistoricaldata.com/api/fundamentals/{}.US",
            symbol
        ))
        .query(&[
            ("api_token", EOD_API_TOKEN),
            ("filter", "SharesStats"),
            ("fmt", "json"),
        ])
        .send()
        .await?
        .json::<Value>()
        .await?;
    match value["ShortPercentFloat"].as_f64() {
        Some(good) => Ok(100.0 * good),
        None => Ok(0.0),
    }
}

pub async fn get_short_ratio_mt(data: Arc<RwLock<(&str, f64)>>) {
    let read = data.read().unwrap();
    let symbol = read.0;
    drop(read);
    if let Ok(ratio) = get_short_ratio(symbol).await {
        if data.try_write().is_err() {
            println!("no write on short ratio")
        }
        let mut write = data.write().unwrap();
        write.1 = ratio;
    }
}
