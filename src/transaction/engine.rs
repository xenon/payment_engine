use std::collections::HashMap;

use crate::{
    account::Account,
    transaction::{Transaction, TransactionType},
};

/// Error type for invalid transactions
pub enum TransactionError {
    InvalidTransaction(u32),
    DuplicateTransaction(u32),
    AccountLocked(u16),
    NonPositiveAmount(u16, u32, f64),
    InsufficientFunds(u16),
    NonExistingDisputeResolveOrChargeback(u16, u32),
    ClientMismatch(u16, u32, u16),
    InvalidDispute(u16, u32),
    InvalidResolve(u16, u32),
    InvalidChargeback(u16, u32),
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionError::InvalidTransaction(tx) => {
                write!(f, "transaction '{}' formatted incorrectly", tx)
            }
            TransactionError::DuplicateTransaction(tx) => {
                write!(
                    f,
                    "transaction '{}' already exists in the transaction engine",
                    tx
                )
            }
            TransactionError::AccountLocked(client) => write!(f, "account '{}' is locked", client),
            TransactionError::NonPositiveAmount(client, tx, amount) => write!(
                f,
                "client '{}' tried to deposit/withdraw a non-positive amount '{}' in transaction '{}'",
                client, amount, tx
            ),
            TransactionError::InsufficientFunds(client) => {
                write!(f, "client '{}' has insufficient funds", client)
            }
            TransactionError::NonExistingDisputeResolveOrChargeback(client, tx) => write!(
                f,
                "client '{}' referred to transaction '{}' which doesn't exist",
                client, tx
            ),
            TransactionError::ClientMismatch(client, tx, tx_client) => {
                write!(
                    f,
                    "client '{}' referred to transaction '{}' which belongs to client '{}'",
                    client, tx, tx_client
                )
            }
            TransactionError::InvalidDispute(client, tx) => {
                write!(f, "client '{}' can't dispute transaction '{}'", client, tx)
            }
            TransactionError::InvalidResolve(client, tx) => {
                write!(f, "client '{}' can't resolve transaction '{}'", client, tx)
            }
            TransactionError::InvalidChargeback(client, tx) => {
                write!(
                    f,
                    "client '{}' can't chargeback transaction '{}'",
                    client, tx
                )
            }
        }
    }
}

#[derive(Default)]
pub struct PaymentEngine {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, Transaction>, // acceptable because transactions are globally unique, but could be under the client id
}

impl PaymentEngine {
    pub fn perform_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<(), TransactionError> {
        // Reading the function body will make these helpers easier to understand

        /// Withdrawals and Deposits create new transactions in the transaction record
        fn new_transaction(
            transactions: &mut HashMap<u32, Transaction>,
            account: &mut Account,
            transaction: Transaction,
        ) -> Result<(), TransactionError> {
            // assume that the transaction is a valid format before this function is called
            let amount = transaction.amount.unwrap();
            // check for duplicate transactions
            if transactions.contains_key(&transaction.tx) {
                return Err(TransactionError::DuplicateTransaction(transaction.tx));
            }
            // check for negative amounts
            if amount <= 0_f64 {
                return Err(TransactionError::NonPositiveAmount(
                    transaction.client,
                    transaction.tx,
                    amount,
                ));
            }
            match transaction.transaction_type {
                TransactionType::Deposit => account.deposit(amount),
                TransactionType::Withdrawal => {
                    if !account.withdrawal(amount) {
                        return Err(TransactionError::InsufficientFunds(transaction.client));
                    }
                }
                _ => unreachable!(),
            }
            transactions.insert(transaction.tx, transaction);
            Ok(())
        }
        /// Disputes, Resolves and Chargebacks refer to older transactions
        fn referring_transaction(
            transactions: &mut HashMap<u32, Transaction>,
            account: &mut Account,
            transaction: Transaction,
        ) -> Result<(), TransactionError> {
            // transaction refers to an old transaction
            let tx = transactions.get_mut(&transaction.tx);
            // make sure the old transaction exists
            if let Some(previous_transaction) = tx {
                if transaction.client != previous_transaction.client {
                    return Err(TransactionError::ClientMismatch(
                        transaction.client,
                        transaction.tx,
                        previous_transaction.client,
                    ));
                }
                // try the transaction action, if it succeeds apply the action on the account too
                match transaction.transaction_type {
                    TransactionType::Dispute => {
                        if previous_transaction.dispute() {
                            account.dispute(previous_transaction.amount.unwrap());
                        } else {
                            return Err(TransactionError::InvalidDispute(
                                transaction.client,
                                transaction.tx,
                            ));
                        }
                    }
                    TransactionType::Resolve => {
                        if previous_transaction.resolve() {
                            account.resolve(previous_transaction.amount.unwrap());
                        } else {
                            return Err(TransactionError::InvalidResolve(
                                transaction.client,
                                transaction.tx,
                            ));
                        }
                    }
                    TransactionType::Chargeback => {
                        if previous_transaction.chargeback() {
                            account.chargeback(previous_transaction.amount.unwrap());
                        } else {
                            return Err(TransactionError::InvalidChargeback(
                                transaction.client,
                                transaction.tx,
                            ));
                        }
                    }
                    _ => unreachable!(),
                }
            } else {
                // refers to non-existing transaction
                return Err(TransactionError::NonExistingDisputeResolveOrChargeback(
                    transaction.client,
                    transaction.tx,
                ));
            }
            Ok(())
        }
        // create the customer account if we've never seen it before
        if !self.accounts.contains_key(&transaction.client) {
            self.accounts
                .insert(transaction.client, Account::new(transaction.client));
        }
        // get the customer account
        let account = self
            .accounts
            .get_mut(&transaction.client)
            .expect("Account should have been added immediately before!");

        // attempt the transaction if the account is not locked
        if !account.locked() {
            // validation is already done upon parsing, but is done here again for interface safety
            // note: new transactions can't be inserted with a dispute status already set
            if !transaction.validate() || transaction.in_dispute() {
                return Err(TransactionError::InvalidTransaction(transaction.tx));
            }
            // perform the transaction on the account
            // transactions are grouped into making a new entry OR referring/modifying an old one
            if transaction.transaction_type.is_new_transaction() {
                new_transaction(&mut self.transactions, account, transaction)?;
            } else {
                referring_transaction(&mut self.transactions, account, transaction)?;
            }
            Ok(())
        } else {
            Err(TransactionError::AccountLocked(transaction.client))
        }
    }

    /// Iterate over all of the accounts in the engine
    pub fn accounts_iter(&self) -> impl Iterator<Item = (&u16, &Account)> {
        self.accounts.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_tx() {
        let mut engine = PaymentEngine::default();
        let tx_number = 1;
        // shouldn't have an amount, this should cause an error
        let res = engine.perform_transaction(Transaction::new(
            TransactionType::Dispute,
            1,
            tx_number,
            Some(1.0),
        ));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::InvalidTransaction(tx) => tx == tx_number,
            _ => false,
        });
    }

    #[test]
    fn duplicate_tx() {
        let mut engine = PaymentEngine::default();
        let tx_number = 1;
        let transaction = Transaction::new(TransactionType::Deposit, 1, tx_number, Some(1.0));
        let res = engine.perform_transaction(transaction.clone());
        assert!(res.is_ok());
        // duplicate the tx number, which is not valid
        let res2 = engine.perform_transaction(transaction);
        assert!(res2.is_err());
        assert!(match res2.unwrap_err() {
            TransactionError::DuplicateTransaction(tx) => tx == tx_number,
            _ => false,
        });
    }

    #[test]
    fn account_locked() {
        let mut engine = PaymentEngine::default();
        // first cause a chargeback
        let txs = [
            Transaction::new(TransactionType::Deposit, 1, 1, Some(10.50)),
            Transaction::new(TransactionType::Dispute, 1, 1, None),
            Transaction::new(TransactionType::Chargeback, 1, 1, None),
        ];
        for tx in txs {
            assert!(engine.perform_transaction(tx).is_ok());
        }
        // account should now be frozen
        let res = engine.perform_transaction(Transaction::new(
            TransactionType::Deposit,
            1,
            1,
            Some(9.50),
        ));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::AccountLocked(client) => client == 1,
            _ => false,
        });
    }

    #[test]
    fn non_positive_amount() {
        let mut engine = PaymentEngine::default();
        // try to transact a negative amount
        let res = engine.perform_transaction(Transaction::new(
            TransactionType::Deposit,
            1,
            1,
            Some(-9.50),
        ));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::NonPositiveAmount(client, transaction, amount) =>
                client == 1 && transaction == 1 && amount == -9.50,
            _ => false,
        });
    }

    #[test]
    fn insufficient_funds() {
        let mut engine = PaymentEngine::default();
        let res = engine.perform_transaction(Transaction::new(
            TransactionType::Withdrawal,
            1,
            1,
            Some(20.5),
        ));
        // can't withdrawal from an empty account!
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::InsufficientFunds(client) => client == 1,
            _ => false,
        });
    }

    #[test]
    fn non_existing_tx_for_dispute_resolve_chargeback() {
        let mut engine = PaymentEngine::default();
        // dispute
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Dispute, 1, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::NonExistingDisputeResolveOrChargeback(client, tx) =>
                client == 1 && tx == 1,
            _ => false,
        });
        // resolve
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Resolve, 2, 5, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::NonExistingDisputeResolveOrChargeback(client, tx) =>
                client == 2 && tx == 5,
            _ => false,
        });
        // chargeback
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Resolve, 3, 10, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::NonExistingDisputeResolveOrChargeback(client, tx) =>
                client == 3 && tx == 10,
            _ => false,
        });
    }

    #[test]
    fn client_mismatch() {
        let mut engine = PaymentEngine::default();
        // first deposit with client '1'
        let res = engine.perform_transaction(Transaction::new(
            TransactionType::Deposit,
            1,
            1,
            Some(120.0),
        ));
        assert!(res.is_ok());

        // then try various dispute actions with client '2', all should fail
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Dispute, 2, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::ClientMismatch(client, tx, owner) =>
                client == 2 && tx == 1 && owner == 1,
            _ => false,
        });

        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Resolve, 2, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::ClientMismatch(client, tx, owner) =>
                client == 2 && tx == 1 && owner == 1,
            _ => false,
        });

        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Chargeback, 2, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::ClientMismatch(client, tx, owner) =>
                client == 2 && tx == 1 && owner == 1,
            _ => false,
        });
    }

    #[test]
    fn invalid_dispute() {
        let mut engine = PaymentEngine::default();
        // first open a dispute
        let txs = [
            Transaction::new(TransactionType::Deposit, 1, 1, Some(10.50)),
            Transaction::new(TransactionType::Dispute, 1, 1, None),
        ];
        for tx in txs {
            assert!(engine.perform_transaction(tx).is_ok());
        }
        // try to open another dispute
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Dispute, 1, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::InvalidDispute(client, tx) => client == 1 && tx == 1,
            _ => false,
        })
    }

    #[test]
    fn invalid_resolve() {
        let mut engine = PaymentEngine::default();
        // first open a dispute
        let txs = [Transaction::new(
            TransactionType::Deposit,
            1,
            1,
            Some(10.50),
        )];
        for tx in txs {
            assert!(engine.perform_transaction(tx).is_ok());
        }
        // try to open another dispute
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Resolve, 1, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::InvalidResolve(client, tx) => client == 1 && tx == 1,
            _ => false,
        })
    }

    #[test]
    fn invalid_chargeback() {
        let mut engine = PaymentEngine::default();
        // first open a dispute
        let txs = [Transaction::new(
            TransactionType::Deposit,
            1,
            1,
            Some(10.50),
        )];
        for tx in txs {
            assert!(engine.perform_transaction(tx).is_ok());
        }
        // try to open another dispute
        let res =
            engine.perform_transaction(Transaction::new(TransactionType::Chargeback, 1, 1, None));
        assert!(res.is_err());
        assert!(match res.unwrap_err() {
            TransactionError::InvalidChargeback(client, tx) => client == 1 && tx == 1,
            _ => false,
        })
    }
}
