use crate::{
    models::{ShortStockData, ShortStockInfo},
    SHORT_STOCK_DATA_FP,
};
use async_ftp::FtpStream;
use chrono::DateTime;
use chrono_tz::Tz;
use serde_json::Value;
use std::str;

pub fn create_csv_file<T: serde::Serialize>(data: &[T], filename: &str) {
    let filename = format!(
        "{}-{}.csv",
        filename,
        chrono::Local::now().format("%F-%H%M")
    );
    if !data.is_empty() {
        let mut writer = csv::Writer::from_path(filename).unwrap();
        for line in data {
            writer.serialize(line).unwrap();
        }
    }
}

pub fn round_to_decimals(float: f64, num_decimals: u32) -> f64 {
    let multiplied = float * (10i64.pow(num_decimals) as f64);
    let rounded = multiplied.round();
    rounded / (10i64.pow(num_decimals) as f64)
}
/*
pub fn get_user_input(prompt: &str) -> String {
    let mut answer = String::new();
    println!("{}", prompt);
    std::io::stdin()
        .read_line(&mut answer)
        .expect("unable to read user input");
    answer.trim().to_string()
}
*/

pub fn get_list(filename: &str) -> Result<Vec<String>, std::io::Error> {
    let mut output: Vec<String> = Vec::new();
    let file = std::fs::read_to_string(filename)?;
    let lines = file.lines();
    for line in lines {
        output.push(line.to_string())
    }
    println!("Found {} symbols in list", output.len());
    Ok(output)
}

pub fn get_new_york_time() -> DateTime<Tz> {
    chrono::Utc::now().with_timezone(&chrono_tz::America::New_York)
    //
}
/*
pub fn get_margin_loan_cost(principal: f64, duration: i64) -> f64 {
    round_to_decimals(
        ((MARGIN_LOAN_RATE / 365.0) / 100.0) * (duration as f64) * (principal),
        2,
    )
}

pub fn get_short_fee_cost(rate: f64, stock_price: f64, duration: i64) -> f64 {
    let rounded_price = round_up(stock_price);
    let buffered_principal = SHORT_FEE_MARGIN_SAFETY*100.0 * rounded_price;
    let rate_multiplier = rate/100.0;
    let yearly_fee = buffered_principal*rate_multiplier;
    let daily_fee = yearly_fee/360.0;
    round_up(daily_fee*duration as f64)
}

pub fn round_up(num: f64) -> f64{
    if num.round() < num {
        num.round()+1.0
    } else {num.round()}
}
*/

impl ShortStockInfo {
    pub async fn get() -> Self {
        let start = tokio::time::Instant::now();
        println!("Getting short trade fees and availability data");
        let mut data: Vec<ShortStockData> = Vec::new();
        let raw_file = get_file().await;
        let lines: Vec<&str> = raw_file.lines().collect();
        let first_line: Vec<&str> = lines[0].split('|').collect();
        let date = first_line[1].to_string();
        let time = first_line[2].to_string();
        for line in lines {
            let split: Vec<&str> = line.split('|').collect();
            if split.len() == 9 && split[0] != "#SYM" {
                let parsed_line = ShortStockData {
                    symbol: split[0].to_string(),
                    currency: split[1].to_string(),
                    name: split[2].to_string(),
                    con: split[3].to_string(),
                    isin: split[4].to_string(),
                    rebate_rate: split[5].to_string(),
                    fee_rate: split[6].to_string(),
                    available: split[7].to_string(),
                };
                data.push(parsed_line)
            }
        }
        println!(
            "Obtained short fee data for {} stocks in {} seconds.",
            data.len(),
            start.elapsed().as_secs()
        );
        Self { date, time, data }
    }
}

pub async fn get_file() -> String {
    let mut ftp_stream = FtpStream::connect((SHORT_STOCK_DATA_FP, 21)).await.unwrap();
    ftp_stream.login("shortstock", "").await.unwrap();
    let remote_file = ftp_stream.simple_retr("usa.txt").await.unwrap();
    let file = str::from_utf8(&remote_file.into_inner())
        .unwrap()
        .to_string();
    ftp_stream.quit().await.unwrap();
    file
}

pub fn open_json(filename: &str) -> Result<Value, std::io::Error> {
    let file = std::fs::File::open(filename)?;
    let reader = std::io::BufReader::new(file);
    let data_file: Value = serde_json::from_reader(reader).unwrap();
    Ok(data_file)
}

pub fn create_json_file<T: serde::Serialize>(filename: &str, contents: &T) {
    let filename = format!("{}.json", filename);
    serde_json::to_writer(&std::fs::File::create(filename).unwrap(), contents).unwrap();
}

pub fn delete_file(filename: &str) {
    std::fs::remove_file(filename).unwrap()
}
