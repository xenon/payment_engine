use serde::{ser::SerializeStruct, Deserialize, Serialize};

// a total is not maintained since it is always calculatable from available and held
#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Account {
    client: u16,
    available: f64,
    held: f64,
    locked: bool,
}

// Account is used like a database entry, not a lot of complex logic happening in here
// Account is fully opaque to enforce a strict interface making it less prone to errors
impl Account {
    pub fn new(client: u16) -> Self {
        Account {
            client,
            ..Default::default()
        }
    }

    // some getters for black-box testing
    #[cfg(test)]
    pub fn client(&self) -> u16 {
        self.client
    }

    #[cfg(test)]
    pub fn available(&self) -> f64 {
        self.available
    }

    #[cfg(test)]
    pub fn held(&self) -> f64 {
        self.held
    }

    // getters used in the program, total isn't actually stored in the struct
    pub fn total(&self) -> f64 {
        self.available + self.held
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    // transaction actions
    pub fn deposit(&mut self, amount: f64) {
        self.available += amount;
    }

    pub fn withdrawal(&mut self, amount: f64) -> bool {
        let can_withdrawal = self.available >= amount;
        if can_withdrawal {
            self.available -= amount;
        }
        can_withdrawal
    }

    pub fn dispute(&mut self, amount: f64) {
        self.available -= amount;
        self.held += amount;
    }

    pub fn resolve(&mut self, amount: f64) {
        self.held -= amount;
        self.available += amount;
    }

    pub fn chargeback(&mut self, amount: f64) {
        self.held -= amount;
        self.locked = true;
    }

    /// Used to deserialize byte strings in tests
    #[cfg(test)]
    fn read_from_bytes(bytes: &[u8]) -> impl Iterator<Item = Result<Account, csv::Error>> + '_ {
        csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(bytes)
            .into_deserialize::<Account>()
    }
}

// Implement serialize manually for two reasons:
// 1. 'total' is injected and calculated at serialization time from available and held amounts
// 2. to output rounded floats to 4 decimal places
impl Serialize for Account {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let f64_round = |val: f64| -> f64 {
            let precision = 10000_f64; // 10000 means round to 4 decimal places
            f64::round(val * precision) / precision
        };

        let mut state = serializer.serialize_struct("Account", 5)?;
        state.serialize_field("client", &self.client)?;
        state.serialize_field("available", &f64_round(self.available))?;
        state.serialize_field("held", &f64_round(self.held))?;
        state.serialize_field("total", &f64_round(self.total()))?;
        state.serialize_field("locked", &self.locked)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deposit_and_withdrawal() {
        // testing the account functions, should be straightforward
        let mut acc = Account::new(1);
        acc.deposit(45.5);
        acc.withdrawal(20.5);

        assert_eq!(acc.client(), 1);
        assert_eq!(acc.available(), 25.0);
        assert_eq!(acc.held(), 0.0);
        assert_eq!(acc.total(), 25.0);
        assert_eq!(acc.locked(), false);
    }

    #[test]
    fn dispute() {
        let mut acc = Account::new(1);
        acc.deposit(50.0);
        acc.deposit(25.0);
        acc.dispute(25.0);

        assert_eq!(acc.client(), 1);
        assert_eq!(acc.available(), 50.0);
        assert_eq!(acc.held(), 25.0);
        assert_eq!(acc.total(), 75.0);
        assert_eq!(acc.locked(), false);
    }

    #[test]
    fn verify_serialize_and_decimal_precision() {
        // input float and its expected rounded output
        let in_float = 20.33338;
        let out_float = 20.3334;
        // also check against the expected output deserialized
        let expected_output = r#"
        client, available, held, total, locked
        1, 20.3334, 0.0, 20.3334, false"#;
        // setup the example
        let mut acc = Account::new(1);
        acc.deposit(in_float);
        // serialize the example
        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.serialize(acc).ok();
        let serialize_str = String::from_utf8(wtr.into_inner().unwrap()).unwrap();
        // deserialize, check precision and check against deserialized expected output
        let reader = Account::read_from_bytes(serialize_str.as_bytes());
        let expected_reader = Account::read_from_bytes(expected_output.as_bytes());
        let mut count = 0;
        for (acc, expected_acc) in reader.zip(expected_reader) {
            let acc: Account = acc.unwrap();
            let expected_acc: Account = expected_acc.unwrap();
            assert_eq!(acc.available(), out_float);
            assert_eq!(acc.total(), out_float);
            assert_eq!(acc.held(), 0.0);
            assert_eq!(acc.locked(), false);
            assert_eq!(acc, expected_acc);
            count += 1;
        }
        assert_eq!(count, 1); // we should run the loop exactly once
    }
}
