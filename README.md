Capital Gain Tax calculator
===========================

This program computes the Capital Gain Tax (in Ireland) for stock sold through ETrade.
This is strongly inspired of wladh's CGT.jl: https://github.com/wladh/CGT.jl

First, download the excel sheet with the trades for the year from Etrade:

        → Click "Stock Plan" 
          ↳ Click "My Account" → "Gains & Losses"
          ↳ Select the right tax year
          ↳ Click "Apply"
          ↳ Click "Download" → "Download Expanded"

> [!WARNING]
> This only works if you sold stock in the same order as you got them. [Irish revenue applies a First-In-First-Out
rule to sold shares](https://www.revenue.ie/en/gains-gifts-and-inheritance/transfering-an-asset/selling-or-disposing-of-shares.aspx),
whereas Etrade attaches the sale transaction to a specific set of stock and does not force
you to sell following the FIFO rule. This program does not account for this so if you sold stock in disorder, it
will not be compliant with revenue's FIFO rule.

Usage
-----

Install Rust and Cargo: https://rustup.rs/

Run the program with `cargo run <path_to_excel_file>`

How to fill Form 11
-------------------

> [!CAUTION]
> **Disclaimer**: I am not an accountant and I am not 100% sure that this is the right way to do it. I may have made a mistake
or this might not suit your specific situation. Use at your own risks. If you notice a mistake, please report it by
opening an issue on this repository.

### Capital Gains section

Here is how I fill the Capital Gains section:

![year part](https://github.com/user-attachments/assets/3d471e7f-4662-4a3e-b2cb-d6b0f027f2b1)

![period part](https://github.com/user-attachments/assets/58705e43-2065-451d-9221-27dcb2bd8039)

### CGT Self assessment section

For the self assessment, I use the `Net chargeable gain (amount above exemption)` and `Tax to pay (33.00%)` values
displayed at the end of the program output. Unlike the values in the Capital Gains section, these values
deduct the €1270,00 exemption from the result. 

### Total proceeds

![Total proceeds](https://github.com/user-attachments/assets/83d32337-3436-4b55-982a-52773d542403)
