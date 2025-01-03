use std::{collections::HashMap, path::Path};

use anyhow::{Context, Error};
use calamine::{open_workbook, Data, DataType, Reader, Xlsx};
use time::{format_description::BorrowedFormatItem, macros::format_description, Date, Month};

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
    usd_proceeds: f64,
    eur_proceeds: f64,
}

#[derive(Debug, Default)]
pub struct PeriodTaxReport {
    pub usd_gain: f64,
    pub usd_loss: f64,
    pub usd_net_gain: f64,
    pub eur_gain: f64,
    pub eur_loss: f64,
    pub eur_net_gain: f64,
    pub usd_proceeds: f64,
    pub eur_proceeds: f64,
}

#[derive(Debug, Default)]
pub struct TaxReport {
    pub fiscal_year: i32,
    pub period_tax_report: PeriodTaxReport,
    pub eur_taxable_gain: f64,
    pub eur_tax: f64,
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

fn get_column_indices(headers: Vec<String>) -> Result<(usize, usize, usize, usize)> {
    let mut date_index: Option<usize> = None;
    let mut gain_loss_index: Option<usize> = None;
    let mut record_type_index: Option<usize> = None;
    let mut total_proceeds_index: Option<usize> = None;
    headers
        .iter()
        .enumerate()
        .for_each(|(pos, h)| match h.trim() {
            "Date Sold" => date_index = Some(pos),
            "Adjusted Gain/Loss" => gain_loss_index = Some(pos),
            "Record Type" => record_type_index = Some(pos),
            "Total Proceeds" => total_proceeds_index = Some(pos),
            _ => {}
        });
    Ok((
        date_index.context("failed to find date header")?,
        gain_loss_index.context("failed to find gain/loss header")?,
        record_type_index.context("failed to find record type header")?,
        total_proceeds_index.context("failed to find total proceeds header")?,
    ))
}

pub fn get_transactions<P: AsRef<Path>>(file_path: P) -> Result<Vec<Transaction>> {
    let mut spreadsheet: Xlsx<_> = open_workbook(file_path)?;
    let Ok(range) = spreadsheet.worksheet_range("G&L_Expanded") else {
        return Err(Error::msg("missing sheet"));
    };
    let headers = range.headers().context("failed to extract headers")?;
    let (date_index, gain_loss_index, record_type_index, total_proceeds_index) =
        get_column_indices(headers)?;

    let mut exr_cache = ExchangeRateCache::new();
    let mut transactions = Vec::new();

    let mut year: i32 = 0;

    for r in range
        .rows()
        .skip(1)
        .filter(|r| r[record_type_index] == Data::String("Sell".to_string()))
    {
        let sell_date = Date::parse(
            r[date_index]
                .as_string()
                .context("wrong date field type")?
                .as_str(),
            &XLSX_DATE_FMT,
        )?;
        if year == 0 {
            year = sell_date.year()
        } else if year != sell_date.year() {
            return Err(Error::msg("all cells should be from the same fiscal year"));
        }

        let usd_proceeds = r[total_proceeds_index]
            .as_f64()
            .context("wrong total proceeds field type")?;

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
            usd_proceeds,
            eur_proceeds: usd_proceeds / exr,
        });
    }
    Ok(transactions)
}

fn compute_period_report(
    transactions: &[Transaction],
    period: Option<(Date, Date)>,
) -> PeriodTaxReport {
    let (usd_gain, usd_loss, eur_gain, eur_loss, usd_proceeds, eur_proceeds) = transactions
        .iter()
        .filter(|t| {
            if let Some((period_start, period_end)) = period {
                t.sell_date >= period_start && t.sell_date <= period_end
            } else {
                true
            }
        })
        .fold(
            (0., 0., 0., 0., 0., 0.),
            |(usd_gain, usd_loss, eur_gain, eur_loss, usd_proceeds, eur_proceeds), t| {
                (
                    usd_gain + t.usd_gain,
                    usd_loss + t.usd_loss,
                    eur_gain + t.eur_gain,
                    eur_loss + t.eur_loss,
                    usd_proceeds + t.usd_proceeds,
                    eur_proceeds + t.eur_proceeds,
                )
            },
        );
    let usd_net_gain = usd_gain - usd_loss;
    let eur_net_gain = eur_gain - eur_loss;
    PeriodTaxReport {
        usd_gain,
        usd_loss,
        usd_net_gain,
        eur_gain,
        eur_loss,
        eur_net_gain,
        usd_proceeds,
        eur_proceeds,
    }
}

fn compute_year_report(transactions: &[Transaction]) -> TaxReport {
    let fiscal_year = transactions
        .first()
        .map(|t| t.sell_date.year())
        .unwrap_or_default();
    let period_tax_report = compute_period_report(transactions, None);
    let eur_taxable_gain = f64::max(period_tax_report.eur_net_gain - EXEMPTION_EUR, 0.);
    let eur_tax = eur_taxable_gain * TAX_RATE;
    TaxReport {
        fiscal_year,
        period_tax_report,
        eur_taxable_gain,
        eur_tax,
    }
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
        "USD Proceeds",
        "EUR Proceeds",
    ])?;
    for t in transactions {
        wtr.write_record(&[
            t.sell_date.format(EXR_API_DATE_FMT)?,
            t.usd_gain.to_string(),
            t.usd_loss.to_string(),
            t.eur_gain.to_string(),
            t.eur_loss.to_string(),
            t.exr.to_string(),
            t.usd_proceeds.to_string(),
            t.eur_proceeds.to_string(),
        ])?;
    }
    println!(
        "The transaction detail was written as CSV to file {}",
        file_path.as_ref().to_string_lossy()
    );
    Ok(())
}

pub fn compute_and_print_report(transactions: &[Transaction]) -> Result<()> {
    let yr_report = compute_year_report(transactions);
    let yr = yr_report.fiscal_year;

    // Jan 1st to Nov 30th
    let period = (
        Date::from_calendar_date(yr, Month::January, 1)?,
        Date::from_calendar_date(yr, Month::November, 30)?,
    );
    print_period_header(period)?;
    let period_report = compute_period_report(transactions, Some(period));
    print_period_report(&period_report);

    // Dec 1st to Dec 31st
    let period = (
        Date::from_calendar_date(yr, Month::December, 1)?,
        Date::from_calendar_date(yr, Month::December, 31)?,
    );
    print_period_header(period)?;
    let period_report = compute_period_report(transactions, Some(period));
    print_period_report(&period_report);

    // Full year
    println!(
        "\n=== TAX REPORT FOR ENTIRE FISCAL YEAR {} ===\n",
        yr_report.fiscal_year
    );
    print_period_report(&yr_report.period_tax_report);
    println!(
        "\nTaxable gain (amount above exemption): €{:.2}",
        yr_report.eur_taxable_gain
    );
    println!(
        "Tax to pay ({:.2}%): €{}",
        TAX_RATE * 100.,
        yr_report.eur_taxable_gain * TAX_RATE
    );
    Ok(())
}

fn print_period_header(period: (Date, Date)) -> Result<()> {
    println!(
        "\n=== TAX REPORT FOR PERIOD {} TO {} ===\n",
        period.0.format(EXR_API_DATE_FMT)?,
        period.1.format(EXR_API_DATE_FMT)?
    );
    Ok(())
}

fn print_period_report(report: &PeriodTaxReport) {
    println!("Total proceeds (USD): ${:.2}", report.usd_proceeds);
    println!("Total gain (USD): ${:.2}", report.usd_gain);
    println!("Total loss (USD): ${:.2}", report.usd_loss);
    println!("Net gain (USD): ${:.2}\n", report.usd_net_gain);
    println!("Total proceeds: €{:.2}", report.eur_proceeds);
    println!("Total gain: €{:.2}", report.eur_gain);
    println!("Total loss: €{:.2}", report.eur_loss);
    println!("Net gain (Gain-Loss): €{:.2}", report.eur_net_gain);
}
