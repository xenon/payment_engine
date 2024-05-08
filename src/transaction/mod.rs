use serde::Deserialize;

pub(crate) mod engine;

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all(deserialize = "lowercase"))] // read the strings as lowercase
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Disputed,
    Resolved,
    Chargeback,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename = "type")] // parse this field as 'type' not 'transaction_type'
    transaction_type: TransactionType,
    client: u16,
    tx: u32,
    amount: Option<f64>, // only should be 'Some' if the type is Deposit or Withdrawal
    #[serde(skip_deserializing)] // not serialized, internal use for disputes
    dispute_status: Option<DisputeStatus>,
}

impl Transaction {
    #[cfg(test)]
    pub fn new(
        transaction_type: TransactionType,
        client: u16,
        tx: u32,
        amount: Option<f64>,
    ) -> Self {
        Self {
            transaction_type,
            client,
            tx,
            amount,
            dispute_status: None,
        }
    }

    /// Ensure that only expected transaction types have amounts.
    /// Since serde can't guarantee the amount field is set according to type we enforce it manually.
    fn validate(&self) -> bool {
        self.amount.is_some() == self.transaction_type.should_have_amount()
    }

    /// Enforces additional restrictions when reading a 'Transaction'.
    /// Namely that some types must have amounts while others must not.
    /// Filters out the transactions which are invalid.
    pub fn read_from_file(
        file: &str,
    ) -> Result<impl Iterator<Item = Result<Transaction, csv::Error>> + '_, csv::Error> {
        Ok(csv::ReaderBuilder::new()
            .trim(csv::Trim::All) // allow whitespace
            .flexible(true) // avoid the extra comma after dispute, resolve and chargeback
            .from_path(file)?
            .into_deserialize::<Transaction>()
            .filter(|res_transaction| {
                res_transaction
                    .as_ref()
                    .map_or_else(|_| false, |t| t.validate())
            }))
    }

    /// Enforces additional restrictions when reading a 'Transaction'.
    /// Namely that some types must have amounts while others must not.
    /// Filters out the transactions which are invalid.
    #[cfg(test)]
    fn read_from_bytes(bytes: &[u8]) -> impl Iterator<Item = Result<Transaction, csv::Error>> + '_ {
        csv::ReaderBuilder::new()
            .trim(csv::Trim::All) // allow whitespace
            .flexible(true) // avoid the extra comma after dispute, resolve and chargeback
            .from_reader(bytes)
            .into_deserialize::<Transaction>()
            .filter(|res_transaction| {
                res_transaction
                    .as_ref()
                    .map_or_else(|_| false, |t| t.validate())
            })
    }

    // Disputes work like a state machine:
    // First the Transaction transitions to the 'Disputed' status
    // From there either 'Resolved' or 'Chargeback' status

    /// Only transactions stored in the transaction engine should have a dispute status
    fn in_dispute(&self) -> bool {
        self.dispute_status.is_some()
    }

    /// Is it the right type of transaction to be disputed?
    fn dispute_possible(&self) -> bool {
        matches!(
            self.transaction_type,
            TransactionType::Deposit | TransactionType::Withdrawal
        )
    }

    /// Start a dispute on the transaction if possible
    pub fn dispute(&mut self) -> bool {
        let can_dispute = self.dispute_possible() && self.dispute_status.is_none();
        if can_dispute {
            self.dispute_status = Some(DisputeStatus::Disputed);
        }
        can_dispute
    }

    /// Resolve a dispute on the transaction if possible
    pub fn resolve(&mut self) -> bool {
        let can_resolve =
            self.dispute_possible() && self.dispute_status == Some(DisputeStatus::Disputed);
        if can_resolve {
            self.dispute_status = Some(DisputeStatus::Resolved);
        }
        can_resolve
    }

    /// Chargeback a dispute on the transaction if possible
    pub fn chargeback(&mut self) -> bool {
        let can_chargeback =
            self.dispute_possible() && self.dispute_status == Some(DisputeStatus::Disputed);
        if can_chargeback {
            self.dispute_status = Some(DisputeStatus::Chargeback);
        }
        can_chargeback
    }
}

impl TransactionType {
    /// Used to ensure correctness of transaction type, only some transactions have an amount field
    const fn should_have_amount(self) -> bool {
        matches!(self, TransactionType::Deposit | TransactionType::Withdrawal)
    }

    /// Is the transaction either a deposit or a withdrawal?
    /// If so it's going to be a new transaction record we have to keep
    const fn is_new_transaction(self) -> bool {
        // The duplication here is for clarity
        self.should_have_amount()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_many_errors() {
        let csv = r#"
        type, client, tx, amount
        despotic, 1, 23, 4.0
        withdrawal, 1.25, 1, 2
        , 25, 1, hello
        "#;
        for _ in Transaction::read_from_bytes(csv.as_bytes()) {
            // all the lines are wrong and should be filtered out, so we never reach the inner loop
            assert!(false);
        }
    }

    #[test]
    fn parse_amount_errors() {
        // check to see if the 'amount' field is where it should be
        // all of the transactions below are wrong
        let csv = r#"
        type, client, tx, amount
        deposit, 1, 23,
        withdrawal, 1, 24,
        dispute, 1, 23, 444.42
        resolve, 1, 23, 444.75
        chargeback, 1, 24, 999.9"#;
        for transaction in Transaction::read_from_bytes(csv.as_bytes()) {
            assert!(transaction.is_ok());
            let transaction: Transaction = transaction.unwrap();
            // assert that we have the wrong configuration in the given transaction
            assert_ne!(
                transaction.amount.is_some(),
                transaction.transaction_type.should_have_amount()
            );
        }
    }

    #[test]
    fn dispute_states() {
        // make sure the state transitions for disputes functions properly
        let mut transaction = Transaction::new(TransactionType::Deposit, 1, 1, Some(500.0));
        assert!(!transaction.in_dispute());
        // move into a disputed state
        assert!(transaction.dispute());
        assert!(!transaction.dispute()); // can't re-apply a dispute
        assert!(transaction.in_dispute());
        // move into a resolved state
        assert!(transaction.resolve());
        assert!(!transaction.chargeback());
        assert!(!transaction.dispute());
    }
}
