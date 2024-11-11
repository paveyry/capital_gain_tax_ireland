Capital Gain Tax calculator
===========================

This program computes the Capital Gain Tax (in Ireland) for stock sold through ETrade.
This is strongly inspired of wladh's CGT.jl: https://github.com/wladh/CGT.jl

First, download the excel sheet with the trades for the year from Etrade:

        Stock Plan -> My Account -> Gains & Losses -> Select tax year -> Apply -> Download -> Download Expanded.

Warning
-------

This only works if you sold stock in the same order as you got them. [Irish revenue applies a First-In-First-Out
rule to sold shares](https://www.revenue.ie/en/gains-gifts-and-inheritance/transfering-an-asset/selling-or-disposing-of-shares.aspx),
whereas Etrade attaches the sale transaction to a specific set of stock and does not force
you to sell following the FIFO rule. This program does not account for this so if you sold stock in disorder, it
will not be compliant with revenue's FIFO rule.

Usage
-----

Install Rust and Cargo: https://rustup.rs/

Run the program with `cargo run <path_to_excel_file>`