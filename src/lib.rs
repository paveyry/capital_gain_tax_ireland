use std::{collections::HashMap, path::Path};

use anyhow::{Context, Error};
use calamine::{open_workbook, Data, DataType, Reader, Xlsx};
use time::{format_description::BorrowedFormatItem, macros::format_description, Date};

pub type Result<T> = std::result::Result<T, Error>;

const FROM_CURRENCY: &str = "USD";
const TO_CURRENCY: &str = "EUR";
const TAX_RATE: f64 = 0.33;
const EXEMPTION_EUR: f64 = 1270.0;

static XLSX_DATE_FMT: &[BorrowedFormatItem] = format_description!("[month]/[day]/[year]");
static EXR_API_DATE_FMT: &[BorrowedFormatItem] = format_description!("[year]-[month]-[day]");

#[derive(Debug, Clone)]
pub struct Transaction {
    sell_date: Date,
    usd_gain: f64,
    usd_loss: f64,
    eur_gain: f64,
    eur_loss: f64,
    exr: f64,
}

#[derive(Debug, Default)]
pub struct Totals {
    pub usd_gain: f64,
    pub usd_loss: f64,
    pub usd_net_gain: f64,
    pub eur_gain: f64,
    pub eur_loss: f64,
    pub eur_net_gain: f64,
    pub eur_taxable_gain: f64,
    pub tax: f64,
}

#[derive(Debug, Default)]
struct ExchangeRateCache {
    cache: HashMap<Date, f64>,
}

impl ExchangeRateCache {
    fn new() -> Self {
        Self::default()
    }

    fn get_exr(&mut self, date: Date) -> Result<f64> {
        if let Some(exr) = self.cache.get(&date) {
            return Ok(*exr);
        }
        let date_str = date.format(EXR_API_DATE_FMT)?;
        let r = reqwest::blocking::get(format!(
            "https://data-api.ecb.europa.eu/service/data/EXR/D.{}.{}.SP00.A?detail=dataonly&startPeriod={}&endPeriod={}&format=csvdata",
            FROM_CURRENCY, TO_CURRENCY, date_str, date_str))?;
        let mut rdr = csv::Reader::from_reader(r);
        let index = rdr
            .headers()?
            .iter()
            .position(|h| h.trim() == "OBS_VALUE")
            .context("failed to find EXR header")?;
        let exr = &rdr
            .records()
            .next()
            .context("missing entry from EXR CSV")??[index];
        let exr = exr
            .parse::<f64>()
            .context("EXR field is not a valid float")?;
        self.cache.insert(date, exr);
        Ok(exr)
    }
}

fn get_column_indices(headers: Vec<String>) -> Result<(usize, usize, usize)> {
    let mut date_index: Option<usize> = None;
    let mut gain_loss_index: Option<usize> = None;
    let mut record_type_index: Option<usize> = None;
    headers
        .iter()
        .enumerate()
        .for_each(|(pos, h)| match h.trim() {
            "Date Sold" => date_index = Some(pos),
            "Adjusted Gain/Loss" => gain_loss_index = Some(pos),
            "Record Type" => record_type_index = Some(pos),
            _ => {}
        });
    Ok((
        date_index.context("failed to find date header")?,
        gain_loss_index.context("failed to find gain/loss header")?,
        record_type_index.context("failed to find record type header")?,
    ))
}

pub fn get_transactions<P: AsRef<Path>>(file_path: P) -> Result<Vec<Transaction>> {
    let mut spreadsheet: Xlsx<_> = open_workbook(file_path)?;
    let Ok(range) = spreadsheet.worksheet_range("G&L_Expanded") else {
        return Err(Error::msg("missing sheet"));
    };
    let headers = range.headers().context("failed to extract headers")?;
    let (date_index, gain_loss_index, record_type_index) = get_column_indices(headers)?;

    let mut exr_cache = ExchangeRateCache::new();
    let mut transactions = Vec::new();
    for r in range.rows().skip(1) {
        if r[record_type_index] != Data::String("Sell".to_string()) {
            continue;
        }
        let sell_date = Date::parse(
            r[date_index]
                .as_string()
                .context("wrong date field type")?
                .as_str(),
            &XLSX_DATE_FMT,
        )?;
        let gain_loss = r[gain_loss_index]
            .as_f64()
            .context("wrong gain/loss field type")?;
        let (usd_gain, usd_loss) = if gain_loss >= 0. {
            (gain_loss, 0.)
        } else {
            (0., -gain_loss)
        };
        let exr = exr_cache
            .get_exr(sell_date)
            .context("failed to retrieve exchange rate")?;
        transactions.push(Transaction {
            sell_date,
            usd_gain,
            usd_loss,
            eur_gain: usd_gain / exr,
            eur_loss: usd_loss / exr,
            exr,
        });
    }
    Ok(transactions)
}

fn compute_report(transactions: &[Transaction]) -> Totals {
    let mut totals = Totals::default();
    transactions.iter().for_each(|t| {
        totals.usd_gain += t.usd_gain;
        totals.usd_loss += t.usd_loss;
        totals.eur_gain += t.eur_gain;
        totals.eur_loss += t.eur_loss;
    });
    totals.usd_net_gain = totals.usd_gain - totals.usd_loss;
    totals.eur_net_gain = totals.eur_gain - totals.eur_loss;
    totals.eur_taxable_gain = totals.eur_net_gain - EXEMPTION_EUR;
    if totals.eur_taxable_gain < 0. {
        totals.eur_taxable_gain = 0.;
    }
    totals.tax = totals.eur_taxable_gain * TAX_RATE;
    totals
}

pub fn write_detail_as_csv<P: AsRef<Path>>(
    transactions: &[Transaction],
    file_path: P,
) -> Result<()> {
    let mut wtr = csv::Writer::from_path(&file_path)?;
    wtr.write_record([
        "Sell Date",
        "USD Gain",
        "USD Loss",
        "EUR Gain",
        "EUR Loss",
        "EXR",
    ])?;
    for t in transactions {
        wtr.write_record(&[
            t.sell_date.format(EXR_API_DATE_FMT)?,
            t.usd_gain.to_string(),
            t.usd_loss.to_string(),
            t.eur_gain.to_string(),
            t.eur_loss.to_string(),
            t.exr.to_string(),
        ])?;
    }
    println!(
        "The transaction detail was written as CSV to file {}",
        file_path.as_ref().to_string_lossy()
    );
    Ok(())
}

pub fn compute_and_print_report(transactions: &[Transaction]) {
    let totals = compute_report(transactions);
    println!("\nTotal gain (USD): ${:.2}", totals.usd_gain);
    println!("Total loss (USD): ${:.2}", totals.usd_loss);
    println!("Net gain (USD): ${:.2}\n", totals.usd_net_gain);
    println!("Total gain: €{:.2}", totals.eur_gain);
    println!("Total loss: €{:.2}", totals.eur_loss);
    println!("Net gain: €{:.2}", totals.eur_net_gain);
    println!(
        "Taxable gain (amount above exemption): €{:.2}",
        totals.eur_taxable_gain
    );
    println!(
        "Tax to pay ({:.2}%): €{}",
        TAX_RATE * 100.,
        totals.eur_taxable_gain
    );
}
