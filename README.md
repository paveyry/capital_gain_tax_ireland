Capital Gain Tax calculator
===========================

This program computes the Capital Gain Tax (in Ireland) for stock sold through ETrade.
This is strongly inspired of wladh's CGT.jl: https://github.com/wladh/CGT.jl

First, download the excel sheet with the trades for the year from Etrade:

        Stock Plan -> My Account -> Gains & Losses -> Select tax year -> Apply -> Download -> Download Expanded.


Usage
-----

Install Rust and Cargo

Run the program with `cargo run <path_to_excel_file>`