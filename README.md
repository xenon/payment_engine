# Payment Engine
Takes a CSV file of transactions and produces a CSV of accounts and their balances.
## Running
```sh
cargo run -- input.csv
```
- The output account CSV data is written to `stdout`, redirect it with `>` to a file
- Transaction errors are written to `stderr` if the feature flag ``printerrors`` was set at compilation (off by default)
## Transaction CSV Format [Input]
- `type`: action to perform *[deposit, withdrawal, dispute, resolve, chargeback]*
- `client`: client id *[16bit unsigned int]*
- `tx`: transaction number *[32bit unsigned int]*
- `amount`: amount to use *[64bit float, up to 4 digits precision]*
### Example:
```
type,client,tx,amount
deposit,1,1,25.0
withdrawal,1,2,10.0
dispute,1,2
```
## Account CSV Format [Output]
- `client`: client id *[16bit unsigned int]*
- `available`: available balance *[64bit float, up to 4 digits precision]*
- `held`: held balance *[64bit float, up to 4 digits precision]*
- `total`: sum of available and held *[64bit float, up to 4 digits precision]*
- `locked`: whether the account is frozen *[boolean]*
### Example:
```
client,available,held,total,locked
1,25.0,0.0,25.0,false
2,100.0,15.0,115.0,false
```
## Error Handling
Payment Engine errors are raised when processing invalid transactions. Invalid transactions are effectively ignored and the error is printed to stderr.
### List of Payment Engine errors
- **Invalid Transaction:** not enough data or invalid fields
- **Duplicate Transaction:** reused a transaction id which must be unique
- **Account Locked:** the account requested is locked
- **Non-Positive Amount:** the `amount` field was not a positive number
- **Insufficient Funds:** can't withdrawal money which is not there
- **Non-existing Dispute:** can't dispute a transaction that is not there
- **Client Mismatch:** client may only dispute their own transactions
- **Invalid Dispute/Resolve/Chargeback:** criteria not met for the action
### Setting the error flag
Pass in ``--features printerrors`` in the ``cargo`` command to see the full error output.
```sh
cargo run --features printerrors -- input.csv
```
This flag defaults to being off because on files with a lot of errors the printing bottlenecks the program.
## Testing
Each module in the crate has its own unit test suite.
### Running the tests
```sh
cargo test
```
### Test csv files
The sub-directory `tests` has a bunch of test files used in manual tests, including a million line file used to test memory usage (zipped in `f.zip`).
## Efficiency
### CSV Efficiency
The CSV file is streamed line-by-line, the entire file is **not** read into memory at once. Tested with a million line file filled with `disputes` and the memory usage stayed constant because the transaction `dispute` does not allocate extra memory.
### Payment Engine Efficiency
There should be no problem reading much more data into the `PaymentEngine` but it can probably be organized more efficiently for concurrency.
The `PaymentEngine` executes each transaction as it comes in, keeping the records and accounts up to date. It will only store data if it could be used in the future for accounting.
## Correctness
- `serde` and the type system enforce the correctness of structs for the most part.
  - An exception is `amount: Option<f64>` in the `Transaction` struct which is enforced by the interface to Transaction.
- Unit tests serve to make sure each function is working correctly
  - The engine is tested on every type of error that it can raise
  - Decimal precision is tested by serializing a long decimal then deserializing it and checking if the digits are rounded to 4 places.
  - Tests to make sure some serialization edge cases succeed
- `tests` folder of CSVs serve to test the entire program at once
## Known Limitations and Design Choices
- `type` fields of a transaction csv must be lowercase
- Why `f64` floats for currency amounts?
  - a better alternative would be some exact decimal crate
  - `f64` was used for simplicity, with more exactness than `f32`
- Chose not to use a command line arg parser (like `clap`) because the arguments are simple
## Assumptions
### Types
- Client ids can be any `u16` value, not necessarily increasing from zero
- Transaction ids can be any `u32` value, not necessarily increasing from zero
- Round to four digits of precision, not truncate
### Semantics of Transactions
- After a dispute is resolved the transaction can not be disputed again
- Dispute, Resolve and Chargeback are no more complex than stated