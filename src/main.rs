use std::process;

use crate::transaction::engine::PaymentEngine;
use crate::transaction::Transaction;

mod account;
mod transaction;

/// Reads a csv transaction file, builds the payment engine and outputs errors.
fn read_file(file: &str) -> Result<PaymentEngine, csv::Error> {
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
                eprintln!("csv error: table is empty or all rows had errors");
            }

            // perform each transaction as they are read into the program, line-by-line
            for (i, result) in peekable_iter.enumerate() {
                if let Ok(transaction) = result {
                    if let Err(e) = engine.perform_transaction(transaction) {
                        if !previous_error {
                            eprintln!("errors: ");
                            previous_error = true;
                        }
                        eprintln!("{}", e);
                    }
                } else {
                    // invalid line in csv
                    eprintln!("csv error: deserialize of row {} failed", i);
                }
            }
            Ok(engine)
        }
        Err(e) => {
            eprintln!("failed to open file: {}", file);
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
    match read_file(&args[1]) {
        // ignore the extra errors
        Ok(engine) => {
            let mut wtr = csv::WriterBuilder::new().from_writer(std::io::stdout());
            // write the output
            for (_, account) in engine.accounts_iter() {
                if let Err(e) = wtr.serialize(account) {
                    eprintln!("Failed to output an account record! {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(-1);
        }
    }
}
