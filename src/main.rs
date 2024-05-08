use std::process;

use crate::transaction::engine::PaymentEngine;
use crate::transaction::Transaction;

mod account;
#[macro_use]
mod macros;
mod transaction;
/// Reads a csv transaction file, builds the payment engine and outputs errors.
fn read_csv_into_engine(file: &str) -> Result<PaymentEngine, csv::Error> {
    // this structure does our accounting
    let mut engine = PaymentEngine::default();

    // reading input
    let reader = Transaction::read_from_file(file);
    match reader {
        Ok(iter) => {
            let mut previous_error = false;

            // check to see if there is at least one valid row
            let mut peekable_iter = iter.peekable();
            if peekable_iter.peek().is_none() {
                eprintln_featureflag!(
                    "csv error: table is empty, all rows had errors or columns don't match"
                );
            }

            // perform each transaction as they are read into the program, line-by-line
            for (row, result) in peekable_iter.enumerate() {
                if let Ok(transaction) = result {
                    if let Err(e) = engine.perform_transaction(transaction) {
                        if !previous_error {
                            eprintln_featureflag!("errors: ");
                            previous_error = true;
                        }
                        eprintln_featureflag!("  {}", e);
                    }
                } else {
                    // invalid line in csv
                    eprintln_featureflag!("csv error: deserialize of row {} failed", row);
                }
            }
            Ok(engine)
        }
        Err(e) => {
            eprintln_featureflag!("failed to open file: {}", file);
            Err(e)
        }
    }
}

fn usage(program: &str) {
    println!("usage: {} [input.csv]", program);
    println!("       Calculates account balances from a list of transactions.");
    process::exit(0);
}

fn main() {
    // argument validation
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 || args[1] == "--help" {
        usage(&args[0]);
    }

    // attempt to read the file
    match read_csv_into_engine(&args[1]) {
        Ok(engine) => {
            let mut wtr = csv::WriterBuilder::new().from_writer(std::io::stdout());
            // write the output
            for (_, account) in engine.accounts_iter() {
                if let Err(e) = wtr.serialize(account) {
                    eprintln_featureflag!("Failed to output an account record! {}", e);
                }
            }
        }
        Err(e) => {
            eprintln_featureflag!("{}", e);
            process::exit(-1);
        }
    }
}
