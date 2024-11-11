use std::env;

use capital_gain_tax_ireland::{
    compute_and_print_output, get_transactions, write_detail_as_csv, Result,
};

use anyhow::Error;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        return Err(Error::msg("Usage: ./cgt \"path/to/file.xlsx\""));
    }

    let transactions = get_transactions(&args[1])?;
    write_detail_as_csv(&transactions, "CGT_transaction_detail.csv")?;
    compute_and_print_output(&transactions);

    Ok(())
}
